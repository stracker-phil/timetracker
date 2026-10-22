//! Writes on single spans of a recording, and merging two recordings.

use super::checks::{closed_span, invalid, require_finished, snapped_span};
use super::ops::Op;
use super::{Change, MINUTE_MS, Overlap, Recordings, Span, snap, update_label};
use crate::db::recordings::{self, Recording, RecordingSpan};
use crate::db::time::{Ms, now_ms};
use crate::error::AppResult;

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

impl Recordings<'_> {
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
        let span = closed_span(current.span(span_id)?)?;
        let step = i64::from(self.snap_minutes.max(1)) * MINUTE_MS;
        let nudged = Span {
            start: span.start + i64::from(start_steps) * step,
            end: span.end + i64::from(end_steps) * step,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::time::Day;
    use crate::error::{AppError, ErrorKind};
    use crate::recordings::RecordingInput;
    use crate::recordings::testing::*;
    use crate::test_support;

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
}
