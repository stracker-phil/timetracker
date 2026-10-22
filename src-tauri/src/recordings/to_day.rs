//! Moving and copying a recording, or its part on one day, to another day.

use chrono::TimeDelta;

use super::checks::closed_span;
use super::{Change, Overlap, RecordingInput, Recordings, Span};
use crate::db::recordings::{self, Recording, RecordingPatch};
use crate::db::time::{Day, same_time_on};
use crate::error::{AppError, AppResult};

/// `span` on `day`: the same local start time, the same length.
pub(super) fn span_on_day(span: Span, day: Day) -> Span {
    let start = same_time_on(span.start, day);
    Span {
        start,
        end: start + (span.end - span.start),
    }
}

fn touches(span: Span, day: Day) -> bool {
    let (start, end) = day.bounds();
    span.start < end && span.end > start
}

/// `span` `days` later, at its own local start time and length.
fn shifted(span: Span, days: TimeDelta) -> Span {
    span_on_day(span, Day::from_date(Day::of(span.start).date() + days))
}

/// Every span of `recording`, `true` with the ones that move: those touching `from` (all of them without),
/// shifted by the days between `from` (else the first span's day) and `day`.
fn placed_on_day(
    recording: &Recording,
    from: Option<Day>,
    day: Day,
) -> AppResult<Vec<(Span, bool)>> {
    let origin = from.unwrap_or_else(|| Day::of(recording.start()));
    let days = day.date() - origin.date();
    let placed = recording
        .spans
        .iter()
        .map(|s| {
            let span = closed_span(s)?;
            let moves = from.is_none_or(|from| touches(span, from));
            Ok((if moves { shifted(span, days) } else { span }, moves))
        })
        .collect::<AppResult<Vec<_>>>()?;
    if !placed.iter().any(|&(_, moved)| moved) {
        return Err(AppError::InvalidInput(format!(
            "The recording has no time on {origin}"
        )));
    }
    Ok(placed)
}

