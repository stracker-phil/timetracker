//! What may change on the running timer's recording while it runs: its closed spans and where its open
//! span starts. The open span's end is the timer's to set when it stops.

use super::checks::{invalid, snapped_spans};
use super::snap;
use crate::db::recordings::{Recording, RecordingPatch, RecordingSpan, respanned};
use crate::db::time::{Day, Ms};
use crate::error::{AppError, AppResult};

/// A new start of the open span, on the snap grid: today (local midnight or later) and not after `now`.
pub fn checked_open_start(start: Ms, now: Ms, snap_minutes: u32) -> AppResult<Ms> {
    let start = snap(start, snap_minutes);
    if start > now {
        return Err(invalid("start", "A timer can't start in the future"));
    }
    if start < Day::of(now).bounds().0 {
        return Err(invalid("start", "A timer can't start before midnight"));
    }
    Ok(start)
}

/// The running recording's spans with `patch`'s closed spans and open start, by start.
pub(super) fn running_spans(
    current: &Recording,
    open: &RecordingSpan,
    patch: &RecordingPatch,
    snap_minutes: u32,
    now: Ms,
) -> AppResult<Vec<RecordingSpan>> {
    let closed: Vec<RecordingSpan> = current
        .spans
        .iter()
        .filter(|s| s.end.is_some())
        .cloned()
        .collect();
    let mut spans = match &patch.spans {
        Some(spans) if spans.is_empty() => Vec::new(),
        Some(spans) => respanned(&closed, &snapped_spans(spans, snap_minutes)?),
        None => closed,
    };
    let start = match patch.open_start {
        Some(start) => checked_open_start(start, now, snap_minutes)?,
        None => open.start,
    };
    spans.push(RecordingSpan {
        start,
        ..open.clone()
    });
    spans.sort_by_key(|s| s.start);
    Ok(spans)
}

