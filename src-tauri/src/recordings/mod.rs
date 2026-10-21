//! Recordings as the user makes them: validated, snapped, kept free of overlaps, and undoable. Every write
//! is an `ops::Op` applied in one transaction and pushed on the `UndoStack` (ADR 0010).

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
use crate::db::recordings::{self, Recording, RecordingPatch};
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
    pub start: Ms,
    pub end: Ms,
    pub note: String,
    pub tag_id: Option<String>,
}

/// What a write does when its span overlaps other recordings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum Overlap {
    /// Trims, splits or deletes the recordings in the way.
    Replace,
    /// Refuses with `overlap`, listing them.
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Span {
    pub start: Ms,
    pub end: Ms,
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
        .flat_map(|r| days_of(r.start, r.end.unwrap_or(now)))
        .collect();
    days.sort();
    days.dedup();
    days
}

/// What clears `span` of `other`: deleted when inside it, trimmed when it sticks out on one side, split
/// around it (the right part a new recording) when it sticks out on both.
fn clear_op(other: Recording, span: Span, now: Ms) -> AppResult<Vec<Op>> {
    let Some(other_end) = other.end else {
        return Err(timer_running());
    };
    let trimmed = |start: Ms, end: Ms| Recording {
        start,
        end: Some(end),
        updated_at: now,
        ..other.clone()
    };
    let ops = match (other.start < span.start, other_end > span.end) {
        (false, false) => vec![Op::Delete { recording: other }],
        (true, false) => vec![Op::Update {
            after: trimmed(other.start, span.start),
            before: other,
        }],
        (false, true) => vec![Op::Update {
            after: trimmed(span.end, other_end),
            before: other,
        }],
        (true, true) => {
            let right = Recording {
                id: new_id(),
                created_at: now,
                ..trimmed(span.end, other_end)
            };
            vec![
                Op::Update {
                    after: trimmed(other.start, span.start),
                    before: other,
                },
                Op::Create { recording: right },
            ]
        }
    };
    Ok(ops)
}

/// `base` as is when nothing is in the way of `new`, else per `overlap`: refused, or batched after what
/// clears its span.
async fn clearing(
    db: &Db,
    base: Op,
    new: &Recording,
    overlap: Overlap,
    label: &str,
) -> AppResult<Op> {
    let span = Span {
        start: new.start,
        end: new.end.unwrap_or(Ms::MAX),
    };
    let others = recordings::overlapping(db, span.start, span.end, Some(&new.id)).await?;
    if others.is_empty() {
        return Ok(base);
    }
    if overlap == Overlap::Cancel {
        return Err(overlap_error(others));
    }
    let now = now_ms();
    let mut ops = Vec::new();
    for other in others {
        ops.extend(clear_op(other, span, now)?);
    }
    ops.push(base);
    Ok(Op::Batch {
        label: label.to_string(),
        ops,
    })
}

/// `current` with `patch` applied: changed times snapped, the rest validated.
fn patched(current: &Recording, patch: RecordingPatch, snap_minutes: u32) -> AppResult<Recording> {
    let Some(current_end) = current.end else {
        return Err(timer_running());
    };
    let end = match patch.end {
        Some(None) => return Err(invalid("end", "is required")),
        Some(Some(end)) => snap(end, snap_minutes),
        None => current_end,
    };
    if patch
        .start
        .zip(patch.end.flatten())
        .is_some_and(|(s, e)| e <= s)
    {
        return Err(invalid("end", "must be after the start"));
    }
    let start = patch.start.map_or(current.start, |s| snap(s, snap_minutes));
    let span = Span {
        start,
        end: past_start(start, end, snap_minutes),
    };
    check_length(span)?;
    let note = match &patch.note {
        Some(note) => checked_note(note)?,
        None => current.note.clone(),
    };
    Ok(Recording {
        project_id: patch
            .project_id
            .unwrap_or_else(|| current.project_id.clone()),
        start: span.start,
        end: Some(span.end),
        note,
        tag_id: patch.tag_id.unwrap_or_else(|| current.tag_id.clone()),
        updated_at: now_ms(),
        ..current.clone()
    })
}

/// The span `recording` would have on `day`: the same local start time, the same length. A recording over
/// midnight moves with the day it starts on.
fn span_on_day(recording: &Recording, day: Day) -> AppResult<(Ms, Ms)> {
    let end = recording.end.ok_or_else(timer_running)?;
    let start = same_time_on(recording.start, day);
    Ok((start, start + (end - recording.start)))
}