impl Recordings<'_> {
    /// Moves the recording's spans on `from` (all of its spans without) to `day` at the same local times,
    /// keeping their lengths; see `placed_on_day`.
    pub async fn move_to_day(
        &self,
        id: &str,
        from: Option<Day>,
        day: Day,
        overlap: Overlap,
    ) -> AppResult<Change<Recording>> {
        let current = recordings::get(self.db, id).await?;
        let spans = placed_on_day(&current, from, day)?;
        let patch = RecordingPatch {
            spans: Some(spans.into_iter().map(|(span, _)| span).collect()),
            ..RecordingPatch::default()
        };
        self.update(id, patch, overlap).await
    }

    /// A new recording like `id` on `day`, of its spans on `from` (all of them without), placed as
    /// `move_to_day` places them.
    pub async fn copy_to_day(
        &self,
        id: &str,
        from: Option<Day>,
        day: Day,
        overlap: Overlap,
    ) -> AppResult<Change<Recording>> {
        let original = recordings::get(self.db, id).await?;
        let spans = placed_on_day(&original, from, day)?;
        let input = RecordingInput {
            spans: spans
                .into_iter()
                .filter_map(|(span, moved)| moved.then_some(span))
                .collect(),
            project_id: original.project_id,
            note: original.note,
            tag_id: original.tag_id,
        };
        self.create(input, overlap).await
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Local, NaiveTime};

    use super::*;
    use crate::db::recordings::RecordingSpan;
    use crate::db::time::Ms;
    use crate::error::{AppError, ErrorKind};
    use crate::recordings::testing::*;
    use crate::test_support;

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
        assert_eq!(w.status().await, undo_label("Record 1h 30m for Acme"));
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

    fn span_of<'a>(recording: &'a Recording, id: &str) -> &'a RecordingSpan {
        recording
            .spans
            .iter()
            .find(|s| s.id == id)
            .expect("span in recording")
    }

    fn local_time(ms: Ms) -> NaiveTime {
        chrono::DateTime::from_timestamp_millis(ms)
            .unwrap()
            .with_timezone(&Local)
            .time()
    }

    #[tokio::test]
    async fn move_to_day_without_from_shifts_every_span_by_the_same_days_at_its_local_time() {
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
    async fn move_to_day_with_from_moves_only_the_spans_touching_that_day() {
        let w = World::new().await;
        let saved = w
            .create_spans(&[
                (at(10, 0), at(11, 0)),
                (at(14, 0) + NEXT_DAY, at(15, 0) + NEXT_DAY),
            ])
            .await;
        let target = Day::parse("2026-11-03").unwrap();

        let moved = w
            .service()
            .move_to_day(&saved.id, Some(next_day()), target, Overlap::Cancel)
            .await
            .unwrap()
            .value;

        let (stayed, shifted) = (
            span_of(&moved, &saved.spans[0].id),
            span_of(&moved, &saved.spans[1].id),
        );
        assert_eq!(moved.spans.len(), 2);
        assert_eq!(*stayed, saved.spans[0]);
        assert_eq!(Day::of(shifted.start), target);
        assert_eq!(local_time(shifted.start), local_time(saved.spans[1].start));
        assert_eq!(shifted.end.unwrap() - shifted.start, HOUR);
        assert_eq!(w.rows().await, [moved]);
        assert_eq!(w.status().await, undo_label("Move recording"));
    }

    #[tokio::test]
    async fn move_to_day_with_from_also_moves_a_span_crossing_midnight_into_that_day() {
        let w = World::new().await;
        let (before, mid, after) = midnight();
        let saved = w
            .create_spans(&[(at(10, 0), at(10, 30)), (mid - HOUR, mid + HOUR)])
            .await;
        let target = Day::parse("2026-11-03").unwrap();

        let moved = w
            .service()
            .move_to_day(&saved.id, Some(after), target, Overlap::Cancel)
            .await
            .unwrap()
            .value;

        let crossing = span_of(&moved, &saved.spans[1].id);
        assert_eq!(*span_of(&moved, &saved.spans[0].id), saved.spans[0]);
        assert_eq!(Day::of(crossing.start), target.prev());
        assert_eq!(local_time(crossing.start), local_time(saved.spans[1].start));
        assert_eq!(crossing.end.unwrap() - crossing.start, 2 * HOUR);
        assert_eq!(Day::of(saved.spans[0].start), before);
    }

    #[tokio::test]
    async fn move_to_day_with_from_follows_the_overlap_mode_on_the_target_day() {
        let w = World::new().await;
        let saved = w
            .create_spans(&[
                (at(9, 0), at(10, 0)),
                (at(9, 0) + NEXT_DAY, at(10, 0) + NEXT_DAY),
            ])
            .await;
        let target = Day::of(at(9, 0)).next().next();
        let blocker = test_support::recording(
            &w.db,
            &w.project.id,
            at(9, 30) + 2 * NEXT_DAY,
            Some(at(10, 30) + 2 * NEXT_DAY),
        )
        .await;

        let err = w
            .service()
            .move_to_day(&saved.id, Some(next_day()), target, Overlap::Cancel)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Overlap);
        assert_eq!(w.rows().await, [saved.clone(), blocker]);

        w.service()
            .move_to_day(&saved.id, Some(next_day()), target, Overlap::Replace)
            .await
            .unwrap();
        assert_eq!(
            spans(&w.rows().await),
            [
                (at(9, 0), at(10, 0)),
                (at(9, 0) + 2 * NEXT_DAY, at(10, 0) + 2 * NEXT_DAY),
                (at(10, 0) + 2 * NEXT_DAY, at(10, 30) + 2 * NEXT_DAY)
            ]
        );
    }

    #[tokio::test]
    async fn moving_or_copying_with_a_from_day_no_span_touches_is_invalid_input() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;
        let empty_day = Day::parse("2026-01-01").unwrap();

        let moved = w
            .service()
            .move_to_day(&saved.id, Some(empty_day), next_day(), Overlap::Cancel)
            .await
            .unwrap_err();
        let copied = w
            .service()
            .copy_to_day(&saved.id, Some(empty_day), next_day(), Overlap::Cancel)
            .await
            .unwrap_err();

        for err in [moved, copied] {
            assert_eq!(err.kind(), ErrorKind::InvalidInput);
        }
        assert_eq!(w.rows().await, [saved]);
    }

    #[tokio::test]
    async fn copy_to_day_with_from_creates_a_recording_of_only_the_spans_touching_that_day() {
        let w = World::new().await;
        let original = w
            .service()
            .create(
                RecordingInput {
                    note: "copy me".into(),
                    tag_id: Some(w.alpha.id.clone()),
                    ..input_of(
                        &w,
                        &[
                            (at(10, 0), at(11, 0)),
                            (at(14, 0), at(15, 30)),
                            (at(10, 0) + NEXT_DAY, at(11, 0) + NEXT_DAY),
                        ],
                    )
                },
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;
        let target = Day::of(at(10, 0)).next().next();

        let copy = w
            .service()
            .copy_to_day(
                &original.id,
                Some(Day::of(at(10, 0))),
                target,
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;

        assert_ne!(copy.id, original.id);
        assert_eq!(
            times(&copy),
            [
                (at(10, 0) + 2 * NEXT_DAY, at(11, 0) + 2 * NEXT_DAY),
                (at(14, 0) + 2 * NEXT_DAY, at(15, 30) + 2 * NEXT_DAY)
            ]
        );
        assert_ne!(copy.spans[0].id, original.spans[1].id);
        assert_eq!(
            (&copy.project_id, &copy.note, &copy.tag_id),
            (&original.project_id, &original.note, &original.tag_id)
        );
        assert_eq!(w.rows().await, [original, copy]);
        assert_eq!(w.status().await, undo_label("Record 2h 30m for Acme"));
    }

    #[tokio::test]
    async fn copy_to_day_without_from_copies_every_span_into_a_new_recording() {
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
    async fn moving_or_copying_the_running_timer_from_its_day_asks_to_stop_it() {
        let w = World::new().await;
        let timer = test_support::recording(&w.db, &w.project.id, at(10, 0), None).await;
        let from = Some(Day::of(at(10, 0)));

        let moved = w
            .service()
            .move_to_day(&timer.id, from, next_day(), Overlap::Cancel)
            .await
            .unwrap_err();
        let copied = w
            .service()
            .copy_to_day(&timer.id, from, next_day(), Overlap::Cancel)
            .await
            .unwrap_err();

        for err in [moved, copied] {
            assert!(matches!(&err, AppError::InvalidInput(m) if m == "Stop the timer first"));
        }
        assert_eq!(w.rows().await, [timer]);
    }
}
