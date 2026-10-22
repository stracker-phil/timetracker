//! Moving and copying a recording, or one of its spans, to another day.

use super::checks::closed_span;
use super::{Change, Overlap, RecordingInput, Recordings, Span};
use crate::db::recordings::{self, Recording, RecordingPatch};
use crate::db::time::{Day, same_time_on};
use crate::error::AppResult;

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
    let offset = day.date() - Day::of(recording.start()).date();
    recording
        .spans
        .iter()
        .map(|span| {
            let span = closed_span(span)?;
            Ok(span_on_day(
                span,
                Day::from_date(Day::of(span.start).date() + offset),
            ))
        })
        .collect()
}

/// The span `span_id` of `recording` on `day`.
fn moved_span(recording: &Recording, span_id: &str, day: Day) -> AppResult<Span> {
    Ok(span_on_day(closed_span(recording.span(span_id)?)?, day))
}

impl Recordings<'_> {
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
}

#[cfg(test)]
mod tests {
    use chrono::{Local, NaiveTime};

    use super::*;
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
}