/// The undo label of an update: moved (same length), resized, or edited (same times).
fn update_label(before: &Recording, after: &Recording) -> &'static str {
    let length = |r: &Recording| r.end.map(|end| end - r.start);
    if (after.start, after.end) == (before.start, before.end) {
        "Edit recording"
    } else if length(after) == length(before) {
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
            duration_text(recording.end.unwrap_or(recording.start) - recording.start),
            project.name
        );
        let base = Op::Create {
            recording: recording.clone(),
        };
        let op = clearing(self.db, base, &recording, overlap, &label).await?;
        self.commit(&mut undo, label, op, &recording.id).await
    }

    /// New recordings as one undoable change; one that would overlap a recording, or one created before it,
    /// is left out.
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
            let (start, end) = (recording.start, recording.end.unwrap_or(recording.start));
            let overlaps_kept = kept
                .iter()
                .any(|r| r.start < end && r.end.is_some_and(|e| e > start));
            if overlaps_kept
                || !recordings::overlapping(self.db, start, end, None)
                    .await?
                    .is_empty()
            {
                continue;
            }
            kept.push(recording);
        }
        Ok(kept)
    }

    /// `input` validated and snapped, with its project; not stored yet.
    async fn new_recording(&self, input: RecordingInput) -> AppResult<(Recording, Project)> {
        if input.end <= input.start {
            return Err(invalid("end", "must be after the start"));
        }
        let span = snap_span(input.start, input.end, self.snap_minutes);
        check_length(span)?;
        let note = checked_note(&input.note)?;
        let project = usable_project(self.db, &input.project_id, None).await?;
        check_tag(self.db, input.tag_id.as_deref()).await?;
        let now = now_ms();
        let recording = Recording {
            id: new_id(),
            project_id: project.id.clone(),
            start: span.start,
            end: Some(span.end),
            note,
            tag_id: input.tag_id,
            created_at: now,
            updated_at: now,
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
        let base = Op::Update {
            before: current,
            after: after.clone(),
        };
        let op = clearing(self.db, base, &after, overlap, label).await?;
        self.commit(&mut undo, label.to_string(), op, &after.id)
            .await
    }

    /// Moves the recording to `day` at the same local time, keeping its length.
    pub async fn move_to_day(
        &self,
        id: &str,
        day: Day,
        overlap: Overlap,
    ) -> AppResult<Change<Recording>> {
        let (start, end) = span_on_day(&recordings::get(self.db, id).await?, day)?;
        let patch = RecordingPatch {
            start: Some(start),
            end: Some(Some(end)),
            ..RecordingPatch::default()
        };
        self.update(id, patch, overlap).await
    }

    /// A new recording like `id` on `day` at the same local time.
    pub async fn copy_to_day(
        &self,
        id: &str,
        day: Day,
        overlap: Overlap,
    ) -> AppResult<Change<Recording>> {
        let original = recordings::get(self.db, id).await?;
        let (start, end) = span_on_day(&original, day)?;
        let input = RecordingInput {
            project_id: original.project_id,
            start,
            end,
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
                start: at(10, 0),
                end: Some(at(11, 0)),
                note: "fix login".into(),
                tag_id: Some(w.alpha.id.clone()),
                created_at: saved.created_at,
                updated_at: saved.updated_at,
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

        assert_eq!((saved.start, saved.end), (at(10, 0), Some(at(10, 45))));
    }

    #[tokio::test]
    async fn a_span_shorter_than_one_step_is_saved_as_one_step() {
        let w = World::new().await;

        let saved = w.create(at(10, 0), at(10, 1)).await;

        assert_eq!((saved.start, saved.end), (at(10, 0), Some(at(10, 5))));
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
        assert_eq!(saved.end, Some(at(0, 0) + 24 * HOUR));
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
                RecordingInput {
                    end: at(9, 0),
                    ..valid.clone()
                },
                "end",
            ),
            (
                "an end equal to the start",
                RecordingInput {
                    end: at(10, 0),
                    ..valid.clone()
                },
                "end",
            ),
            (
                "a span longer than 24 h",
                RecordingInput {
                    end: at(10, 0) + 24 * HOUR + 5 * MIN,
                    ..valid.clone()
                },
                "end",
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
    async fn update_labels_name_what_changed() {
        let cases = [
            (
                "both ends shifted",
                patch(|p| {
                    p.start = Some(at(10, 30));
                    p.end = Some(Some(at(11, 30)));
                }),
                "Move recording",
            ),
            (
                "end changed",
                patch(|p| p.end = Some(Some(at(11, 30)))),
                "Resize recording",
            ),
            (
                "start changed",
                patch(|p| p.start = Some(at(9, 30))),
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
            (
                "same times",
                patch(|p| {
                    p.start = Some(at(10, 0));
                    p.end = Some(Some(at(11, 0)));
                }),
                "Edit recording",
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
        let change = patch(|p| {
            p.start = Some(at(10, 32));
            p.end = Some(Some(at(11, 27)));
        });

        let updated = w
            .service()
            .update(&saved.id, change, Overlap::Cancel)
            .await
            .unwrap()
            .value;

        assert_eq!(
            updated,
            Recording {
                start: at(10, 30),
                end: Some(at(11, 25)),
                updated_at: updated.updated_at,
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

        assert_eq!(
            (updated.start, updated.end, updated.note.as_str()),
            (stored.start, stored.end, "later")
        );
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
            ("the end removed", patch(|p| p.end = Some(None)), "end"),
            (
                "an end before the start",
                patch(|p| {
                    p.start = Some(at(12, 0));
                    p.end = Some(Some(at(11, 0)));
                }),
                "end",
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
                patch(|p| p.end = Some(Some(at(10, 0) + 25 * HOUR))),
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
        assert_eq!(w.status().await, undo_label("Record 1 h 00 min for Acme"));
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
    async fn the_right_part_of_a_split_keeps_project_note_and_tag_under_a_new_id() {
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
        assert_eq!(rows.len(), 3);
        let (left, middle, right) = (&rows[0], &rows[1], &rows[2]);
        assert_eq!(
            *left,
            Recording {
                end: Some(at(10, 20)),
                updated_at: left.updated_at,
                ..original.clone()
            }
        );
        assert_eq!(middle.project_id, w.project.id);
        assert_ne!(right.id, original.id);
        assert_eq!(right.tag_id, Some(w.alpha.id.clone()));
        assert_eq!(
            *right,
            Recording {
                id: right.id.clone(),
                start: at(10, 40),
                created_at: right.created_at,
                updated_at: right.updated_at,
                ..original
            }
        );
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

        assert_eq!(saved.end, Some(at(11, 0)));
        assert_eq!(w.rows().await.len(), 2);
    }

    #[tokio::test]
    async fn update_into_another_recording_follows_the_overlap_mode() {
        let w = World::new().await;
        let first = w.create(at(10, 0), at(11, 0)).await;
        let second = w.create(at(12, 0), at(13, 0)).await;
        let earlier = || patch(|p| p.start = Some(at(10, 30)));

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
            .update(
                &saved.id,
                patch(|p| p.end = Some(Some(at(11, 30)))),
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;

        assert_eq!(updated.end, Some(at(11, 30)));
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
            p.start = Some(at(10, 30));
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
            .update(
                &saved.id,
                patch(|p| {
                    p.start = Some(at(12, 0));
                    p.end = Some(Some(at(13, 0)));
                }),
                Overlap::Cancel,
            )
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
                patch(|p| {
                    p.start = Some(midnight + HOUR);
                    p.end = Some(Some(midnight + 2 * HOUR));
                }),
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
                patch(|p| {
                    p.start = Some(midnight - 3 * HOUR);
                    p.end = Some(Some(midnight - 2 * HOUR));
                }),
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
            .move_to_day(&saved.id, next_day(), Overlap::Cancel)
            .await
            .unwrap();

        assert_eq!(moved.value.id, saved.id);
        assert_eq!(
            (moved.value.start, moved.value.end),
            (at(12, 0) + NEXT_DAY, Some(at(13, 30) + NEXT_DAY))
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
            .move_to_day(&saved.id, next_day(), Overlap::Cancel)
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
            .move_to_day(&saved.id, next_day(), Overlap::Cancel)
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
            .move_to_day(&saved.id, next_day(), Overlap::Cancel)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Overlap);
        assert_eq!(w.rows().await, [saved.clone(), blocker]);

        w.service()
            .move_to_day(&saved.id, next_day(), Overlap::Replace)
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
            .move_to_day(&timer.id, next_day(), Overlap::Cancel)
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
            .move_to_day("nope", next_day(), Overlap::Cancel)
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
            .copy_to_day(&original.id, next_day(), Overlap::Cancel)
            .await
            .unwrap();

        let copy = &copied.value;
        assert_ne!(copy.id, original.id);
        assert_eq!(
            (copy.start, copy.end),
            (at(12, 0) + NEXT_DAY, Some(at(13, 30) + NEXT_DAY))
        );
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
            .copy_to_day(&original.id, next_day(), Overlap::Cancel)
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
            .copy_to_day(&original.id, next_day(), Overlap::Cancel)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Overlap);
        assert_eq!(w.rows().await, [original.clone(), blocker]);

        w.service()
            .copy_to_day(&original.id, next_day(), Overlap::Replace)
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
            .copy_to_day(&timer.id, next_day(), Overlap::Cancel)
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(w.rows().await, [timer]);
        assert_eq!(w.status().await, no_status());
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