/// The refusal of a write that would move or set the open span's end.
pub(super) fn open_end_fixed() -> AppError {
    AppError::InvalidInput("The timer's end moves when it stops".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::time::now_ms;
    use crate::recordings::testing::*;
    use crate::recordings::{Overlap, SnapEdge, snap};
    use crate::test_support;

    fn refusal(err: AppError) -> (String, String) {
        match err {
            AppError::InvalidField { field, message } => (field, message),
            other => panic!("expected an invalid field, got {other:?}"),
        }
    }

    #[test]
    fn an_open_start_is_snapped_to_the_grid() {
        let now = at(12, 0);

        for (input, expected) in [
            (at(11, 57), at(11, 55)),
            (at(11, 58), at(12, 0)),
            (at(11, 32), at(11, 30)),
        ] {
            assert_eq!(
                checked_open_start(input, now, 5).unwrap(),
                expected,
                "{input}"
            );
        }
    }

    #[test]
    fn an_open_start_at_now_is_accepted() {
        let now = at(12, 0);

        assert_eq!(checked_open_start(now, now, 5).unwrap(), now);
    }

    #[test]
    fn an_open_start_at_local_midnight_is_accepted() {
        let now = at(12, 0);
        let midnight = Day::of(now).bounds().0;

        assert_eq!(checked_open_start(midnight, now, 5).unwrap(), midnight);
    }

    #[test]
    fn an_open_start_after_now_is_refused_on_the_start_field() {
        let err = checked_open_start(at(12, 3), at(12, 0), 5).unwrap_err();

        assert_eq!(
            refusal(err),
            ("start".into(), "A timer can't start in the future".into())
        );
    }

    #[test]
    fn an_open_start_before_local_midnight_is_refused_on_the_start_field() {
        let now = at(12, 0);
        let midnight = Day::of(now).bounds().0;

        let err = checked_open_start(midnight - 5 * MIN, now, 5).unwrap_err();

        assert_eq!(
            refusal(err),
            ("start".into(), "A timer can't start before midnight".into())
        );
    }

    #[tokio::test]
    async fn the_running_recordings_closed_spans_can_be_replaced_while_it_runs() {
        let w = World::new().await;
        let timer = test_support::recording(&w.db, &w.project.id, at(10, 0), None).await;

        let updated = w
            .service()
            .update(&timer.id, respan(at(9, 0), at(9, 30)), Overlap::Cancel)
            .await
            .unwrap()
            .value;

        assert_eq!(updated.spans.len(), 2);
        assert_eq!(
            (updated.spans[0].start, updated.spans[0].end),
            (at(9, 0), Some(at(9, 30)))
        );
        assert_eq!(updated.spans[1], timer.spans[0]);
        assert_eq!(w.rows().await, [updated]);
        assert_eq!(w.status().await, undo_label("Resize recording"));
    }

    #[tokio::test]
    async fn replacing_the_spans_of_a_running_recording_keeps_the_open_span_with_its_id() {
        let w = World::new().await;
        let timer = running_with(&w, &[(at(8, 0), at(9, 0))], at(10, 0)).await;
        let open = timer.spans[1].clone();

        let updated = w
            .service()
            .update(
                &timer.id,
                patch(|p| {
                    p.spans = Some(vec![span(at(6, 0), at(7, 0)), span(at(7, 30), at(8, 0))])
                }),
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;

        assert_eq!(updated.spans.len(), 3);
        assert_eq!(
            (updated.spans[0].start, updated.spans[0].end),
            (at(6, 0), Some(at(7, 0)))
        );
        assert_eq!(
            (updated.spans[1].start, updated.spans[1].end),
            (at(7, 30), Some(at(8, 0)))
        );
        assert_eq!(updated.spans[2], open);
    }

    #[tokio::test]
    async fn an_empty_span_list_leaves_a_running_recording_with_only_its_open_span() {
        let w = World::new().await;
        let timer = running_with(&w, &[(at(8, 0), at(9, 0))], at(10, 0)).await;
        let open = timer.spans[1].clone();

        let updated = w
            .service()
            .update(
                &timer.id,
                patch(|p| p.spans = Some(Vec::new())),
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;

        assert_eq!(updated.spans, [open]);
        assert_eq!(w.rows().await, [updated]);
    }

    #[tokio::test]
    async fn a_closed_span_after_the_open_start_is_refused_on_the_spans_field() {
        let w = World::new().await;
        let timer = test_support::recording(&w.db, &w.project.id, at(10, 0), None).await;

        let err = w
            .service()
            .update(&timer.id, respan(at(11, 0), at(12, 0)), Overlap::Cancel)
            .await
            .unwrap_err();

        assert_eq!(field_of(&err).as_deref(), Some("spans"));
        assert_eq!(w.rows().await, [timer]);
        assert_eq!(w.status().await, no_status());
    }

    #[tokio::test]
    async fn open_start_moves_the_start_of_a_running_recording_and_it_keeps_running() {
        let w = World::new().await;
        let (midnight, target) = today_starts();
        let timer = test_support::recording(&w.db, &w.project.id, midnight, None).await;

        let updated = w
            .service()
            .update(
                &timer.id,
                patch(|p| p.open_start = Some(target)),
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;

        assert_eq!(
            updated.spans,
            [RecordingSpan {
                id: timer.spans[0].id.clone(),
                start: target,
                end: None,
            }]
        );
        assert_eq!(w.rows().await, [updated]);
        let label = if target == midnight {
            "Edit recording"
        } else {
            "Resize recording"
        };
        assert_eq!(w.status().await, undo_label(label));
    }

    #[tokio::test]
    async fn undo_restores_the_start_of_a_running_recording() {
        let w = World::new().await;
        let (midnight, target) = today_starts();
        let timer = test_support::recording(&w.db, &w.project.id, midnight, None).await;
        w.service()
            .update(
                &timer.id,
                patch(|p| p.open_start = Some(target)),
                Overlap::Cancel,
            )
            .await
            .unwrap();

        w.service().undo().await.unwrap();

        assert_eq!(w.rows().await, [timer]);
    }

    #[tokio::test]
    async fn open_start_in_the_future_or_before_midnight_is_refused_on_the_start_field() {
        let w = World::new().await;
        let (midnight, _) = today_starts();
        let timer = test_support::recording(&w.db, &w.project.id, midnight, None).await;
        let cases = [
            (
                "a day ahead",
                now_ms() + 24 * HOUR,
                "A timer can't start in the future",
            ),
            (
                "before midnight",
                midnight - 30 * MIN,
                "A timer can't start before midnight",
            ),
        ];

        for (name, start, message) in cases {
            let err = w
                .service()
                .update(
                    &timer.id,
                    patch(|p| p.open_start = Some(start)),
                    Overlap::Cancel,
                )
                .await
                .unwrap_err();
            let field = err.field().unwrap();
            assert_eq!((field.path, field.message), ("start", message), "{name}");
        }

        assert_eq!(w.rows().await, [timer]);
        assert_eq!(w.status().await, no_status());
    }

    #[tokio::test]
    async fn open_start_on_a_finished_recording_is_refused_on_the_open_start_field() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;

        let err = w
            .service()
            .update(
                &saved.id,
                patch(|p| p.open_start = Some(at(9, 0))),
                Overlap::Cancel,
            )
            .await
            .unwrap_err();

        assert_eq!(field_of(&err).as_deref(), Some("openStart"));
        assert_eq!(w.rows().await, [saved]);
    }

    #[tokio::test]
    async fn nudge_span_moves_the_start_of_the_open_span_and_leaves_it_open() {
        let w = World::new().await;
        let (midnight, target) = today_starts();
        let start = target.max(midnight + 10 * MIN);
        let timer = test_support::recording(&w.db, &w.project.id, start, None).await;

        let nudged = w
            .service()
            .nudge_span(&timer.id, &timer.spans[0].id, -1, 0, Overlap::Cancel)
            .await
            .unwrap()
            .value;

        assert_eq!(
            nudged.spans,
            [RecordingSpan {
                id: timer.spans[0].id.clone(),
                start: snap(start, 5) - 5 * MIN,
                end: None,
            }]
        );
        assert_eq!(w.rows().await, [nudged]);
    }

    #[tokio::test]
    async fn nudge_span_refuses_to_move_the_end_of_the_open_span() {
        let w = World::new().await;
        let timer = test_support::recording(&w.db, &w.project.id, at(10, 0), None).await;

        for (start_steps, end_steps) in [(0, 1), (1, 1)] {
            let err = w
                .service()
                .nudge_span(
                    &timer.id,
                    &timer.spans[0].id,
                    start_steps,
                    end_steps,
                    Overlap::Cancel,
                )
                .await
                .unwrap_err();

            assert!(
                matches!(&err, AppError::InvalidInput(m) if m == "The timer's end moves when it stops"),
                "({start_steps}, {end_steps})"
            );
        }
        assert_eq!(w.rows().await, [timer]);
        assert_eq!(w.status().await, no_status());
    }

    #[tokio::test]
    async fn drag_span_on_the_open_span_moves_only_the_start() {
        let w = World::new().await;
        let (midnight, target) = today_starts();
        let timer = test_support::recording(&w.db, &w.project.id, midnight, None).await;

        let dragged = w
            .service()
            .drag_span(
                &timer.id,
                &timer.spans[0].id,
                span(target, target + HOUR),
                Some(SnapEdge::Start),
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;

        assert_eq!(
            dragged.spans,
            [RecordingSpan {
                id: timer.spans[0].id.clone(),
                start: target,
                end: None,
            }]
        );
    }

    #[tokio::test]
    async fn drag_span_on_the_open_spans_end_or_whole_is_refused() {
        let w = World::new().await;
        let timer = test_support::recording(&w.db, &w.project.id, at(10, 0), None).await;

        for (name, moving) in [("whole", None), ("end", Some(SnapEdge::End))] {
            let err = w
                .service()
                .drag_span(
                    &timer.id,
                    &timer.spans[0].id,
                    span(at(10, 30), at(11, 30)),
                    moving,
                    Overlap::Cancel,
                )
                .await
                .unwrap_err();

            assert!(
                matches!(&err, AppError::InvalidInput(m) if m == "The timer's end moves when it stops"),
                "{name}"
            );
        }
        assert_eq!(w.rows().await, [timer]);
    }

    #[tokio::test]
    async fn add_span_before_the_open_start_joins_a_running_recording() {
        let w = World::new().await;
        let timer = test_support::recording(&w.db, &w.project.id, at(10, 0), None).await;

        let added = w
            .service()
            .add_span(&timer.id, span(at(8, 0), at(9, 0)), Overlap::Cancel)
            .await
            .unwrap()
            .value;

        assert_eq!(added.spans.len(), 2);
        assert_eq!(
            (added.spans[0].start, added.spans[0].end),
            (at(8, 0), Some(at(9, 0)))
        );
        assert_eq!(added.spans[1], timer.spans[0]);
    }

    #[tokio::test]
    async fn add_span_after_the_open_start_is_refused_on_the_spans_field() {
        let w = World::new().await;
        let timer = test_support::recording(&w.db, &w.project.id, at(10, 0), None).await;

        let err = w
            .service()
            .add_span(&timer.id, span(at(12, 0), at(13, 0)), Overlap::Cancel)
            .await
            .unwrap_err();

        assert_eq!(field_of(&err).as_deref(), Some("spans"));
        assert_eq!(w.rows().await, [timer]);
    }

    #[tokio::test]
    async fn a_closed_span_of_a_running_recording_can_be_removed_and_split() {
        let w = World::new().await;
        let timer = running_with(&w, &[(at(8, 0), at(9, 0))], at(10, 0)).await;
        let open = timer.spans[1].clone();

        let split = w
            .service()
            .split_span(&timer.id, &timer.spans[0].id, at(8, 30))
            .await
            .unwrap()
            .value;
        assert_eq!(split.spans.len(), 3);
        assert_eq!(split.spans[2], open);

        let removed = w
            .service()
            .remove_span(&timer.id, &timer.spans[0].id)
            .await
            .unwrap()
            .value
            .unwrap();
        assert_eq!(removed.spans.len(), 2);
        assert_eq!(
            (removed.spans[0].start, removed.spans[0].end),
            (at(8, 30), Some(at(9, 0)))
        );
        assert_eq!(removed.spans[1], open);
    }

    #[tokio::test]
    async fn update_span_on_the_open_span_says_the_end_moves_when_it_stops() {
        let w = World::new().await;
        let timer = test_support::recording(&w.db, &w.project.id, at(10, 0), None).await;

        let err = w
            .service()
            .update_span(
                &timer.id,
                &timer.spans[0].id,
                span(at(12, 0), at(13, 0)),
                Overlap::Cancel,
            )
            .await
            .unwrap_err();

        assert!(
            matches!(&err, AppError::InvalidInput(m) if m == "The timer's end moves when it stops")
        );
        assert_eq!(w.rows().await, [timer]);
    }
}
