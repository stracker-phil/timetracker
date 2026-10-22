//! Recordings as the user makes them: validated, snapped, kept free of overlaps, and undoable. Every write
//! is an `ops::Op` applied in one transaction and pushed on the `UndoStack` (ADR 0010). A recording holds
//! one or more spans; the overlap rule applies per span (ADR 0020).

pub mod cli;
pub mod commands;
pub mod ops;
#[cfg(test)]
mod testing;
pub mod timer;
pub mod undo;

use chrono::{Duration, Local, LocalResult, NaiveTime, TimeZone, Timelike};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use ts_rs::TS;

use crate::db::projects::{self, Project};
pub use crate::db::recordings::Span;
use crate::db::recordings::{self, Recording, RecordingPatch, RecordingSpan, respanned};
use crate::db::time::{Day, Ms, now_ms, same_time_on};
use crate::db::{Db, new_id, tags};
use crate::error::{AppError, AppResult};
use ops::{Op, overlap_error};
use undo::{Entry, UndoStack};

const MINUTE_MS: Ms = 60_000;
const MAX_SPAN_MS: Ms = 24 * 60 * MINUTE_MS;
const MAX_NOTE_CHARS: usize = 500;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct RecordingInput {
    pub project_id: String,
    /// At least one; they must not overlap each other.
    pub spans: Vec<Span>,
    pub note: String,
    pub tag_id: Option<String>,
}

/// What a write does when its span overlaps other recordings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum Overlap {
    /// Trims, cuts or deletes the spans in the way.
    Replace,
    /// Refuses with `overlap`, listing their recordings.
    Cancel,
}

/// A write's result and the days whose views changed.
#[derive(Debug)]
pub struct Change<T> {
    pub value: T,
    pub days: Vec<Day>,
}

impl<T> Change<T> {
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Change<U> {
        Change {
            value: f(self.value),
            days: self.days,
        }
    }
}

/// `ms` moved to the nearest multiple of `minutes` counted from its local midnight, so DST days still land
/// on round wall-clock minutes. A tie rounds up.
pub fn snap(ms: Ms, minutes: u32) -> Ms {
    snap_in(ms, minutes, &Local)
}

pub fn snap_in<Tz: TimeZone>(ms: Ms, minutes: u32, tz: &Tz) -> Ms {
    let step = i64::from(minutes.max(1)) * MINUTE_MS;
    let Some(utc) = chrono::DateTime::from_timestamp_millis(ms) else {
        return ms;
    };
    let local = utc.with_timezone(tz).naive_local();
    let since_midnight = i64::from(local.time().num_seconds_from_midnight()) * 1000
        + i64::from(local.and_utc().timestamp_subsec_millis());
    let rounded = (since_midnight + step / 2).div_euclid(step) * step;
    let mut wall = local.date().and_time(NaiveTime::MIN) + Duration::milliseconds(rounded);
    // A wall time in a DST gap does not exist: the previous round one does. Bounded to a day of steps.
    for _ in 0..=(24 * 60 * MINUTE_MS / step) {
        match tz.from_local_datetime(&wall) {
            LocalResult::Single(t) => return t.timestamp_millis(),
            // The hour repeated when clocks go back: the pass `ms` is in.
            LocalResult::Ambiguous(a, b) => {
                let (a, b) = (a.timestamp_millis(), b.timestamp_millis());
                return if (a - ms).abs() <= (b - ms).abs() {
                    a
                } else {
                    b
                };
            }
            LocalResult::None => wall -= Duration::milliseconds(step),
        }
    }
    ms
}

/// Both ends snapped (see `past_start`).
pub fn snap_span(start: Ms, end: Ms, minutes: u32) -> Span {
    let start = snap(start, minutes);
    Span {
        start,
        end: past_start(start, snap(end, minutes), minutes),
    }
}

/// How long a recording made with New Recording lasts by default.
pub const NEW_RECORDING_MINUTES: i64 = 30;

/// The span a new recording without a selection proposes: the half hour up to `now`, snapped.
pub fn default_span(now: Ms, minutes: u32) -> Span {
    snap_span(now - NEW_RECORDING_MINUTES * MINUTE_MS, now, minutes)
}

/// An end that snapped onto or before the start lands one step after it.
fn past_start(start: Ms, end: Ms, minutes: u32) -> Ms {
    let step = i64::from(minutes.max(1)) * MINUTE_MS;
    if end <= start { start + step } else { end }
}

fn invalid(field: &str, message: &str) -> AppError {
    AppError::InvalidField {
        field: field.into(),
        message: message.into(),
    }
}

fn timer_running() -> AppError {
    AppError::InvalidInput("Stop the timer first".into())
}

fn checked_note(note: &str) -> AppResult<String> {
    if note.contains(['\n', '\r']) {
        return Err(invalid("note", "must be a single line"));
    }
    let note = note.trim();
    if note.chars().count() > MAX_NOTE_CHARS {
        return Err(invalid(
            "note",
            &format!("must be at most {MAX_NOTE_CHARS} characters"),
        ));
    }
    Ok(note.to_string())
}

fn check_length(span: Span) -> AppResult<()> {
    if span.end - span.start > MAX_SPAN_MS {
        let hours = MAX_SPAN_MS / (60 * MINUTE_MS);
        return Err(invalid(
            "end",
            &format!("must be at most {hours} h after the start"),
        ));
    }
    Ok(())
}

/// `span` checked and snapped.
fn snapped_span(span: Span, snap_minutes: u32) -> AppResult<Span> {
    if span.end <= span.start {
        return Err(invalid("end", "must be after the start"));
    }
    let span = snap_span(span.start, span.end, snap_minutes);
    check_length(span)?;
    Ok(span)
}

/// Each span checked and snapped, by start; refused when there is none. Whether they overlap each other
/// is `recordings::check_spans`'s call once they belong to a recording.
fn snapped_spans(spans: &[Span], snap_minutes: u32) -> AppResult<Vec<Span>> {
    if spans.is_empty() {
        return Err(invalid("spans", "must hold at least one span"));
    }
    let mut snapped = spans
        .iter()
        .map(|&span| snapped_span(span, snap_minutes))
        .collect::<AppResult<Vec<_>>>()?;
    snapped.sort_by_key(|s| s.start);
    Ok(snapped)
}

/// Closed spans with new ids.
fn new_spans(spans: &[Span]) -> Vec<RecordingSpan> {
    spans
        .iter()
        .map(|s| RecordingSpan::new(s.start, Some(s.end)))
        .collect()
}

/// Refuses the running timer: only finished recordings are edited.
fn require_finished(recording: &Recording) -> AppResult<()> {
    if recording.is_running() {
        return Err(timer_running());
    }
    Ok(())
}

/// `spans` with the span `span_id` set to `[start, end)`, keeping its id.
fn with_span_set(spans: &[RecordingSpan], span_id: &str, start: Ms, end: Ms) -> Vec<RecordingSpan> {
    spans
        .iter()
        .map(|s| {
            if s.id == span_id {
                RecordingSpan {
                    id: s.id.clone(),
                    start,
                    end: Some(end),
                }
            } else {
                s.clone()
            }
        })
        .collect()
}

/// The closed spans of a finished recording.
fn closed_spans(recording: &Recording) -> AppResult<Vec<Span>> {
    recording
        .spans
        .iter()
        .map(|s| {
            s.end.map(|end| Span {
                start: s.start,
                end,
            })
        })
        .collect::<Option<_>>()
        .ok_or_else(timer_running)
}

/// The project; an archived one only when the recording already belongs to it.
async fn usable_project(db: &Db, id: &str, current: Option<&str>) -> AppResult<Project> {
    let project = projects::get(db, id).await?;
    if project.archived_at.is_some() && current != Some(id) {
        return Err(invalid("projectId", "is archived"));
    }
    Ok(project)
}

async fn check_tag(db: &Db, tag_id: Option<&str>) -> AppResult<()> {
    let Some(id) = tag_id else { return Ok(()) };
    if tags::list(db).await?.iter().any(|t| t.id == id) {
        Ok(())
    } else {
        Err(invalid("tagId", "is an unknown tag"))
    }
}

/// `1 h 05 min`, `45 min`.
fn duration_text(ms: Ms) -> String {
    let minutes = ms / MINUTE_MS;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, m) => format!("{h} h {m:02} min"),
    }
}

/// The local days a span touches; an empty span its start's day.
fn days_of(start: Ms, end: Ms) -> Vec<Day> {
    let last = Day::of((end - 1).max(start));
    let mut day = Day::of(start);
    let mut days = vec![day];
    while day < last {
        day = day.next();
        days.push(day);
    }
    days
}

fn days_touched(op: &Op, now: Ms) -> Vec<Day> {
    let mut days: Vec<Day> = op
        .recordings()
        .iter()
        .flat_map(|r| &r.spans)
        .flat_map(|s| days_of(s.start, s.end.unwrap_or(now)))
        .collect();
    days.sort();
    days.dedup();
    days
}

/// `[start, end)` without `cut`: nothing, one piece or two.
fn without(start: Ms, end: Ms, cut: Span) -> Vec<(Ms, Ms)> {
    if cut.end <= start || cut.start >= end {
        return vec![(start, end)];
    }
    let mut pieces = Vec::new();
    if start < cut.start {
        pieces.push((start, cut.start));
    }
    if end > cut.end {
        pieces.push((cut.end, end));
    }
    pieces
}

/// What clears `cut` of `other`'s spans: a span inside it goes, one sticking out is trimmed, one around it
/// is cut in two (both stay spans of `other`). Without spans left, `other` is deleted.
fn clear_op(other: Recording, cut: &[Span], now: Ms) -> AppResult<Op> {
    let mut rest = Vec::new();
    for span in &other.spans {
        let Some(end) = span.end else {
            if cut.iter().any(|c| c.end > span.start) {
                return Err(timer_running());
            }
            rest.push(span.clone());
            continue;
        };
        let mut pieces = vec![(span.start, end)];
        for &c in cut {
            pieces = pieces
                .into_iter()
                .flat_map(|(s, e)| without(s, e, c))
                .collect();
        }
        for (i, (start, end)) in pieces.into_iter().enumerate() {
            let id = if i == 0 { span.id.clone() } else { new_id() };
            rest.push(RecordingSpan {
                id,
                start,
                end: Some(end),
            });
        }
    }
    if rest.is_empty() {
        return Ok(Op::Delete { recording: other });
    }
    rest.sort_by_key(|s| s.start);
    let after = Recording {
        spans: rest,
        updated_at: now,
        ..other.clone()
    };
    Ok(Op::Update {
        before: other,
        after,
    })
}

/// The time `recording` claims: its spans, an open one up to the end of time.
fn claimed(recording: &Recording) -> Vec<Span> {
    recording
        .spans
        .iter()
        .map(|s| Span {
            start: s.start,
            end: s.end.unwrap_or(Ms::MAX),
        })
        .collect()
}

/// Other recordings with a span sharing time with one of `spans`, each once.
async fn in_the_way(db: &Db, spans: &[Span], except: &str) -> AppResult<Vec<Recording>> {
    let mut others: Vec<Recording> = Vec::new();
    for span in spans {
        for other in recordings::overlapping(db, span.start, span.end, Some(except)).await? {
            if !others.iter().any(|o| o.id == other.id) {
                others.push(other);
            }
        }
    }
    others.sort_by_key(Recording::start);
    Ok(others)
}

/// `base` as is when nothing is in the way of `new`, else per `overlap`: refused, or batched after what
/// clears its spans.
async fn clearing(
    db: &Db,
    base: Op,
    new: &Recording,
    overlap: Overlap,
    label: &str,
) -> AppResult<Op> {
    let cut = claimed(new);
    let others = in_the_way(db, &cut, &new.id).await?;
    if others.is_empty() {
        return Ok(base);
    }
    if overlap == Overlap::Cancel {
        return Err(overlap_error(others));
    }
    let now = now_ms();
    let mut ops = Vec::new();
    for other in others {
        ops.push(clear_op(other, &cut, now)?);
    }
    ops.push(base);
    Ok(Op::Batch {
        label: label.to_string(),
        ops,
    })
}

/// `current` with `patch` applied: changed spans snapped, the rest validated.
fn patched(current: &Recording, patch: RecordingPatch, snap_minutes: u32) -> AppResult<Recording> {
    require_finished(current)?;
    let spans = match &patch.spans {
        Some(spans) => respanned(&current.spans, &snapped_spans(spans, snap_minutes)?),
        None => current.spans.clone(),
    };
    recordings::check_spans(&spans)?;
    let note = match &patch.note {
        Some(note) => checked_note(note)?,
        None => current.note.clone(),
    };
    Ok(Recording {
        project_id: patch
            .project_id
            .unwrap_or_else(|| current.project_id.clone()),
        spans,
        note,
        tag_id: patch.tag_id.unwrap_or_else(|| current.tag_id.clone()),
        updated_at: now_ms(),
        ..current.clone()
    })
}

