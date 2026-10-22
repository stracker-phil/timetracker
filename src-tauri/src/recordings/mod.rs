//! Recordings as the user makes them: validated, snapped, kept free of overlaps, and undoable. Every write
//! is an `ops::Op` applied in one transaction and pushed on the `UndoStack` (ADR 0010). A recording holds
//! one or more spans; the overlap rule applies per span (ADR 0020).

mod checks;
pub mod cli;
pub mod commands;
mod draft;
pub mod ops;
mod overlaps;
mod proposal;
mod review;
mod snap;
mod spans;
#[cfg(test)]
mod testing;
pub mod timer;
mod to_day;
pub mod undo;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use ts_rs::TS;

use crate::db::projects::Project;
pub use crate::db::recordings::Span;
use crate::db::recordings::{self, Recording, RecordingPatch, RecordingSpan, respanned};
use crate::db::time::{Day, Ms, now_ms};
use crate::db::{Db, new_id};
use crate::error::AppResult;
use checks::{check_tag, checked_note, require_finished, snapped_spans, usable_project};
use ops::Op;
use overlaps::{claimed, clearing, in_the_way};
pub use review::suggestions_label;
pub use snap::{
    NEW_RECORDING_MINUTES, SnapEdge, default_span, default_span_on, grid_step, snap, snap_moving,
    snap_span,
};
use undo::{Entry, UndoStack};

const MINUTE_MS: Ms = 60_000;

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

/// Closed spans with new ids.
fn new_spans(spans: &[Span]) -> Vec<RecordingSpan> {
    spans
        .iter()
        .map(|s| RecordingSpan::new(s.start, Some(s.end)))
        .collect()
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
        .chain(op.dismissal_days())
        .collect();
    days.sort();
    days.dedup();
    days
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
        self.create_with(input, overlap, Vec::new()).await
    }

    /// `create`, with `also` applied in the same undoable change.
    async fn create_with(
        &self,
        input: RecordingInput,
        overlap: Overlap,
        also: Vec<Op>,
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
        let mut op = clearing(self.db, base, &recording, overlap, &label).await?;
        if !also.is_empty() {
            let ops = std::iter::once(op).chain(also).collect();
            op = Op::Batch {
                label: label.clone(),
                ops,
            };
        }
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
    use serde_json::json;

    use super::testing::*;
    use super::undo::UndoStatus;
    use super::*;
    use crate::db::projects;
    use crate::error::ErrorKind;
    use crate::test_support;

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