/// `recording` with `spans` in place of its own, by start; refused when two overlap.
fn with_spans(recording: &Recording, mut spans: Vec<RecordingSpan>) -> AppResult<Recording> {
    spans.sort_by_key(|s| s.start);
    let after = Recording {
        spans,
        updated_at: now_ms(),
        ..recording.clone()
    };
    recordings::check_spans(&after.spans)?;
    Ok(after)
}

/// `span` on `day`: the same local start time, the same length.
fn span_on_day(span: Span, day: Day) -> Span {
    let start = same_time_on(span.start, day);
    Span {
        start,
        end: start + (span.end - span.start),
    }
}

/// Every span of `recording` moved by whole days so that its first span lands on `day`, each at its own
/// local start time and length.
fn spans_on_day(recording: &Recording, day: Day) -> AppResult<Vec<Span>> {
    let spans = closed_spans(recording)?;
    let offset = day.date() - Day::of(recording.start()).date();
    Ok(spans
        .into_iter()
        .map(|span| span_on_day(span, Day::from_date(Day::of(span.start).date() + offset)))
        .collect())
}

/// The span `span_id` of `recording` on `day`.
fn moved_span(recording: &Recording, span_id: &str, day: Day) -> AppResult<Span> {
    let span = recording.span(span_id)?;
    let end = span.end.ok_or_else(timer_running)?;
    Ok(span_on_day(
        Span {
            start: span.start,
            end,
        },
        day,
    ))
}

/// The undo label of an update: moved (same lengths), resized, or edited (same times).
fn update_label(before: &Recording, after: &Recording) -> &'static str {
    let times = |r: &Recording| r.spans.iter().map(|s| (s.start, s.end)).collect::<Vec<_>>();
    let lengths = |r: &Recording| {
        r.spans
            .iter()
            .map(|s| s.end.map(|end| end - s.start))
            .collect::<Vec<_>>()
    };
    if times(after) == times(before) {
        "Edit recording"
    } else if lengths(after) == lengths(before) {
        "Move recording"
    } else {
        "Resize recording"
    }
}

/// The recordings service: one write at a time, each validated, snapped, applied and made undoable.
pub struct Recordings<'a> {
    pub db: &'a Db,
    pub undo: &'a Mutex<UndoStack>,
    /// `Settings.timeline.snapMinutes`, read for each call.
    pub snap_minutes: u32,
}

impl Recordings<'_> {
    pub async fn create(
        &self,
        input: RecordingInput,
        overlap: Overlap,
    ) -> AppResult<Change<Recording>> {
        let mut undo = self.undo.lock().await;
        let (recording, project) = self.new_recording(input).await?;
        let label = format!(
            "Record {} for {}",
            duration_text(recording.duration(recording.start())),
            project.name
        );
        let base = Op::Create {
            recording: recording.clone(),
        };
        let op = clearing(self.db, base, &recording, overlap, &label).await?;
        self.commit(&mut undo, label, op, &recording.id).await
    }

    /// New recordings as one undoable change; one with a span that would overlap a recording, or one
    /// created before it, is left out.
    pub async fn create_all(
        &self,
        inputs: Vec<RecordingInput>,
        label: String,
    ) -> AppResult<Change<Vec<Recording>>> {
        let mut undo = self.undo.lock().await;
        let created = self.without_overlaps(inputs).await?;
        let days = if created.is_empty() {
            Vec::new()
        } else {
            let ops = created
                .iter()
                .map(|recording| Op::Create {
                    recording: recording.clone(),
                })
                .collect();
            let op = Op::Batch {
                label: label.clone(),
                ops,
            };
            self.apply_and_push(&mut undo, label, op).await?
        };
        Ok(Change {
            value: created,
            days,
        })
    }

    /// The inputs as new recordings, leaving out those that overlap a stored one or an earlier input.
    async fn without_overlaps(&self, inputs: Vec<RecordingInput>) -> AppResult<Vec<Recording>> {
        let mut kept: Vec<Recording> = Vec::new();
        for input in inputs {
            let (recording, _) = self.new_recording(input).await?;
            let spans = claimed(&recording);
            let overlaps_kept = kept
                .iter()
                .flat_map(claimed)
                .any(|k| spans.iter().any(|s| k.start < s.end && k.end > s.start));
            if overlaps_kept || !in_the_way(self.db, &spans, &recording.id).await?.is_empty() {
                continue;
            }
            kept.push(recording);
        }
        Ok(kept)
    }

    /// `input` validated and snapped, with its project; not stored yet.
    async fn new_recording(&self, input: RecordingInput) -> AppResult<(Recording, Project)> {
        let spans = new_spans(&snapped_spans(&input.spans, self.snap_minutes)?);
        recordings::check_spans(&spans)?;
        let note = checked_note(&input.note)?;
        let project = usable_project(self.db, &input.project_id, None).await?;
        check_tag(self.db, input.tag_id.as_deref()).await?;
        let now = now_ms();
        let recording = Recording {
            id: new_id(),
            project_id: project.id.clone(),
            note,
            tag_id: input.tag_id,
            created_at: now,
            updated_at: now,
            spans,
        };
        Ok((recording, project))
    }

    pub async fn update(
        &self,
        id: &str,
        patch: RecordingPatch,
        overlap: Overlap,
    ) -> AppResult<Change<Recording>> {
        let mut undo = self.undo.lock().await;
        let current = recordings::get(self.db, id).await?;
        let tag_changed = patch.tag_id.is_some();
        let after = patched(&current, patch, self.snap_minutes)?;
        usable_project(self.db, &after.project_id, Some(&current.project_id)).await?;
        if tag_changed {
            check_tag(self.db, after.tag_id.as_deref()).await?;
        }
        let label = update_label(&current, &after);
        self.replace(&mut undo, current, after, overlap, label)
            .await
    }

    /// Adds `span` to the recording.
    pub async fn add_span(
        &self,
        id: &str,
        span: Span,
        overlap: Overlap,
    ) -> AppResult<Change<Recording>> {
        let mut undo = self.undo.lock().await;
        let current = recordings::get(self.db, id).await?;
        require_finished(&current)?;
        let span = snapped_span(span, self.snap_minutes)?;
        let mut spans = current.spans.clone();
        spans.push(RecordingSpan::new(span.start, Some(span.end)));
        let after = with_spans(&current, spans)?;
        self.replace(&mut undo, current, after, overlap, "Add span")
            .await
    }

    /// Gives the span `span_id` the times of `span`.
    pub async fn update_span(
        &self,
        id: &str,
        span_id: &str,
        span: Span,
        overlap: Overlap,
    ) -> AppResult<Change<Recording>> {
        let mut undo = self.undo.lock().await;
        let current = recordings::get(self.db, id).await?;
        require_finished(&current)?;
        current.span(span_id)?;
        let span = snapped_span(span, self.snap_minutes)?;
        let spans = with_span_set(&current.spans, span_id, span.start, span.end);
        let after = with_spans(&current, spans)?;
        let label = update_label(&current, &after);
        self.replace(&mut undo, current, after, overlap, label)
            .await
    }

    /// Moves the start and the end of the span `span_id` by whole snap steps each.
    pub async fn nudge_span(
        &self,
        id: &str,
        span_id: &str,
        start_steps: i32,
        end_steps: i32,
        overlap: Overlap,
    ) -> AppResult<Change<Recording>> {
        let current = recordings::get(self.db, id).await?;
        let span = current.span(span_id)?;
        let end = span.end.ok_or_else(timer_running)?;
        let step = i64::from(self.snap_minutes.max(1)) * MINUTE_MS;
        let nudged = Span {
            start: span.start + i64::from(start_steps) * step,
            end: end + i64::from(end_steps) * step,
        };
        self.update_span(id, span_id, nudged, overlap).await
    }

    /// Removes the span `span_id`; removing the last one deletes the recording (then `None`).
    pub async fn remove_span(
        &self,
        id: &str,
        span_id: &str,
    ) -> AppResult<Change<Option<Recording>>> {
        let mut undo = self.undo.lock().await;
        let current = recordings::get(self.db, id).await?;
        require_finished(&current)?;
        current.span(span_id)?;
        if current.spans.len() == 1 {
            let op = Op::Delete { recording: current };
            let days = self
                .apply_and_push(&mut undo, "Delete recording".into(), op)
                .await?;
            return Ok(Change { value: None, days });
        }
        let spans = current
            .spans
            .iter()
            .filter(|s| s.id != span_id)
            .cloned()
            .collect();
        let after = with_spans(&current, spans)?;
        let op = Op::Update {
            before: current,
            after: after.clone(),
        };
        let change = self
            .commit(&mut undo, "Remove span".into(), op, &after.id)
            .await?;
        Ok(change.map(Some))
    }

    /// Cuts the span `span_id` in two at `at` (snapped); both halves stay spans of the recording.
    pub async fn split_span(
        &self,
        id: &str,
        span_id: &str,
        at: Ms,
    ) -> AppResult<Change<Recording>> {
        let mut undo = self.undo.lock().await;
        let current = recordings::get(self.db, id).await?;
        require_finished(&current)?;
        let span = current.span(span_id)?.clone();
        let at = snap(at, self.snap_minutes);
        if at <= span.start || span.end.is_none_or(|end| at >= end) {
            return Err(invalid("at", "must lie inside the span"));
        }
        let mut spans = with_span_set(&current.spans, span_id, span.start, at);
        spans.push(RecordingSpan::new(at, span.end));
        let after = with_spans(&current, spans)?;
        let op = Op::Update {
            before: current,
            after: after.clone(),
        };
        self.commit(&mut undo, "Split span".into(), op, &after.id)
            .await
    }

    /// The spans of `from` join `into`, which keeps its project, note and tag; `from` goes.
    pub async fn merge(&self, into: &str, from: &str) -> AppResult<Change<Recording>> {
        if into == from {
            return Err(invalid("from", "must be another recording"));
        }
        let mut undo = self.undo.lock().await;
        let target = recordings::get(self.db, into).await?;
        let source = recordings::get(self.db, from).await?;
        require_finished(&target)?;
        require_finished(&source)?;
        let mut spans = target.spans.clone();
        spans.extend(source.spans.iter().cloned());
        let after = with_spans(&target, spans)?;
        // The source goes first: its spans keep their ids in the target.
        let op = Op::Batch {
            label: "Merge recordings".into(),
            ops: vec![
                Op::Delete { recording: source },
                Op::Update {
                    before: target,
                    after: after.clone(),
                },
            ],
        };
        self.commit(&mut undo, "Merge recordings".into(), op, &after.id)
            .await
    }

    /// Moves the recording to `day` at the same local times, keeping its lengths: the span `span_id`
    /// alone, else every span by the days between its first span's day and `day`.
    pub async fn move_to_day(
        &self,
        id: &str,
        span_id: Option<&str>,
        day: Day,
        overlap: Overlap,
    ) -> AppResult<Change<Recording>> {
        let current = recordings::get(self.db, id).await?;
        match span_id {
            Some(span_id) => {
                let moved = moved_span(&current, span_id, day)?;
                self.update_span(id, span_id, moved, overlap).await
            }
            None => {
                let patch = RecordingPatch {
                    spans: Some(spans_on_day(&current, day)?),
                    ..RecordingPatch::default()
                };
                self.update(id, patch, overlap).await
            }
        }
    }

    /// A new recording like `id` on `day` at the same local times: of the span `span_id` alone, else of
    /// every span as `move_to_day` places them.
    pub async fn copy_to_day(
        &self,
        id: &str,
        span_id: Option<&str>,
        day: Day,
        overlap: Overlap,
    ) -> AppResult<Change<Recording>> {
        let original = recordings::get(self.db, id).await?;
        let spans = match span_id {
            Some(span_id) => vec![moved_span(&original, span_id, day)?],
            None => spans_on_day(&original, day)?,
        };
        let input = RecordingInput {
            spans,
            project_id: original.project_id,
            note: original.note,
            tag_id: original.tag_id,
        };
        self.create(input, overlap).await
    }

    pub async fn delete(&self, id: &str) -> AppResult<Change<()>> {
        let mut undo = self.undo.lock().await;
        let recording = recordings::get(self.db, id).await?;
        let days = self
            .apply_and_push(
                &mut undo,
                "Delete recording".into(),
                Op::Delete { recording },
            )
            .await?;
        Ok(Change { value: (), days })
    }

    /// The undone change's label; `None` when there is nothing to undo. A change that would now overlap
    /// another recording is refused and stays undoable.
    pub async fn undo(&self) -> AppResult<Change<Option<String>>> {
        let mut undo = self.undo.lock().await;
        let Some(entry) = undo.last_done().cloned() else {
            return Ok(Change {
                value: None,
                days: Vec::new(),
            });
        };
        let inverse = entry.op.inverse();
        ops::apply(self.db, &inverse).await?;
        undo.mark_undone();
        Ok(Change {
            value: Some(entry.label),
            days: days_touched(&inverse, now_ms()),
        })
    }

    pub async fn redo(&self) -> AppResult<Change<Option<String>>> {
        let mut undo = self.undo.lock().await;
        let Some(entry) = undo.last_undone().cloned() else {
            return Ok(Change {
                value: None,
                days: Vec::new(),
            });
        };
        ops::apply(self.db, &entry.op).await?;
        undo.mark_redone();
        Ok(Change {
            value: Some(entry.label),
            days: days_touched(&entry.op, now_ms()),
        })
    }

    /// Writes `after` over `before`, clearing its spans of other recordings per `overlap`.
    async fn replace(
        &self,
        undo: &mut UndoStack,
        before: Recording,
        after: Recording,
        overlap: Overlap,
        label: &str,
    ) -> AppResult<Change<Recording>> {
        let base = Op::Update {
            before,
            after: after.clone(),
        };
        let op = clearing(self.db, base, &after, overlap, label).await?;
        self.commit(undo, label.to_string(), op, &after.id).await
    }

    /// Applies a new change and makes it undoable; returns the days it touched.
    async fn apply_and_push(
        &self,
        undo: &mut UndoStack,
        label: String,
        op: Op,
    ) -> AppResult<Vec<Day>> {
        ops::apply(self.db, &op).await?;
        let days = days_touched(&op, now_ms());
        undo.push(Entry { label, op });
        Ok(days)
    }

    async fn commit(
        &self,
        undo: &mut UndoStack,
        label: String,
        op: Op,
        id: &str,
    ) -> AppResult<Change<Recording>> {
        let days = self.apply_and_push(undo, label, op).await?;
        Ok(Change {
            value: recordings::get(self.db, id).await?,
            days,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono_tz::Europe::Vienna;
    use serde_json::json;

    use super::testing::*;
    use super::undo::UndoStatus;
    use super::*;
    use crate::error::ErrorKind;
    use crate::test_support;

    fn vienna(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> Ms {
        Vienna
            .with_ymd_and_hms(year, month, day, hour, minute, second)
            .earliest()
            .unwrap()
            .timestamp_millis()
    }

    fn no_status() -> UndoStatus {
        UndoStatus {
            undo_label: None,
            redo_label: None,
        }
    }

    fn undo_label(label: &str) -> UndoStatus {
        UndoStatus {
            undo_label: Some(label.into()),
            redo_label: None,
        }
    }

    fn patch(f: impl FnOnce(&mut RecordingPatch)) -> RecordingPatch {
        let mut patch = RecordingPatch::default();
        f(&mut patch);
        patch
    }

    fn span(start: Ms, end: Ms) -> Span {
        Span { start, end }
    }

    fn respan(start: Ms, end: Ms) -> RecordingPatch {
        patch(|p| p.spans = Some(vec![span(start, end)]))
    }

    fn input_of(w: &World, spans: &[(Ms, Ms)]) -> RecordingInput {
        RecordingInput {
            spans: spans.iter().map(|&(s, e)| span(s, e)).collect(),
            ..w.input(0, 0)
        }
    }

    fn overlap_ids(err: &AppError) -> Vec<String> {
        let in_the_way: Vec<Recording> =
            serde_json::from_value(err.details().unwrap().clone()).unwrap();
        in_the_way.into_iter().map(|r| r.id).collect()
    }

    #[test]
    fn snapping_goes_to_the_nearest_multiple_from_local_midnight_and_ties_round_up() {
        let at = |h, m, s| vienna(2026, 6, 15, h, m, s);
        let cases = [
            (5, at(10, 0, 0), at(10, 0, 0)),
            (5, at(10, 2, 29), at(10, 0, 0)),
            (5, at(10, 2, 30), at(10, 5, 0)),
            (5, at(10, 7, 29), at(10, 5, 0)),
            (5, at(10, 7, 30), at(10, 10, 0)),
            (1, at(10, 2, 29), at(10, 2, 0)),
            (1, at(10, 2, 30), at(10, 3, 0)),
            (10, at(10, 4, 59), at(10, 0, 0)),
            (10, at(10, 5, 0), at(10, 10, 0)),
            (15, at(10, 7, 29), at(10, 0, 0)),
            (15, at(10, 7, 30), at(10, 15, 0)),
            (15, at(10, 52, 30), at(11, 0, 0)),
            (5, at(23, 58, 0), vienna(2026, 6, 16, 0, 0, 0)),
        ];
        for (minutes, input, expected) in cases {
            assert_eq!(
                snap_in(input, minutes, &Vienna),
                expected,
                "{minutes} min: {input}"
            );
        }
    }

    #[test]
    fn snapping_on_a_dst_day_lands_on_round_wall_clock_minutes() {
        let spring = |h, m, s| vienna(2026, 3, 29, h, m, s);
        let autumn = |h, m, s| vienna(2026, 10, 25, h, m, s);
        let cases = [
            (15, spring(0, 7, 30), spring(0, 15, 0)),
            (15, spring(1, 52, 0), spring(1, 45, 0)),
            (5, spring(3, 7, 0), spring(3, 5, 0)),
            (15, spring(3, 14, 0), spring(3, 15, 0)),
            (15, spring(4, 8, 0), spring(4, 15, 0)),
            (15, autumn(1, 52, 0), autumn(1, 45, 0)),
            (5, autumn(2, 7, 0), autumn(2, 5, 0)),
            (15, autumn(3, 8, 0), autumn(3, 15, 0)),
            (15, autumn(23, 52, 30), vienna(2026, 10, 26, 0, 0, 0)),
        ];
        for (minutes, input, expected) in cases {
            assert_eq!(
                snap_in(input, minutes, &Vienna),
                expected,
                "{minutes} min: {input}"
            );
        }
    }

    #[test]
    fn an_instant_in_the_repeated_autumn_hour_snaps_within_that_hour() {
        let second_pass = Vienna
            .with_ymd_and_hms(2026, 10, 25, 2, 7, 0)
            .latest()
            .unwrap()
            .timestamp_millis();

        let snapped = snap_in(second_pass, 5, &Vienna);

        assert_eq!(snapped, second_pass - 2 * MIN);
    }

    #[test]
    fn an_instant_before_the_spring_gap_snaps_to_a_round_minute_close_by() {
        let before_gap = vienna(2026, 3, 29, 1, 58, 0);

        let snapped = snap_in(before_gap, 5, &Vienna);

        let local = chrono::DateTime::from_timestamp_millis(snapped)
            .unwrap()
            .with_timezone(&Vienna);
        assert_eq!(local.minute() % 5, 0, "{local}");
        assert!((snapped - before_gap).abs() <= 5 * MIN, "{local}");
    }

    #[test]
    fn a_snapped_span_is_unchanged_and_an_end_that_snaps_onto_the_start_moves_one_step_on() {
        let cases = [
            (
                "already snapped",
                (at(9, 0), at(10, 30), 5),
                (at(9, 0), at(10, 30)),
            ),
            (
                "already snapped at 15",
                (at(9, 0), at(10, 30), 15),
                (at(9, 0), at(10, 30)),
            ),
            (
                "both ends round",
                (at(10, 3), at(10, 58), 5),
                (at(10, 5), at(11, 0)),
            ),
            (
                "end snaps onto the start",
                (at(10, 0), at(10, 1), 5),
                (at(10, 0), at(10, 5)),
            ),
            (
                "end equals start",
                (at(10, 0), at(10, 0), 5),
                (at(10, 0), at(10, 5)),
            ),
            (
                "end before start",
                (at(10, 0), at(9, 0), 5),
                (at(10, 0), at(10, 5)),
            ),
            (
                "one step is kept",
                (at(10, 2), at(10, 4), 5),
                (at(10, 0), at(10, 5)),
            ),
            (
                "end collapses at 15",
                (at(10, 0), at(10, 5), 15),
                (at(10, 0), at(10, 15)),
            ),
        ];
        for (name, (start, end, minutes), (expected_start, expected_end)) in cases {
            assert_eq!(
                snap_span(start, end, minutes),
                Span {
                    start: expected_start,
                    end: expected_end
                },
                "{name}"
            );
        }
    }

    #[test]
    fn default_span_is_the_half_hour_up_to_now_snapped_to_the_grid() {
        assert_eq!(
            default_span(at(13, 12), 5),
            Span {
                start: at(12, 40),
                end: at(13, 10)
            }
        );
    }

    #[test]
    fn default_span_ending_on_the_grid_is_exactly_the_half_hour_before() {
        assert_eq!(
            default_span(at(13, 15), 5),
            Span {
                start: at(12, 45),
                end: at(13, 15)
            }
        );
    }

    #[test]
    fn a_span_serializes_with_start_and_end() {
        assert_eq!(
            serde_json::to_value(Span { start: 1, end: 2 }).unwrap(),
            json!({"start": 1, "end": 2})
        );
    }

    #[test]
    fn overlap_modes_use_camel_case_on_the_wire() {
        assert_eq!(
            serde_json::to_value(Overlap::Replace).unwrap(),
            json!("replace")
        );
        assert_eq!(
            serde_json::to_value(Overlap::Cancel).unwrap(),
            json!("cancel")
        );
    }

    #[tokio::test]
    async fn create_trims_the_note_snaps_the_times_and_saves_the_tag() {
        let w = World::new().await;
        let input = RecordingInput {
            note: "  fix login  ".into(),
            tag_id: Some(w.alpha.id.clone()),
            ..w.input(at(10, 2), at(10, 58))
        };

        let saved = w
            .service()
            .create(input, Overlap::Cancel)
            .await
            .unwrap()
            .value;

        assert_eq!(
            saved,
            Recording {
                id: saved.id.clone(),
                project_id: w.project.id.clone(),
                note: "fix login".into(),
                tag_id: Some(w.alpha.id.clone()),
                created_at: saved.created_at,
                updated_at: saved.updated_at,
                spans: vec![RecordingSpan {
                    id: saved.spans[0].id.clone(),
                    start: at(10, 0),
                    end: Some(at(11, 0)),
                }],
            }
        );
        assert_eq!(saved.created_at, saved.updated_at);
        assert_eq!(w.rows().await, [saved]);
    }

    #[tokio::test]
    async fn create_uses_the_snap_minutes_it_was_given() {
        let w = World::new().await;

        let saved = w
            .snapping(15)
            .create(w.input(at(10, 7), at(10, 38)), Overlap::Cancel)
            .await
            .unwrap()
            .value;

        assert_eq!(times(&saved), [(at(10, 0), at(10, 45))]);
    }

    #[tokio::test]
    async fn a_span_shorter_than_one_step_is_saved_as_one_step() {
        let w = World::new().await;

        let saved = w.create(at(10, 0), at(10, 1)).await;

        assert_eq!(times(&saved), [(at(10, 0), at(10, 5))]);
    }

    #[tokio::test]
    async fn the_limits_of_a_note_and_a_span_are_inclusive() {
        let w = World::new().await;
        let input = RecordingInput {
            note: "x".repeat(500),
            ..w.input(at(0, 0), at(0, 0) + 24 * HOUR)
        };

        let saved = w
            .service()
            .create(input, Overlap::Cancel)
            .await
            .unwrap()
            .value;

        assert_eq!(saved.note.chars().count(), 500);
        assert_eq!(times(&saved), [(at(0, 0), at(0, 0) + 24 * HOUR)]);
    }

    #[tokio::test]
    async fn invalid_input_is_refused_naming_the_field_and_saves_nothing() {
        let w = World::new().await;
        projects::set_archived(&w.db, &w.other.id, true)
            .await
            .unwrap();
        let valid = w.input(at(10, 0), at(11, 0));
        let cases: Vec<(&str, RecordingInput, &str)> = vec![
            (
                "a note with a line break",
                RecordingInput {
                    note: "a\nb".into(),
                    ..valid.clone()
                },
                "note",
            ),
            (
                "a note with a carriage return",
                RecordingInput {
                    note: "a\rb".into(),
                    ..valid.clone()
                },
                "note",
            ),
            (
                "a note of 501 characters",
                RecordingInput {
                    note: "x".repeat(501),
                    ..valid.clone()
                },
                "note",
            ),
            (
                "an end before the start",
                input_of(&w, &[(at(10, 0), at(9, 0))]),
                "end",
            ),
            (
                "an end equal to the start",
                input_of(&w, &[(at(10, 0), at(10, 0))]),
                "end",
            ),
            (
                "a span longer than 24 h",
                input_of(&w, &[(at(10, 0), at(10, 0) + 24 * HOUR + 5 * MIN)]),
                "end",
            ),
            (
                "a good span next to a bad one",
                input_of(&w, &[(at(8, 0), at(9, 0)), (at(12, 0), at(11, 0))]),
                "end",
            ),
            ("no span", input_of(&w, &[]), "spans"),
            (
                "spans overlapping each other",
                input_of(&w, &[(at(10, 0), at(11, 0)), (at(10, 30), at(12, 0))]),
                "spans",
            ),
            (
                "spans that overlap once snapped",
                input_of(&w, &[(at(10, 0), at(10, 1)), (at(10, 2), at(11, 0))]),
                "spans",
            ),
            (
                "an archived project",
                RecordingInput {
                    project_id: w.other.id.clone(),
                    ..valid.clone()
                },
                "projectId",
            ),
            (
                "an unknown tag",
                RecordingInput {
                    tag_id: Some("nope".into()),
                    ..valid.clone()
                },
                "tagId",
            ),
        ];

        for (name, input, field) in cases {
            let err = w
                .service()
                .create(input, Overlap::Cancel)
                .await
                .unwrap_err();
            assert_eq!(field_of(&err).as_deref(), Some(field), "{name}");
        }

        assert!(w.rows().await.is_empty());
        assert_eq!(w.status().await, no_status());
    }

    #[tokio::test]
    async fn create_for_an_unknown_project_is_not_found() {
        let w = World::new().await;
        let input = RecordingInput {
            project_id: "nope".into(),
            ..w.input(at(10, 0), at(11, 0))
        };

        let err = w
            .service()
            .create(input, Overlap::Cancel)
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(w.rows().await.is_empty());
    }

    #[tokio::test]
    async fn create_is_labelled_with_the_duration_and_the_project() {
        for (minutes, label) in [
            (45, "Record 45 min for Acme"),
            (60, "Record 1 h 00 min for Acme"),
            (65, "Record 1 h 05 min for Acme"),
            (120, "Record 2 h 00 min for Acme"),
        ] {
            let w = World::new().await;

            w.create(at(8, 0), at(8, 0) + minutes * MIN).await;

            assert_eq!(w.status().await, undo_label(label), "{minutes} min");
        }
    }

    #[tokio::test]
    async fn create_with_several_spans_snaps_and_sorts_them_into_one_recording() {
        let w = World::new().await;
        let input = input_of(&w, &[(at(16, 7), at(20, 13)), (at(14, 23), at(15, 2))]);

        let saved = w
            .service()
            .create(input, Overlap::Cancel)
            .await
            .unwrap()
            .value;

        assert_eq!(
            times(&saved),
            [(at(14, 25), at(15, 0)), (at(16, 5), at(20, 15))]
        );
        assert_eq!(w.rows().await, [saved]);
    }

    #[tokio::test]
    async fn spans_that_only_touch_each_other_are_accepted() {
        let w = World::new().await;

        let saved = w
            .create_spans(&[(at(10, 0), at(11, 0)), (at(11, 0), at(12, 0))])
            .await;

        assert_eq!(
            times(&saved),
            [(at(10, 0), at(11, 0)), (at(11, 0), at(12, 0))]
        );
    }

    #[tokio::test]
    async fn the_label_of_a_recording_with_several_spans_sums_them() {
        let w = World::new().await;

        w.create_spans(&[(at(14, 25), at(15, 0)), (at(16, 5), at(20, 15))])
            .await;

        assert_eq!(w.status().await, undo_label("Record 4 h 45 min for Acme"));
    }

    #[tokio::test]
    async fn update_labels_name_what_changed() {
        let cases = [
            (
                "both ends shifted",
                respan(at(10, 30), at(11, 30)),
                "Move recording",
            ),
            (
                "end changed",
                respan(at(10, 0), at(11, 30)),
                "Resize recording",
            ),
            (
                "start changed",
                respan(at(9, 30), at(11, 0)),
                "Resize recording",
            ),
            (
                "note changed",
                patch(|p| p.note = Some("new".into())),
                "Edit recording",
            ),
            (
                "project changed",
                patch(|p| p.project_id = Some("other".into())),
                "Edit recording",
            ),
            ("same times", respan(at(10, 0), at(11, 0)), "Edit recording"),
            (
                "a span added",
                patch(|p| {
                    p.spans = Some(vec![span(at(10, 0), at(11, 0)), span(at(12, 0), at(13, 0))]);
                }),
                "Resize recording",
            ),
        ];
        for (name, mut patch, label) in cases {
            let w = World::new().await;
            let saved = w.create(at(10, 0), at(11, 0)).await;
            if patch.project_id.is_some() {
                patch.project_id = Some(w.other.id.clone());
            }

            w.service()
                .update(&saved.id, patch, Overlap::Cancel)
                .await
                .unwrap();

            let status = w.status().await;
            assert_eq!(status.undo_label.as_deref(), Some(label), "{name}");
        }
    }

    #[tokio::test]
    async fn update_snaps_the_new_times_and_returns_the_saved_recording() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;
        let change = respan(at(10, 32), at(11, 27));

        let updated = w
            .service()
            .update(&saved.id, change, Overlap::Cancel)
            .await
            .unwrap()
            .value;

        assert_eq!(
            updated,
            Recording {
                updated_at: updated.updated_at,
                spans: vec![RecordingSpan {
                    id: saved.spans[0].id.clone(),
                    start: at(10, 30),
                    end: Some(at(11, 25)),
                }],
                ..saved
            }
        );
        assert_eq!(w.rows().await, [updated]);
    }

    #[tokio::test]
    async fn editing_only_the_note_keeps_unsnapped_times_as_they_were() {
        let w = World::new().await;
        let stored = test_support::recording(
            &w.db,
            &w.project.id,
            at(10, 2) + 17_000,
            Some(at(10, 58) + 17_000),
        )
        .await;

        let updated = w
            .service()
            .update(
                &stored.id,
                patch(|p| p.note = Some(" later ".into())),
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;

        assert_eq!(updated.spans, stored.spans);
        assert_eq!(updated.note, "later");
        assert_eq!(w.status().await, undo_label("Edit recording"));
    }

    #[tokio::test]
    async fn update_replaces_clears_and_keeps_the_tag_depending_on_the_patch() {
        let w = World::new().await;
        let input = RecordingInput {
            tag_id: Some(w.alpha.id.clone()),
            ..w.input(at(10, 0), at(11, 0))
        };
        let saved = w
            .service()
            .create(input, Overlap::Cancel)
            .await
            .unwrap()
            .value;

        let kept = w
            .service()
            .update(
                &saved.id,
                patch(|p| p.note = Some("n".into())),
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;
        let replaced = w
            .service()
            .update(
                &saved.id,
                patch(|p| p.tag_id = Some(Some(w.beta.id.clone()))),
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;
        let cleared = w
            .service()
            .update(&saved.id, patch(|p| p.tag_id = Some(None)), Overlap::Cancel)
            .await
            .unwrap()
            .value;

        assert_eq!(kept.tag_id, Some(w.alpha.id.clone()));
        assert_eq!(replaced.tag_id, Some(w.beta.id.clone()));
        assert_eq!(cleared.tag_id, None);
        assert_eq!(w.rows().await, [cleared]);
    }

    #[tokio::test]
    async fn an_unknown_tag_is_refused_on_the_tag_id_field() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;
        let created = w
            .service()
            .create(
                RecordingInput {
                    tag_id: Some("nope".into()),
                    ..w.input(at(12, 0), at(13, 0))
                },
                Overlap::Cancel,
            )
            .await
            .unwrap_err();
        let updated = w
            .service()
            .update(
                &saved.id,
                patch(|p| p.tag_id = Some(Some("nope".into()))),
                Overlap::Cancel,
            )
            .await
            .unwrap_err();

        for err in [created, updated] {
            let field = err.field().unwrap();
            assert_eq!((field.path, field.message), ("tagId", "is an unknown tag"));
        }
        assert_eq!(w.rows().await, [saved]);
    }

    #[tokio::test]
    async fn a_recording_of_an_archived_project_can_still_be_edited() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;
        projects::set_archived(&w.db, &w.project.id, true)
            .await
            .unwrap();

        let updated = w
            .service()
            .update(
                &saved.id,
                patch(|p| p.note = Some("n".into())),
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;

        assert_eq!(updated.note, "n");
        assert_eq!(updated.project_id, w.project.id);
    }

    #[tokio::test]
    async fn update_refuses_invalid_patches_naming_the_field_and_changes_nothing() {
        let w = World::new().await;
        projects::set_archived(&w.db, &w.other.id, true)
            .await
            .unwrap();
        let saved = w.create(at(10, 0), at(11, 0)).await;
        let cases = [
            (
                "an end before the start",
                respan(at(12, 0), at(11, 0)),
                "end",
            ),
            ("no span", patch(|p| p.spans = Some(Vec::new())), "spans"),
            (
                "spans overlapping each other",
                patch(|p| {
                    p.spans = Some(vec![span(at(8, 0), at(9, 30)), span(at(9, 0), at(10, 0))]);
                }),
                "spans",
            ),
            (
                "a note with a line break",
                patch(|p| p.note = Some("a\nb".into())),
                "note",
            ),
            (
                "an archived project",
                patch(|p| p.project_id = Some(w.other.id.clone())),
                "projectId",
            ),
            (
                "an unknown tag",
                patch(|p| p.tag_id = Some(Some("nope".into()))),
                "tagId",
            ),
            (
                "a span longer than 24 h",
                respan(at(10, 0), at(10, 0) + 25 * HOUR),
                "end",
            ),
        ];

        for (name, change, field) in cases {
            let err = w
                .service()
                .update(&saved.id, change, Overlap::Cancel)
                .await
                .unwrap_err();
            assert_eq!(field_of(&err).as_deref(), Some(field), "{name}");
        }

        assert_eq!(w.rows().await, [saved]);
        assert_eq!(w.status().await, undo_label("Record 1 h 00 min for Acme"));
    }

    #[tokio::test]
    async fn update_of_an_unknown_recording_is_not_found() {
        let w = World::new().await;

        let err = w
            .service()
            .update(
                "nope",
                patch(|p| p.note = Some("n".into())),
                Overlap::Cancel,
            )
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn the_running_recording_cannot_be_edited_here() {
        let w = World::new().await;
        let timer = test_support::recording(&w.db, &w.project.id, at(10, 0), None).await;

        let err = w
            .service()
            .update(
                &timer.id,
                patch(|p| p.note = Some("n".into())),
                Overlap::Cancel,
            )
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(w.rows().await, [timer]);
    }

    #[tokio::test]
    async fn delete_removes_the_recording_and_is_labelled() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;

        w.service().delete(&saved.id).await.unwrap();

        assert!(w.rows().await.is_empty());
        assert_eq!(
            w.status().await.undo_label.as_deref(),
            Some("Delete recording")
        );
    }

    #[tokio::test]
    async fn delete_of_an_unknown_recording_is_not_found_and_not_undoable() {
        let w = World::new().await;

        let err = w.service().delete("nope").await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(w.status().await, no_status());
    }

    #[tokio::test]
    async fn cancel_refuses_an_overlap_listing_the_recordings_in_the_way() {
        let w = World::new().await;
        let first = w.create(at(10, 0), at(11, 0)).await;
        let second = w.create(at(11, 0), at(12, 0)).await;

        let err = w
            .service()
            .create(w.input(at(10, 30), at(11, 30)), Overlap::Cancel)
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::Overlap);
        let in_the_way: Vec<Recording> =
            serde_json::from_value(err.details().unwrap().clone()).unwrap();
        assert_eq!(in_the_way, [first.clone(), second.clone()]);
        assert_eq!(w.rows().await, [first, second]);
    }

    #[tokio::test]
    async fn cancel_lists_a_neighbour_with_several_spans_in_the_way_once() {
        let w = World::new().await;
        let neighbour = w
            .create_spans(&[(at(10, 0), at(11, 0)), (at(12, 0), at(13, 0))])
            .await;

        let err = w
            .service()
            .create(
                input_of(&w, &[(at(10, 30), at(10, 45)), (at(12, 15), at(12, 30))]),
                Overlap::Cancel,
            )
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::Overlap);
        assert_eq!(overlap_ids(&err), [neighbour.id.clone()]);
        assert_eq!(w.rows().await, [neighbour]);
    }

    #[tokio::test]
    async fn a_span_in_the_gap_between_a_neighbours_spans_is_not_in_the_way() {
        let w = World::new().await;
        let neighbour = w
            .create_spans(&[(at(10, 0), at(11, 0)), (at(13, 0), at(14, 0))])
            .await;

        let saved = w.create(at(11, 0), at(13, 0)).await;

        assert_eq!(w.rows().await, [neighbour, saved]);
    }

    #[tokio::test]
    async fn recordings_that_only_touch_do_not_overlap() {
        let w = World::new().await;
        w.create(at(10, 0), at(11, 0)).await;
        w.create(at(12, 0), at(13, 0)).await;

        w.create(at(11, 0), at(12, 0)).await;

        assert_eq!(
            spans(&w.rows().await),
            [
                (at(10, 0), at(11, 0)),
                (at(11, 0), at(12, 0)),
                (at(12, 0), at(13, 0))
            ]
        );
    }

    #[tokio::test]
    async fn replace_trims_splits_and_deletes_what_is_in_the_way() {
        type Spans = Vec<(Ms, Ms)>;
        let cases: Vec<(&str, Spans, (Ms, Ms), Spans)> = vec![
            (
                "trims the one sticking out on the left",
                vec![(at(10, 0), at(11, 0))],
                (at(10, 30), at(11, 30)),
                vec![(at(10, 0), at(10, 30)), (at(10, 30), at(11, 30))],
            ),
            (
                "trims the one sticking out on the right",
                vec![(at(10, 0), at(11, 0))],
                (at(9, 30), at(10, 30)),
                vec![(at(9, 30), at(10, 30)), (at(10, 30), at(11, 0))],
            ),
            (
                "deletes the one inside",
                vec![(at(10, 0), at(11, 0))],
                (at(9, 0), at(12, 0)),
                vec![(at(9, 0), at(12, 0))],
            ),
            (
                "deletes the one with the same span",
                vec![(at(10, 0), at(11, 0))],
                (at(10, 0), at(11, 0)),
                vec![(at(10, 0), at(11, 0))],
            ),
            (
                "splits the one around it",
                vec![(at(10, 0), at(11, 0))],
                (at(10, 20), at(10, 40)),
                vec![
                    (at(10, 0), at(10, 20)),
                    (at(10, 20), at(10, 40)),
                    (at(10, 40), at(11, 0)),
                ],
            ),
            (
                "handles several at once",
                vec![
                    (at(10, 0), at(11, 0)),
                    (at(11, 0), at(12, 0)),
                    (at(12, 0), at(13, 0)),
                ],
                (at(10, 30), at(12, 30)),
                vec![
                    (at(10, 0), at(10, 30)),
                    (at(10, 30), at(12, 30)),
                    (at(12, 30), at(13, 0)),
                ],
            ),
            (
                "leaves touching neighbours alone",
                vec![(at(9, 0), at(10, 0)), (at(11, 0), at(12, 0))],
                (at(10, 0), at(11, 0)),
                vec![
                    (at(9, 0), at(10, 0)),
                    (at(10, 0), at(11, 0)),
                    (at(11, 0), at(12, 0)),
                ],
            ),
        ];

        for (name, existing, (start, end), expected) in cases {
            let w = World::new().await;
            for (s, e) in existing {
                w.create(s, e).await;
            }

            w.service()
                .create(w.input(start, end), Overlap::Replace)
                .await
                .unwrap();

            let rows = w.rows().await;
            assert_eq!(spans(&rows), expected, "{name}");
            assert!(no_two_overlap(&rows), "{name}");
        }
    }

    #[tokio::test]
    async fn cutting_a_span_sticking_out_on_both_sides_leaves_two_spans_of_the_same_recording() {
        let w = World::new().await;
        let original = w
            .service()
            .create(
                RecordingInput {
                    project_id: w.other.id.clone(),
                    note: "keep".into(),
                    tag_id: Some(w.alpha.id.clone()),
                    ..w.input(at(10, 0), at(11, 0))
                },
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;

        w.service()
            .create(w.input(at(10, 20), at(10, 40)), Overlap::Replace)
            .await
            .unwrap();

        let rows = w.rows().await;
        assert_eq!(rows.len(), 2);
        let (cut, new) = (&rows[0], &rows[1]);
        assert_eq!(cut.id, original.id);
        assert_eq!(
            *cut,
            Recording {
                updated_at: cut.updated_at,
                spans: vec![
                    RecordingSpan {
                        id: original.spans[0].id.clone(),
                        start: at(10, 0),
                        end: Some(at(10, 20)),
                    },
                    RecordingSpan {
                        id: cut.spans[1].id.clone(),
                        start: at(10, 40),
                        end: Some(at(11, 0)),
                    },
                ],
                ..original.clone()
            }
        );
        assert_ne!(cut.spans[1].id, original.spans[0].id);
        assert_eq!(new.project_id, w.project.id);
        assert_eq!(times(new), [(at(10, 20), at(10, 40))]);
    }

    #[tokio::test]
    async fn replace_deletes_inside_spans_trims_edge_spans_and_cuts_a_span_in_two() {
        let w = World::new().await;
        let one = w
            .create_spans(&[(at(8, 0), at(9, 0)), (at(10, 0), at(11, 0))])
            .await;
        let two = w
            .create_spans(&[(at(12, 0), at(13, 0)), (at(14, 0), at(15, 0))])
            .await;

        w.service()
            .create(
                input_of(&w, &[(at(10, 30), at(10, 45)), (at(12, 30), at(16, 0))]),
                Overlap::Replace,
            )
            .await
            .unwrap();

        let rows = w.rows().await;
        assert_eq!(rows.len(), 3);
        let one_after = rows.iter().find(|r| r.id == one.id).unwrap();
        let two_after = rows.iter().find(|r| r.id == two.id).unwrap();
        assert_eq!(
            times(one_after),
            [
                (at(8, 0), at(9, 0)),
                (at(10, 0), at(10, 30)),
                (at(10, 45), at(11, 0))
            ]
        );
        assert_eq!(times(two_after), [(at(12, 0), at(12, 30))]);
        assert_eq!(one_after.spans[0].id, one.spans[0].id);
        assert_eq!(one_after.spans[1].id, one.spans[1].id);
        assert!(no_two_overlap(&rows));
    }

    #[tokio::test]
    async fn replace_deletes_a_recording_left_without_spans() {
        let w = World::new().await;
        w.create_spans(&[(at(10, 0), at(10, 30)), (at(11, 0), at(11, 30))])
            .await;

        w.service()
            .create(w.input(at(9, 0), at(12, 0)), Overlap::Replace)
            .await
            .unwrap();

        assert_eq!(spans(&w.rows().await), [(at(9, 0), at(12, 0))]);
        assert_eq!(w.rows().await.len(), 1);
    }

    #[tokio::test]
    async fn undoing_and_redoing_a_replace_restores_the_exact_span_ids() {
        let w = World::new().await;
        let one = w
            .create_spans(&[(at(10, 0), at(11, 0)), (at(12, 0), at(13, 0))])
            .await;
        let two = w.create(at(14, 0), at(15, 0)).await;
        let before = w.rows().await;
        w.service()
            .create(
                input_of(&w, &[(at(10, 20), at(10, 40)), (at(12, 0), at(15, 0))]),
                Overlap::Replace,
            )
            .await
            .unwrap();
        let replaced = w.rows().await;
        assert!(replaced.iter().all(|r| r.id != two.id));

        w.service().undo().await.unwrap();

        let restored = w.rows().await;
        assert_eq!(restored, before);
        assert_eq!(
            span_ids(restored.iter().find(|r| r.id == one.id).unwrap()),
            span_ids(&one)
        );

        w.service().redo().await.unwrap();

        assert_eq!(w.rows().await, replaced);
    }

    #[tokio::test]
    async fn a_replace_is_one_undo_step_that_restores_the_trimmed_recordings_exactly() {
        let w = World::new().await;
        w.service()
            .create(
                RecordingInput {
                    note: "keep".into(),
                    tag_id: Some(w.alpha.id.clone()),
                    ..w.input(at(10, 0), at(11, 0))
                },
                Overlap::Cancel,
            )
            .await
            .unwrap();
        w.create(at(11, 0), at(12, 0)).await;
        let before = w.rows().await;
        w.service()
            .create(w.input(at(10, 20), at(11, 30)), Overlap::Replace)
            .await
            .unwrap();
        let replaced = w.rows().await;

        let undone = w.service().undo().await.unwrap();
        let after_undo = w.rows().await;
        let redone = w.service().redo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Record 1 h 10 min for Acme"));
        assert_eq!(after_undo, before);
        assert_eq!(redone.value.as_deref(), Some("Record 1 h 10 min for Acme"));
        assert_eq!(w.rows().await, replaced);
    }

    #[tokio::test]
    async fn a_running_recording_in_the_way_cannot_be_replaced() {
        let w = World::new().await;
        let timer = test_support::recording(&w.db, &w.project.id, at(10, 0), None).await;

        let replace = w
            .service()
            .create(w.input(at(9, 0), at(10, 30)), Overlap::Replace)
            .await
            .unwrap_err();
        let cancel = w
            .service()
            .create(w.input(at(9, 0), at(10, 30)), Overlap::Cancel)
            .await
            .unwrap_err();

        assert_eq!(replace.kind(), ErrorKind::InvalidInput);
        assert_eq!(cancel.kind(), ErrorKind::Overlap);
        assert_eq!(w.rows().await, [timer]);
        assert_eq!(w.status().await, no_status());
    }

    #[tokio::test]
    async fn a_recording_before_the_running_one_does_not_overlap_it() {
        let w = World::new().await;
        test_support::recording(&w.db, &w.project.id, at(12, 0), None).await;

        let saved = w.create(at(10, 0), at(11, 0)).await;

        assert_eq!(times(&saved), [(at(10, 0), at(11, 0))]);
        assert_eq!(w.rows().await.len(), 2);
    }

    #[tokio::test]
    async fn update_into_another_recording_follows_the_overlap_mode() {
        let w = World::new().await;
        let first = w.create(at(10, 0), at(11, 0)).await;
        let second = w.create(at(12, 0), at(13, 0)).await;
        let earlier = || respan(at(10, 30), at(13, 0));

        let err = w
            .service()
            .update(&second.id, earlier(), Overlap::Cancel)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Overlap);
        assert_eq!(w.rows().await, [first, second.clone()]);

        w.service()
            .update(&second.id, earlier(), Overlap::Replace)
            .await
            .unwrap();
        let rows = w.rows().await;
        assert_eq!(
            spans(&rows),
            [(at(10, 0), at(10, 30)), (at(10, 30), at(13, 0))]
        );
        assert!(no_two_overlap(&rows));
    }

    #[tokio::test]
    async fn a_recording_can_grow_and_move_within_its_own_span() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;

        let updated = w
            .service()
            .update(&saved.id, respan(at(10, 0), at(11, 30)), Overlap::Cancel)
            .await
            .unwrap()
            .value;

        assert_eq!(times(&updated), [(at(10, 0), at(11, 30))]);
        assert_eq!(w.rows().await, [updated]);
    }

    #[tokio::test]
    async fn undo_with_nothing_to_undo_changes_nothing() {
        let w = World::new().await;

        let undone = w.service().undo().await.unwrap();
        let redone = w.service().redo().await.unwrap();

        assert_eq!((undone.value, undone.days), (None, vec![]));
        assert_eq!((redone.value, redone.days), (None, vec![]));
    }

    #[tokio::test]
    async fn undo_then_redo_restores_a_creation() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;

        let undone = w.service().undo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Record 1 h 00 min for Acme"));
        assert!(w.rows().await.is_empty());
        assert_eq!(
            w.status().await,
            UndoStatus {
                undo_label: None,
                redo_label: Some("Record 1 h 00 min for Acme".into())
            }
        );

        let redone = w.service().redo().await.unwrap();

        assert_eq!(redone.value.as_deref(), Some("Record 1 h 00 min for Acme"));
        assert_eq!(w.rows().await, [saved]);
        assert_eq!(w.status().await, undo_label("Record 1 h 00 min for Acme"));
    }

    #[tokio::test]
    async fn undo_restores_an_edited_recording_exactly() {
        let w = World::new().await;
        let saved = w
            .service()
            .create(
                RecordingInput {
                    note: "before".into(),
                    tag_id: Some(w.alpha.id.clone()),
                    ..w.input(at(10, 0), at(11, 0))
                },
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;
        let change = patch(|p| {
            p.note = Some("after".into());
            p.spans = Some(vec![span(at(10, 30), at(11, 0))]);
            p.tag_id = Some(Some(w.beta.id.clone()));
            p.project_id = Some(w.other.id.clone());
        });
        w.service()
            .update(&saved.id, change, Overlap::Cancel)
            .await
            .unwrap();

        w.service().undo().await.unwrap();

        assert_eq!(w.rows().await, [saved]);
    }

    #[tokio::test]
    async fn undo_restores_a_deleted_recording_with_its_tag_and_timestamps() {
        let w = World::new().await;
        let saved = w
            .service()
            .create(
                RecordingInput {
                    note: "n".into(),
                    tag_id: Some(w.alpha.id.clone()),
                    ..w.input(at(10, 0), at(11, 0))
                },
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;
        w.service().delete(&saved.id).await.unwrap();

        let undone = w.service().undo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Delete recording"));
        assert_eq!(w.rows().await, [saved.clone()]);
        assert_eq!(saved.tag_id, Some(w.alpha.id.clone()));
    }

    #[tokio::test]
    async fn a_new_change_clears_what_could_be_redone() {
        let w = World::new().await;
        w.create(at(10, 0), at(11, 0)).await;
        w.service().undo().await.unwrap();
        assert!(w.status().await.redo_label.is_some());

        w.create(at(12, 0), at(13, 0)).await;

        assert_eq!(w.status().await, undo_label("Record 1 h 00 min for Acme"));
        assert_eq!(w.service().redo().await.unwrap().value, None);
    }

    #[tokio::test]
    async fn an_undo_that_would_overlap_a_newer_recording_is_refused_and_stays_undoable() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;
        w.service()
            .update(&saved.id, respan(at(12, 0), at(13, 0)), Overlap::Cancel)
            .await
            .unwrap();
        let newer =
            test_support::recording(&w.db, &w.project.id, at(10, 30), Some(at(11, 30))).await;
        let rows = w.rows().await;
        let status = w.status().await;
        assert_eq!(status, undo_label("Move recording"));

        let err = w.service().undo().await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::Overlap);
        let in_the_way: Vec<Recording> =
            serde_json::from_value(err.details().unwrap().clone()).unwrap();
        assert_eq!(in_the_way, [newer.clone()]);
        assert_eq!(w.rows().await, rows);
        assert_eq!(w.status().await, status);

        recordings::delete(&w.db, &newer.id).await.unwrap();
        w.service().undo().await.unwrap();
        assert_eq!(spans(&w.rows().await), [(at(10, 0), at(11, 0))]);
    }

    #[tokio::test]
    async fn changes_report_the_day_they_happen_on() {
        let w = World::new().await;
        let day = Day::of(at(12, 0));

        let created = w
            .service()
            .create(w.input(at(12, 0), at(12, 30)), Overlap::Cancel)
            .await
            .unwrap();

        assert_eq!(created.days, [day]);
    }

    #[tokio::test]
    async fn a_recording_across_midnight_touches_both_days() {
        let w = World::new().await;
        let (before, midnight, after) = midnight();

        let created = w
            .service()
            .create(w.input(midnight - HOUR, midnight + HOUR), Overlap::Cancel)
            .await
            .unwrap();
        let id = created.value.id.clone();
        let deleted = w.service().delete(&id).await.unwrap();
        let undone = w.service().undo().await.unwrap();
        let redone = w.service().redo().await.unwrap();

        assert_eq!(created.days, [before, after]);
        assert_eq!(deleted.days, [before, after]);
        assert_eq!(undone.days, [before, after]);
        assert_eq!(redone.days, [before, after]);
    }

    #[tokio::test]
    async fn a_move_from_one_day_to_another_reports_both_days() {
        let w = World::new().await;
        let (before, midnight, after) = midnight();
        let saved = w.create(midnight - 5 * HOUR, midnight - 4 * HOUR).await;

        let moved = w
            .service()
            .update(
                &saved.id,
                respan(midnight + HOUR, midnight + 2 * HOUR),
                Overlap::Cancel,
            )
            .await
            .unwrap();

        assert_eq!(moved.days, [before, after]);
    }

    #[tokio::test]
    async fn a_move_within_a_day_reports_that_day_once() {
        let w = World::new().await;
        let (before, midnight, _) = midnight();
        let saved = w.create(midnight - 5 * HOUR, midnight - 4 * HOUR).await;

        let moved = w
            .service()
            .update(
                &saved.id,
                respan(midnight - 3 * HOUR, midnight - 2 * HOUR),
                Overlap::Cancel,
            )
            .await
            .unwrap();

        assert_eq!(moved.days, [before]);
    }

    const NEXT_DAY: Ms = 24 * HOUR;

    fn next_day() -> Day {
        Day::of(at(12, 0)).next()
    }

    #[tokio::test]
    async fn move_to_day_keeps_the_local_time_and_length_and_reports_both_days() {
        let w = World::new().await;
        let saved = w.create(at(12, 0), at(13, 30)).await;

        let moved = w
            .service()
            .move_to_day(&saved.id, None, next_day(), Overlap::Cancel)
            .await
            .unwrap();

        assert_eq!(moved.value.id, saved.id);
        assert_eq!(
            times(&moved.value),
            [(at(12, 0) + NEXT_DAY, at(13, 30) + NEXT_DAY)]
        );
        assert_eq!(moved.days, [Day::of(at(12, 0)), next_day()]);
        assert_eq!(w.rows().await, [moved.value]);
        assert_eq!(w.status().await, undo_label("Move recording"));
    }

    #[tokio::test]
    async fn move_to_day_keeps_note_project_and_tag() {
        let w = World::new().await;
        let saved = w
            .service()
            .create(
                RecordingInput {
                    note: "keep me".into(),
                    tag_id: Some(w.alpha.id.clone()),
                    ..w.input(at(12, 0), at(13, 0))
                },
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;

        let moved = w
            .service()
            .move_to_day(&saved.id, None, next_day(), Overlap::Cancel)
            .await
            .unwrap()
            .value;

        assert_eq!(
            (moved.project_id, moved.note, moved.tag_id),
            (saved.project_id, saved.note, saved.tag_id)
        );
    }

    #[tokio::test]
    async fn undoing_a_move_to_another_day_restores_the_original_day() {
        let w = World::new().await;
        let saved = w.create(at(12, 0), at(13, 0)).await;
        w.service()
            .move_to_day(&saved.id, None, next_day(), Overlap::Cancel)
            .await
            .unwrap();

        let undone = w.service().undo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Move recording"));
        assert_eq!(w.rows().await, [saved]);
    }

    #[tokio::test]
    async fn move_to_day_follows_the_overlap_mode_on_the_target_day() {
        let w = World::new().await;
        let saved = w.create(at(9, 0), at(10, 0)).await;
        let blocker = test_support::recording(
            &w.db,
            &w.project.id,
            at(9, 30) + NEXT_DAY,
            Some(at(10, 30) + NEXT_DAY),
        )
        .await;

        let err = w
            .service()
            .move_to_day(&saved.id, None, next_day(), Overlap::Cancel)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Overlap);
        assert_eq!(w.rows().await, [saved.clone(), blocker]);

        w.service()
            .move_to_day(&saved.id, None, next_day(), Overlap::Replace)
            .await
            .unwrap();
        assert_eq!(
            spans(&w.rows().await),
            [
                (at(9, 0) + NEXT_DAY, at(10, 0) + NEXT_DAY),
                (at(10, 0) + NEXT_DAY, at(10, 30) + NEXT_DAY)
            ]
        );
    }

    #[tokio::test]
    async fn move_to_day_of_a_running_recording_asks_to_stop_the_timer() {
        let w = World::new().await;
        let timer = test_support::recording(&w.db, &w.project.id, at(10, 0), None).await;

        let err = w
            .service()
            .move_to_day(&timer.id, None, next_day(), Overlap::Cancel)
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(w.rows().await, [timer]);
        assert_eq!(w.status().await, no_status());
    }

    #[tokio::test]
    async fn move_to_day_of_an_unknown_recording_is_not_found() {
        let w = World::new().await;

        let err = w
            .service()
            .move_to_day("nope", None, next_day(), Overlap::Cancel)
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn copy_to_day_creates_a_new_recording_and_keeps_the_original() {
        let w = World::new().await;
        let original = w
            .service()
            .create(
                RecordingInput {
                    note: "copy me".into(),
                    tag_id: Some(w.alpha.id.clone()),
                    ..w.input(at(12, 0), at(13, 30))
                },
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;

        let copied = w
            .service()
            .copy_to_day(&original.id, None, next_day(), Overlap::Cancel)
            .await
            .unwrap();

        let copy = &copied.value;
        assert_ne!(copy.id, original.id);
        assert_eq!(times(copy), [(at(12, 0) + NEXT_DAY, at(13, 30) + NEXT_DAY)]);
        assert_eq!(
            (&copy.project_id, &copy.note, &copy.tag_id),
            (&original.project_id, &original.note, &original.tag_id)
        );
        assert_eq!(copied.days, [next_day()]);
        assert_eq!(w.rows().await, [original, copy.clone()]);
        assert_eq!(w.status().await, undo_label("Record 1 h 30 min for Acme"));
    }

    #[tokio::test]
    async fn undoing_a_copy_removes_only_the_copy() {
        let w = World::new().await;
        let original = w.create(at(12, 0), at(13, 0)).await;
        w.service()
            .copy_to_day(&original.id, None, next_day(), Overlap::Cancel)
            .await
            .unwrap();

        w.service().undo().await.unwrap();

        assert_eq!(w.rows().await, [original]);
    }

    #[tokio::test]
    async fn copy_to_day_follows_the_overlap_mode_on_the_target_day() {
        let w = World::new().await;
        let original = w.create(at(9, 0), at(10, 0)).await;
        let blocker = test_support::recording(
            &w.db,
            &w.project.id,
            at(9, 30) + NEXT_DAY,
            Some(at(10, 30) + NEXT_DAY),
        )
        .await;

        let err = w
            .service()
            .copy_to_day(&original.id, None, next_day(), Overlap::Cancel)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Overlap);
        assert_eq!(w.rows().await, [original.clone(), blocker]);

        w.service()
            .copy_to_day(&original.id, None, next_day(), Overlap::Replace)
            .await
            .unwrap();
        assert_eq!(
            spans(&w.rows().await),
            [
                (at(9, 0), at(10, 0)),
                (at(9, 0) + NEXT_DAY, at(10, 0) + NEXT_DAY),
                (at(10, 0) + NEXT_DAY, at(10, 30) + NEXT_DAY)
            ]
        );
    }

    #[tokio::test]
    async fn copy_to_day_of_a_running_recording_asks_to_stop_the_timer() {
        let w = World::new().await;
        let timer = test_support::recording(&w.db, &w.project.id, at(10, 0), None).await;

        let err = w
            .service()
            .copy_to_day(&timer.id, None, next_day(), Overlap::Cancel)
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(w.rows().await, [timer]);
        assert_eq!(w.status().await, no_status());
    }

    fn local_time(ms: Ms) -> NaiveTime {
        chrono::DateTime::from_timestamp_millis(ms)
            .unwrap()
            .with_timezone(&Local)
            .time()
    }

    #[tokio::test]
    async fn move_to_day_without_a_span_shifts_every_span_by_the_same_days_at_its_local_time() {
        let w = World::new().await;
        let first_day = Day::of(at(10, 0));
        let saved = w
            .create_spans(&[
                (at(10, 0), at(11, 0)),
                (at(14, 0) + NEXT_DAY, at(15, 30) + NEXT_DAY),
            ])
            .await;
        let target = Day::parse("2026-11-03").unwrap();

        let moved = w
            .service()
            .move_to_day(&saved.id, None, target, Overlap::Cancel)
            .await
            .unwrap()
            .value;

        let days_apart = target.date() - first_day.date();
        assert_eq!(moved.spans.len(), 2);
        for (before, after) in saved.spans.iter().zip(&moved.spans) {
            assert_eq!(after.id, before.id);
            assert_eq!(local_time(after.start), local_time(before.start));
            assert_eq!(
                Day::of(after.start).date() - Day::of(before.start).date(),
                days_apart
            );
            assert_eq!(
                after.end.unwrap() - after.start,
                before.end.unwrap() - before.start
            );
        }
        assert_eq!(Day::of(moved.start()), target);
        assert_eq!(w.status().await, undo_label("Move recording"));
    }

    #[tokio::test]
    async fn move_to_day_with_a_span_moves_only_that_span() {
        let w = World::new().await;
        let saved = w
            .create_spans(&[(at(10, 0), at(11, 0)), (at(14, 0), at(15, 0))])
            .await;

        let moved = w
            .service()
            .move_to_day(
                &saved.id,
                Some(saved.spans[1].id.as_str()),
                next_day(),
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;

        assert_eq!(moved.spans[0], saved.spans[0]);
        assert_eq!(moved.spans[1].id, saved.spans[1].id);
        assert_eq!(Day::of(moved.spans[1].start), next_day());
        assert_eq!(
            local_time(moved.spans[1].start),
            local_time(saved.spans[1].start)
        );
        assert_eq!(moved.spans[1].end.unwrap() - moved.spans[1].start, HOUR);
        assert_eq!(w.rows().await, [moved]);
    }

    #[tokio::test]
    async fn copy_to_day_with_a_span_creates_a_single_span_recording() {
        let w = World::new().await;
        let original = w
            .service()
            .create(
                RecordingInput {
                    note: "copy me".into(),
                    tag_id: Some(w.alpha.id.clone()),
                    ..input_of(&w, &[(at(10, 0), at(11, 0)), (at(14, 0), at(15, 30))])
                },
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;

        let copy = w
            .service()
            .copy_to_day(
                &original.id,
                Some(original.spans[1].id.as_str()),
                next_day(),
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;

        assert_ne!(copy.id, original.id);
        assert_eq!(
            times(&copy),
            [(at(14, 0) + NEXT_DAY, at(15, 30) + NEXT_DAY)]
        );
        assert_ne!(copy.spans[0].id, original.spans[1].id);
        assert_eq!(
            (&copy.project_id, &copy.note, &copy.tag_id),
            (&original.project_id, &original.note, &original.tag_id)
        );
        assert_eq!(w.rows().await, [original, copy]);
        assert_eq!(w.status().await, undo_label("Record 1 h 30 min for Acme"));
    }

    #[tokio::test]
    async fn copy_to_day_without_a_span_copies_every_span_into_a_new_recording() {
        let w = World::new().await;
        let original = w
            .create_spans(&[(at(10, 0), at(11, 0)), (at(14, 0), at(15, 0))])
            .await;

        let copy = w
            .service()
            .copy_to_day(&original.id, None, next_day(), Overlap::Cancel)
            .await
            .unwrap()
            .value;

        assert_ne!(copy.id, original.id);
        assert_eq!(
            times(&copy),
            [
                (at(10, 0) + NEXT_DAY, at(11, 0) + NEXT_DAY),
                (at(14, 0) + NEXT_DAY, at(15, 0) + NEXT_DAY)
            ]
        );
        assert!(
            copy.spans
                .iter()
                .all(|c| original.spans.iter().all(|o| o.id != c.id))
        );
        assert_eq!(w.rows().await, [original, copy]);
    }

    #[tokio::test]
    async fn moving_or_copying_the_open_span_of_a_running_recording_asks_to_stop_the_timer() {
        let w = World::new().await;
        let timer = test_support::recording(&w.db, &w.project.id, at(10, 0), None).await;
        let open = timer.spans[0].id.clone();

        let moved = w
            .service()
            .move_to_day(&timer.id, Some(open.as_str()), next_day(), Overlap::Cancel)
            .await
            .unwrap_err();
        let copied = w
            .service()
            .copy_to_day(&timer.id, Some(open.as_str()), next_day(), Overlap::Cancel)
            .await
            .unwrap_err();

        for err in [moved, copied] {
            assert!(matches!(&err, AppError::InvalidInput(m) if m == "Stop the timer first"));
        }
        assert_eq!(w.rows().await, [timer]);
    }

    /// Undoes and redoes the change `label` made on top of the rows `before`.
    async fn assert_round_trip(w: &World, label: &str, before: &[Recording]) {
        let after = w.rows().await;
        assert_eq!(w.status().await.undo_label.as_deref(), Some(label));

        let undone = w.service().undo().await.unwrap();
        assert_eq!(undone.value.as_deref(), Some(label));
        assert_eq!(w.rows().await, before);

        let redone = w.service().redo().await.unwrap();
        assert_eq!(redone.value.as_deref(), Some(label));
        assert_eq!(w.rows().await, after);
    }

    #[tokio::test]
    async fn add_span_joins_a_snapped_span_to_the_recording() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;
        let before = w.rows().await;

        let added = w
            .service()
            .add_span(&saved.id, span(at(12, 3), at(12, 58)), Overlap::Cancel)
            .await
            .unwrap();

        assert_eq!(
            times(&added.value),
            [(at(10, 0), at(11, 0)), (at(12, 5), at(13, 0))]
        );
        assert_eq!(added.value.spans[0].id, saved.spans[0].id);
        assert_eq!(w.rows().await, [added.value.clone()]);
        assert!(added.days.contains(&Day::of(at(12, 5))));
        assert_round_trip(&w, "Add span", &before).await;
    }

    #[tokio::test]
    async fn add_span_refuses_a_span_overlapping_the_recordings_own_or_an_invalid_one() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;

        for (name, new, field) in [
            ("inside its own span", span(at(10, 30), at(12, 0)), "spans"),
            ("an end before the start", span(at(13, 0), at(12, 0)), "end"),
            (
                "longer than 24 h",
                span(at(13, 0), at(13, 0) + 25 * HOUR),
                "end",
            ),
        ] {
            let err = w
                .service()
                .add_span(&saved.id, new, Overlap::Replace)
                .await
                .unwrap_err();
            assert_eq!(field_of(&err).as_deref(), Some(field), "{name}");
        }

        assert_eq!(w.rows().await, [saved]);
    }

    #[tokio::test]
    async fn add_span_follows_the_overlap_mode_for_other_recordings() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;
        let neighbour = w.create(at(12, 0), at(13, 0)).await;

        let err = w
            .service()
            .add_span(&saved.id, span(at(12, 30), at(14, 0)), Overlap::Cancel)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Overlap);
        assert_eq!(overlap_ids(&err), [neighbour.id.clone()]);
        assert_eq!(w.rows().await, [saved.clone(), neighbour]);

        w.service()
            .add_span(&saved.id, span(at(12, 30), at(14, 0)), Overlap::Replace)
            .await
            .unwrap();
        assert_eq!(
            spans(&w.rows().await),
            [
                (at(10, 0), at(11, 0)),
                (at(12, 0), at(12, 30)),
                (at(12, 30), at(14, 0))
            ]
        );
    }

    #[tokio::test]
    async fn update_span_changes_only_that_span_and_keeps_its_id() {
        let w = World::new().await;
        let saved = w
            .create_spans(&[(at(10, 0), at(11, 0)), (at(13, 0), at(14, 0))])
            .await;
        let before = w.rows().await;

        let updated = w
            .service()
            .update_span(
                &saved.id,
                &saved.spans[1].id,
                span(at(13, 2), at(14, 28)),
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;

        assert_eq!(
            times(&updated),
            [(at(10, 0), at(11, 0)), (at(13, 0), at(14, 30))]
        );
        assert_eq!(span_ids(&updated), span_ids(&saved));
        assert_round_trip(&w, "Resize recording", &before).await;
    }

    #[tokio::test]
    async fn update_span_to_the_same_length_is_labelled_a_move() {
        let w = World::new().await;
        let saved = w
            .create_spans(&[(at(10, 0), at(11, 0)), (at(13, 0), at(14, 0))])
            .await;
        let before = w.rows().await;

        w.service()
            .update_span(
                &saved.id,
                &saved.spans[1].id,
                span(at(15, 0), at(16, 0)),
                Overlap::Cancel,
            )
            .await
            .unwrap();

        assert_round_trip(&w, "Move recording", &before).await;
    }

    #[tokio::test]
    async fn update_span_refuses_overlapping_a_sibling_and_unknown_spans() {
        let w = World::new().await;
        let saved = w
            .create_spans(&[(at(10, 0), at(11, 0)), (at(13, 0), at(14, 0))])
            .await;

        let sibling = w
            .service()
            .update_span(
                &saved.id,
                &saved.spans[1].id,
                span(at(10, 30), at(14, 0)),
                Overlap::Replace,
            )
            .await
            .unwrap_err();
        let unknown = w
            .service()
            .update_span(
                &saved.id,
                "nope",
                span(at(15, 0), at(16, 0)),
                Overlap::Cancel,
            )
            .await
            .unwrap_err();

        assert_eq!(field_of(&sibling).as_deref(), Some("spans"));
        assert_eq!(unknown.kind(), ErrorKind::NotFound);
        assert_eq!(w.rows().await, [saved]);
        assert_eq!(
            w.status().await.undo_label.as_deref(),
            Some("Record 2 h 00 min for Acme")
        );
    }

    #[tokio::test]
    async fn update_span_follows_the_overlap_mode_for_other_recordings() {
        let w = World::new().await;
        let saved = w
            .create_spans(&[(at(8, 0), at(9, 0)), (at(10, 0), at(11, 0))])
            .await;
        let neighbour = w.create(at(12, 0), at(13, 0)).await;

        let err = w
            .service()
            .update_span(
                &saved.id,
                &saved.spans[1].id,
                span(at(10, 0), at(12, 30)),
                Overlap::Cancel,
            )
            .await
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Overlap);
        assert_eq!(w.rows().await, [saved.clone(), neighbour]);

        w.service()
            .update_span(
                &saved.id,
                &saved.spans[1].id,
                span(at(10, 0), at(12, 30)),
                Overlap::Replace,
            )
            .await
            .unwrap();
        assert_eq!(
            spans(&w.rows().await),
            [
                (at(8, 0), at(9, 0)),
                (at(10, 0), at(12, 30)),
                (at(12, 30), at(13, 0))
            ]
        );
    }

    #[tokio::test]
    async fn nudge_span_moves_the_start_and_the_end_by_whole_snap_steps() {
        let cases = [
            (5, 1, 1, (at(10, 5), at(11, 5)), "Move recording"),
            (5, -2, -2, (at(9, 50), at(10, 50)), "Move recording"),
            (5, 0, 1, (at(10, 0), at(11, 5)), "Resize recording"),
            (5, 0, -1, (at(10, 0), at(10, 55)), "Resize recording"),
            (5, 1, 0, (at(10, 5), at(11, 0)), "Resize recording"),
            (5, -1, 2, (at(9, 55), at(11, 10)), "Resize recording"),
            (15, 1, 1, (at(10, 15), at(11, 15)), "Move recording"),
            (15, 0, -1, (at(10, 0), at(10, 45)), "Resize recording"),
            (1, 0, 1, (at(10, 0), at(11, 1)), "Resize recording"),
        ];
        for (snap_minutes, start_steps, end_steps, expected, label) in cases {
            let w = World::new().await;
            let saved = w
                .create_spans(&[(at(8, 0), at(9, 0)), (at(10, 0), at(11, 0))])
                .await;
            let before = w.rows().await;

            let nudged = w
                .snapping(snap_minutes)
                .nudge_span(
                    &saved.id,
                    &saved.spans[1].id,
                    start_steps,
                    end_steps,
                    Overlap::Cancel,
                )
                .await
                .unwrap()
                .value;

            let name = format!("{snap_minutes} min, ({start_steps}, {end_steps})");
            assert_eq!(times(&nudged), [(at(8, 0), at(9, 0)), expected], "{name}");
            assert_eq!(span_ids(&nudged), span_ids(&saved), "{name}");
            assert_round_trip(&w, label, &before).await;
        }
    }

    #[tokio::test]
    async fn nudge_span_refuses_a_span_that_would_end_before_it_starts() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;

        let err = w
            .service()
            .nudge_span(&saved.id, &saved.spans[0].id, 12, 0, Overlap::Cancel)
            .await
            .unwrap_err();

        assert_eq!(field_of(&err).as_deref(), Some("end"));
        assert_eq!(w.rows().await, [saved]);
    }

    #[tokio::test]
    async fn nudge_span_refuses_the_open_span_of_a_running_recording() {
        let w = World::new().await;
        let timer = test_support::recording(&w.db, &w.project.id, at(10, 0), None).await;

        let err = w
            .service()
            .nudge_span(&timer.id, &timer.spans[0].id, 1, 1, Overlap::Cancel)
            .await
            .unwrap_err();

        assert!(matches!(&err, AppError::InvalidInput(m) if m == "Stop the timer first"));
        assert_eq!(w.rows().await, [timer]);
        assert_eq!(w.status().await, no_status());
    }

    #[tokio::test]
    async fn nudge_span_of_unknown_ids_is_not_found() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;

        let span = w
            .service()
            .nudge_span(&saved.id, "nope", 1, 1, Overlap::Cancel)
            .await
            .unwrap_err();
        let recording = w
            .service()
            .nudge_span("nope", &saved.spans[0].id, 1, 1, Overlap::Cancel)
            .await
            .unwrap_err();

        assert_eq!(span.kind(), ErrorKind::NotFound);
        assert_eq!(recording.kind(), ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn nudge_span_follows_the_overlap_mode() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;
        let neighbour = w.create(at(11, 0), at(12, 0)).await;

        let err = w
            .service()
            .nudge_span(&saved.id, &saved.spans[0].id, 0, 2, Overlap::Cancel)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Overlap);
        assert_eq!(overlap_ids(&err), [neighbour.id.clone()]);
        assert_eq!(w.rows().await, [saved.clone(), neighbour]);

        w.service()
            .nudge_span(&saved.id, &saved.spans[0].id, 0, 2, Overlap::Replace)
            .await
            .unwrap();
        assert_eq!(
            spans(&w.rows().await),
            [(at(10, 0), at(11, 10)), (at(11, 10), at(12, 0))]
        );
    }

    #[tokio::test]
    async fn remove_span_keeps_the_other_spans() {
        let w = World::new().await;
        let saved = w
            .create_spans(&[(at(10, 0), at(11, 0)), (at(13, 0), at(14, 0))])
            .await;
        let before = w.rows().await;

        let change = w
            .service()
            .remove_span(&saved.id, &saved.spans[0].id)
            .await
            .unwrap();

        let rest = change.value.unwrap();
        assert_eq!(times(&rest), [(at(13, 0), at(14, 0))]);
        assert_eq!(rest.spans[0].id, saved.spans[1].id);
        assert_eq!(w.rows().await, [rest]);
        assert_round_trip(&w, "Remove span", &before).await;
    }

    #[tokio::test]
    async fn removing_the_last_span_deletes_the_recording_and_returns_none() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;
        let before = w.rows().await;

        let change = w
            .service()
            .remove_span(&saved.id, &saved.spans[0].id)
            .await
            .unwrap();

        assert_eq!(change.value, None);
        assert_eq!(change.days, [Day::of(at(10, 0))]);
        assert!(w.rows().await.is_empty());
        assert_round_trip(&w, "Delete recording", &before).await;
    }

    #[tokio::test]
    async fn remove_span_of_an_unknown_span_or_recording_is_not_found() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;

        let span_err = w
            .service()
            .remove_span(&saved.id, "nope")
            .await
            .unwrap_err();
        let recording_err = w
            .service()
            .remove_span("nope", &saved.spans[0].id)
            .await
            .unwrap_err();

        assert_eq!(span_err.kind(), ErrorKind::NotFound);
        assert_eq!(recording_err.kind(), ErrorKind::NotFound);
        assert_eq!(w.rows().await, [saved]);
    }

    #[tokio::test]
    async fn split_span_cuts_a_span_in_two_at_the_snapped_instant() {
        let w = World::new().await;
        let saved = w
            .create_spans(&[(at(8, 0), at(9, 0)), (at(10, 0), at(12, 0))])
            .await;
        let before = w.rows().await;

        let split = w
            .service()
            .split_span(&saved.id, &saved.spans[1].id, at(11, 2))
            .await
            .unwrap()
            .value;

        assert_eq!(
            times(&split),
            [
                (at(8, 0), at(9, 0)),
                (at(10, 0), at(11, 0)),
                (at(11, 0), at(12, 0))
            ]
        );
        assert_eq!(split.spans[1].id, saved.spans[1].id);
        assert_ne!(split.spans[2].id, saved.spans[1].id);
        assert_round_trip(&w, "Split span", &before).await;
    }

    #[tokio::test]
    async fn split_span_outside_the_span_is_refused_on_the_at_field() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;
        let span_id = saved.spans[0].id.clone();

        for (name, at_ms) in [
            ("before the span", at(9, 0)),
            ("at the start", at(10, 0)),
            ("snapping onto the start", at(10, 1)),
            ("at the end", at(11, 0)),
            ("snapping onto the end", at(10, 59)),
            ("after the span", at(12, 0)),
        ] {
            let err = w
                .service()
                .split_span(&saved.id, &span_id, at_ms)
                .await
                .unwrap_err();
            assert_eq!(field_of(&err).as_deref(), Some("at"), "{name}");
        }

        assert_eq!(w.rows().await, [saved]);
        assert_eq!(
            w.status().await.undo_label.as_deref(),
            Some("Record 1 h 00 min for Acme")
        );
    }

    #[tokio::test]
    async fn merge_moves_the_spans_of_the_source_into_the_target_and_deletes_the_source() {
        let w = World::new().await;
        let target = w
            .service()
            .create(
                RecordingInput {
                    note: "keep".into(),
                    tag_id: Some(w.alpha.id.clone()),
                    ..input_of(&w, &[(at(10, 0), at(11, 0))])
                },
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;
        let source = w
            .service()
            .create(
                RecordingInput {
                    project_id: w.other.id.clone(),
                    note: "drop".into(),
                    tag_id: Some(w.beta.id.clone()),
                    ..input_of(&w, &[(at(8, 0), at(9, 0)), (at(12, 0), at(13, 0))])
                },
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;
        let before = w.rows().await;

        let merged = w
            .service()
            .merge(&target.id, &source.id)
            .await
            .unwrap()
            .value;

        assert_eq!(merged.id, target.id);
        assert_eq!(
            (&merged.project_id, &merged.note, &merged.tag_id),
            (&target.project_id, &target.note, &target.tag_id)
        );
        assert_eq!(
            times(&merged),
            [
                (at(8, 0), at(9, 0)),
                (at(10, 0), at(11, 0)),
                (at(12, 0), at(13, 0))
            ]
        );
        assert_eq!(
            span_ids(&merged),
            [
                source.spans[0].id.clone(),
                target.spans[0].id.clone(),
                source.spans[1].id.clone()
            ]
        );
        assert_eq!(w.rows().await, [merged]);
        assert_round_trip(&w, "Merge recordings", &before).await;
    }

    #[tokio::test]
    async fn merge_refuses_the_same_recording_and_unknown_ones() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;

        let same = w.service().merge(&saved.id, &saved.id).await.unwrap_err();
        let no_source = w.service().merge(&saved.id, "nope").await.unwrap_err();
        let no_target = w.service().merge("nope", &saved.id).await.unwrap_err();

        assert_eq!(same.kind(), ErrorKind::InvalidInput);
        assert_eq!(field_of(&same).as_deref(), Some("from"));
        assert_eq!(no_source.kind(), ErrorKind::NotFound);
        assert_eq!(no_target.kind(), ErrorKind::NotFound);
        assert_eq!(w.rows().await, [saved]);
        assert_eq!(
            w.status().await.undo_label.as_deref(),
            Some("Record 1 h 00 min for Acme")
        );
    }

    #[tokio::test]
    async fn merge_of_recordings_whose_spans_overlap_is_refused() {
        let w = World::new().await;
        let target =
            test_support::recording(&w.db, &w.project.id, at(10, 0), Some(at(11, 0))).await;
        let source =
            test_support::recording(&w.db, &w.project.id, at(10, 30), Some(at(12, 0))).await;

        let err = w.service().merge(&target.id, &source.id).await.unwrap_err();

        assert_eq!(field_of(&err).as_deref(), Some("spans"));
        assert_eq!(w.rows().await, [target, source]);
        assert_eq!(w.status().await, no_status());
    }

    #[tokio::test]
    async fn span_operations_on_a_running_recording_ask_to_stop_the_timer() {
        let w = World::new().await;
        let other = w.create(at(7, 0), at(8, 0)).await;
        let timer = test_support::recording(&w.db, &w.project.id, at(10, 0), None).await;
        let id = timer.id.as_str();
        let span_id = timer.spans[0].id.as_str();
        let service = w.service();

        let errors = [
            service
                .add_span(id, span(at(12, 0), at(13, 0)), Overlap::Cancel)
                .await
                .unwrap_err(),
            service
                .update_span(id, span_id, span(at(12, 0), at(13, 0)), Overlap::Cancel)
                .await
                .unwrap_err(),
            service.remove_span(id, span_id).await.unwrap_err(),
            service
                .split_span(id, span_id, at(10, 30))
                .await
                .unwrap_err(),
            service.merge(id, &other.id).await.unwrap_err(),
            service.merge(&other.id, id).await.unwrap_err(),
        ];

        for err in errors {
            assert!(matches!(&err, AppError::InvalidInput(m) if m == "Stop the timer first"));
        }
        assert_eq!(w.rows().await, [other, timer]);
    }

    #[tokio::test]
    async fn create_all_saves_snapped_recordings_as_one_undo_step() {
        let w = World::new().await;
        let inputs = vec![
            RecordingInput {
                note: " standup ".into(),
                tag_id: Some(w.alpha.id.clone()),
                ..w.input(at(10, 2), at(10, 58))
            },
            w.input(at(12, 0), at(13, 0)),
        ];

        let change = w
            .service()
            .create_all(inputs, "Accept suggestions of Slack".into())
            .await
            .unwrap();

        assert_eq!(
            spans(&change.value),
            [(at(10, 0), at(11, 0)), (at(12, 0), at(13, 0))]
        );
        assert_eq!(
            (change.value[0].note.as_str(), &change.value[0].tag_id),
            ("standup", &Some(w.alpha.id.clone()))
        );
        assert!(change.days.contains(&Day::of(at(12, 0))));
        assert_eq!(w.rows().await, change.value);
        assert_eq!(w.status().await, undo_label("Accept suggestions of Slack"));

        let undone = w.service().undo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Accept suggestions of Slack"));
        assert!(w.rows().await.is_empty());
        assert_eq!(w.status().await.undo_label, None);

        w.service().redo().await.unwrap();
        assert_eq!(w.rows().await, change.value);
    }

    #[tokio::test]
    async fn create_all_skips_inputs_that_overlap_a_recording_or_an_earlier_input() {
        let w = World::new().await;
        let existing = w.create(at(10, 0), at(11, 0)).await;
        let inputs = vec![
            w.input(at(9, 30), at(10, 30)),
            w.input(at(11, 0), at(12, 0)),
            w.input(at(11, 30), at(12, 30)),
            w.input(at(12, 0), at(13, 0)),
        ];

        let change = w
            .service()
            .create_all(inputs, "Accept".into())
            .await
            .unwrap();

        assert_eq!(
            spans(&change.value),
            [(at(11, 0), at(12, 0)), (at(12, 0), at(13, 0))]
        );
        let rows = w.rows().await;
        assert_eq!(rows[0], existing);
        assert_eq!(rows.len(), 3);
        assert!(no_two_overlap(&rows));

        w.service().undo().await.unwrap();

        assert_eq!(w.rows().await, [existing]);
    }

    #[tokio::test]
    async fn create_all_without_anything_to_create_leaves_no_undo_entry_and_no_days() {
        let w = World::new().await;
        w.create(at(10, 0), at(11, 0)).await;
        let before = w.status().await;

        let empty = w
            .service()
            .create_all(Vec::new(), "Accept".into())
            .await
            .unwrap();
        let all_skipped = w
            .service()
            .create_all(vec![w.input(at(10, 30), at(11, 30))], "Accept".into())
            .await
            .unwrap();

        for change in [empty, all_skipped] {
            assert!(change.value.is_empty());
            assert!(change.days.is_empty());
        }
        assert_eq!(w.status().await, before);
        assert_eq!(w.rows().await.len(), 1);
    }

    #[tokio::test]
    async fn create_all_with_an_invalid_input_saves_nothing() {
        let w = World::new().await;
        let inputs = vec![
            w.input(at(10, 0), at(11, 0)),
            RecordingInput {
                note: "a\nb".into(),
                ..w.input(at(12, 0), at(13, 0))
            },
        ];

        let err = w
            .service()
            .create_all(inputs, "Accept".into())
            .await
            .unwrap_err();

        assert_eq!(field_of(&err).as_deref(), Some("note"));
        assert!(w.rows().await.is_empty());
        assert_eq!(w.status().await, no_status());
    }
}
