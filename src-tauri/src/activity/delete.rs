//! Deleting activity and away spans in a range, with a short undo window. Spans that cross the
//! range go whole (activity is never trimmed, ADR 0005); recordings are never touched.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::db::Db;
use crate::db::activities::{self, ActivityRow};
use crate::db::away::{self, AwayRow};
use crate::db::time::{Day, Ms};
use crate::error::{AppError, AppResult};
use crate::lock::MutexExt;
use crate::tracker::Tracker;

pub const UNDO_WINDOW_MS: Ms = 30_000;
/// A leap year, so any calendar year fits.
const MAX_RANGE_MS: Ms = 366 * 24 * 3_600_000;

/// What a delete removed, kept for its undo.
#[derive(Debug, Clone)]
pub struct DeletedActivity {
    pub activities: Vec<ActivityRow>,
    pub away: Vec<AwayRow>,
    pub from: Ms,
    pub to: Ms,
    pub at: Ms,
}

/// How many spans a delete removed or an undo restored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct DeleteResult {
    pub activities: usize,
    pub away: usize,
    /// When the undo window of the delete closes.
    pub undo_until: Ms,
}

/// How many spans deleting a range would remove.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ActivityCount {
    pub activities: usize,
    pub away: usize,
}

/// The last delete of this run; a new one replaces it.
#[derive(Default)]
pub struct DeleteBuffer {
    last: Mutex<Option<DeletedActivity>>,
}

/// A delete or undo and the local days whose activity changed.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub result: DeleteResult,
    pub days: Vec<Day>,
}

fn validate(from: Ms, to: Ms) -> AppResult<()> {
    if from >= to {
        return Err(AppError::InvalidInput(
            "The range must end after it starts".into(),
        ));
    }
    if to - from > MAX_RANGE_MS {
        return Err(AppError::InvalidInput(
            "Delete at most one year at a time".into(),
        ));
    }
    Ok(())
}

pub async fn count(db: &Db, from: Ms, to: Ms) -> AppResult<ActivityCount> {
    validate(from, to)?;
    Ok(ActivityCount {
        activities: activities::count_range(db, from, to).await? as usize,
        away: away::count_range(db, from, to).await? as usize,
    })
}

/// Deletes every activity and away span that shares time with `[from, to)` in one transaction and
/// keeps them for `undo`. The tracker forgets its open span when it was deleted.
pub async fn delete(
    db: &Db,
    buffer: &DeleteBuffer,
    tracker: Option<&Tracker>,
    from: Ms,
    to: Ms,
    now: Ms,
) -> AppResult<Outcome> {
    validate(from, to)?;
    let mut tx = db.pool().begin().await?;
    let deleted_activities = activities::take_range_on(&mut tx, from, to).await?;
    let deleted_away = away::take_range_on(&mut tx, from, to).await?;
    tx.commit().await?;
    if let Some(tracker) = tracker {
        let open: Vec<i64> = deleted_activities
            .iter()
            .filter(|row| row.end.is_none())
            .map(|row| row.id)
            .collect();
        tracker.reset_span(&open, now).await;
    }
    let deleted = DeletedActivity {
        activities: deleted_activities,
        away: deleted_away,
        from,
        to,
        at: now,
    };
    let outcome = outcome(&deleted);
    *buffer.last.lock_unpoisoned() = Some(deleted);
    Ok(outcome)
}

/// Restores the last delete under the original ids, within `UNDO_WINDOW_MS` of it. Rows that were
/// open end where they were deleted: the tracker has opened a new span since.
pub async fn undo(db: &Db, buffer: &DeleteBuffer, now: Ms) -> AppResult<Outcome> {
    let deleted = {
        let last = buffer.last.lock_unpoisoned().take();
        last.filter(|d| now <= d.at + UNDO_WINDOW_MS)
            .ok_or_else(|| AppError::NotFound("Nothing to undo".into()))?
    };
    let outcome = outcome(&deleted);
    if let Err(e) = restore(db, &deleted).await {
        *buffer.last.lock_unpoisoned() = Some(deleted);
        return Err(e);
    }
    Ok(outcome)
}

async fn restore(db: &Db, deleted: &DeletedActivity) -> AppResult<()> {
    let mut tx = db.pool().begin().await?;
    for row in &deleted.activities {
        let end = row.end.or(Some(deleted.at.max(row.start)));
        activities::restore_on(&mut tx, &ActivityRow { end, ..row.clone() }).await?;
    }
    for row in &deleted.away {
        let end = row.end.or(Some(deleted.at.max(row.start)));
        away::restore_on(&mut tx, &AwayRow { end, ..row.clone() }).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Every local day from the earliest start to the latest end of the range and its rows.
fn outcome(deleted: &DeletedActivity) -> Outcome {
    let spans = deleted
        .activities
        .iter()
        .map(|row| (row.start, row.end))
        .chain(deleted.away.iter().map(|row| (row.start, row.end)));
    let (mut first, mut last) = (deleted.from, deleted.to);
    for (start, end) in spans {
        first = first.min(start);
        last = last.max(end.unwrap_or(deleted.at));
    }
    Outcome {
        result: DeleteResult {
            activities: deleted.activities.len(),
            away: deleted.away.len(),
            undo_until: deleted.at + UNDO_WINDOW_MS,
        },
        // `last` is exclusive: a range ending at midnight does not touch the next day.
        days: Day::range(&Day::of(first), &Day::of((last - 1).max(first))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::fake::FakeSource;
    use crate::db::away::{AwayReason, NewAway};
    use crate::store::settings::TrackerSettings;
    use crate::test_support::{activity, project, recording};

    const T: Ms = 1_800_000_000_000;
    const HOUR: Ms = 3_600_000;

    async fn away_span(db: &Db, start: Ms, end: Option<Ms>) -> AwayRow {
        let id = away::insert(
            db,
            NewAway {
                start,
                reason: AwayReason::Idle,
            },
        )
        .await
        .unwrap();
        if let Some(end) = end {
            away::close_open(db, end).await.unwrap();
        }
        let rows = away::list_for_range(db, 0, Ms::MAX).await.unwrap();
        rows.into_iter().find(|row| row.id == id).unwrap()
    }

    async fn activity_ids(db: &Db) -> Vec<i64> {
        activities::list_for_range(db, 0, Ms::MAX)
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.id)
            .collect()
    }

    async fn away_ids(db: &Db) -> Vec<i64> {
        away::list_for_range(db, 0, Ms::MAX)
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.id)
            .collect()
    }

    /// Activity `before`, `crossing` (the range start), `inside`, `after` and an away span for each
    /// of them, around the range `[T + 2h, T + 4h)`.
    struct Seeded {
        activities: [ActivityRow; 4],
        away: [AwayRow; 4],
    }

    async fn seeded(db: &Db) -> Seeded {
        let spans = [
            (T, T + HOUR),
            (T + HOUR, T + 3 * HOUR),
            (T + 3 * HOUR, T + 3 * HOUR + HOUR / 2),
            (T + 5 * HOUR, T + 6 * HOUR),
        ];
        let mut rows = Vec::new();
        let mut aways = Vec::new();
        for (index, (start, end)) in spans.into_iter().enumerate() {
            rows.push(activity(db, start, Some(end), "Code", &format!("a{index}")).await);
            aways.push(away_span(db, start, Some(end)).await);
        }
        Seeded {
            activities: rows.try_into().unwrap(),
            away: aways.try_into().unwrap(),
        }
    }

    #[tokio::test]
    async fn the_count_matches_what_a_delete_removes() {
        let db = Db::open_memory().await.unwrap();
        seeded(&db).await;
        let (from, to) = (T + 2 * HOUR, T + 4 * HOUR);

        let counted = count(&db, from, to).await.unwrap();
        let outcome = delete(&db, &DeleteBuffer::default(), None, from, to, T + 7 * HOUR)
            .await
            .unwrap();

        assert_eq!(
            counted,
            ActivityCount {
                activities: 2,
                away: 2
            }
        );
        assert_eq!(
            (outcome.result.activities, outcome.result.away),
            (counted.activities, counted.away)
        );
    }

    #[tokio::test]
    async fn the_count_of_a_range_without_activity_is_zero() {
        let db = Db::open_memory().await.unwrap();
        seeded(&db).await;

        assert_eq!(
            count(&db, T + 10 * HOUR, T + 11 * HOUR).await.unwrap(),
            ActivityCount {
                activities: 0,
                away: 0
            }
        );
    }

    #[tokio::test]
    async fn a_delete_removes_spans_sharing_time_whole_and_keeps_the_rest() {
        let db = Db::open_memory().await.unwrap();
        let seeded = seeded(&db).await;
        let [before, _, _, after] = &seeded.activities;
        let [away_before, _, _, away_after] = &seeded.away;

        delete(
            &db,
            &DeleteBuffer::default(),
            None,
            T + 2 * HOUR,
            T + 4 * HOUR,
            T + 7 * HOUR,
        )
        .await
        .unwrap();

        assert_eq!(activity_ids(&db).await, [before.id, after.id]);
        assert_eq!(away_ids(&db).await, [away_before.id, away_after.id]);
    }

    #[tokio::test]
    async fn spans_that_only_touch_the_range_stay() {
        let db = Db::open_memory().await.unwrap();
        let ends_at_start = activity(&db, T, Some(T + HOUR), "Code", "a").await;
        let starts_at_end = activity(&db, T + 2 * HOUR, Some(T + 3 * HOUR), "Code", "b").await;
        let away_ends = away_span(&db, T, Some(T + HOUR)).await;
        let away_starts = away_span(&db, T + 2 * HOUR, Some(T + 3 * HOUR)).await;

        let range = (T + HOUR, T + 2 * HOUR);
        assert_eq!(
            count(&db, range.0, range.1).await.unwrap(),
            ActivityCount {
                activities: 0,
                away: 0
            }
        );
        delete(
            &db,
            &DeleteBuffer::default(),
            None,
            range.0,
            range.1,
            T + 4 * HOUR,
        )
        .await
        .unwrap();

        assert_eq!(
            activity_ids(&db).await,
            [ends_at_start.id, starts_at_end.id]
        );
        assert_eq!(away_ids(&db).await, [away_ends.id, away_starts.id]);
    }

    #[tokio::test]
    async fn an_open_span_in_the_range_is_deleted() {
        let db = Db::open_memory().await.unwrap();
        activity(&db, T + HOUR, None, "Code", "open").await;
        away_span(&db, T + HOUR, None).await;

        let outcome = delete(
            &db,
            &DeleteBuffer::default(),
            None,
            T,
            T + 2 * HOUR,
            T + 3 * HOUR,
        )
        .await
        .unwrap();

        assert_eq!((outcome.result.activities, outcome.result.away), (1, 1));
        assert_eq!(activity_ids(&db).await, Vec::<i64>::new());
        assert_eq!(away_ids(&db).await, Vec::<i64>::new());
    }

    #[tokio::test]
    async fn recordings_are_not_touched() {
        let db = Db::open_memory().await.unwrap();
        let project = project(&db, "Acme").await;
        let kept = recording(&db, &project.id, T + HOUR, Some(T + 3 * HOUR)).await;
        activity(&db, T + HOUR, Some(T + 3 * HOUR), "Code", "a").await;

        delete(
            &db,
            &DeleteBuffer::default(),
            None,
            T,
            T + 4 * HOUR,
            T + 5 * HOUR,
        )
        .await
        .unwrap();

        assert_eq!(
            crate::db::recordings::get(&db, &kept.id).await.unwrap(),
            kept
        );
    }

    #[tokio::test]
    async fn ranges_that_are_empty_inverted_or_longer_than_a_year_are_refused() {
        let db = Db::open_memory().await.unwrap();
        let row = activity(&db, T, Some(T + HOUR), "Code", "a").await;
        let year = 366 * 24 * HOUR;

        for (from, to) in [(T, T), (T + HOUR, T), (T, T + year + 1)] {
            let buffer = DeleteBuffer::default();
            let counted = count(&db, from, to).await.unwrap_err();
            let deleted = delete(&db, &buffer, None, from, to, T).await.unwrap_err();
            assert!(
                matches!(counted, AppError::InvalidInput(_)),
                "{from} {to}: {counted:?}"
            );
            assert!(
                matches!(deleted, AppError::InvalidInput(_)),
                "{from} {to}: {deleted:?}"
            );
            assert_eq!(activity_ids(&db).await, [row.id], "{from} {to}");
        }
        assert!(count(&db, T, T + year).await.is_ok());
    }

    #[tokio::test]
    async fn an_undo_restores_the_deleted_spans_unchanged() {
        let db = Db::open_memory().await.unwrap();
        let seeded = seeded(&db).await;
        let buffer = DeleteBuffer::default();
        delete(&db, &buffer, None, T + 2 * HOUR, T + 4 * HOUR, T + 7 * HOUR)
            .await
            .unwrap();

        let outcome = undo(&db, &buffer, T + 7 * HOUR + 10_000).await.unwrap();

        assert_eq!((outcome.result.activities, outcome.result.away), (2, 2));
        assert_eq!(
            activities::list_for_range(&db, 0, Ms::MAX).await.unwrap(),
            seeded.activities
        );
        assert_eq!(
            away::list_for_range(&db, 0, Ms::MAX).await.unwrap(),
            seeded.away
        );
    }

    #[tokio::test]
    async fn an_undo_is_possible_until_the_window_closes_and_only_once() {
        for (after, restored) in [
            (0, true),
            (UNDO_WINDOW_MS, true),
            (UNDO_WINDOW_MS + 1, false),
        ] {
            let db = Db::open_memory().await.unwrap();
            activity(&db, T, Some(T + HOUR), "Code", "a").await;
            let buffer = DeleteBuffer::default();
            delete(&db, &buffer, None, T, T + HOUR, T + 2 * HOUR)
                .await
                .unwrap();

            let result = undo(&db, &buffer, T + 2 * HOUR + after).await;

            assert_eq!(result.is_ok(), restored, "{after}");
            assert_eq!(
                activity_ids(&db).await.len(),
                usize::from(restored),
                "{after}"
            );
            if !restored {
                assert_eq!(
                    result.unwrap_err(),
                    AppError::NotFound("Nothing to undo".into())
                );
            }
            assert_eq!(
                undo(&db, &buffer, T + 2 * HOUR).await.unwrap_err(),
                AppError::NotFound("Nothing to undo".into()),
                "{after}"
            );
        }
    }

    #[tokio::test]
    async fn an_undo_without_a_delete_is_not_found() {
        let db = Db::open_memory().await.unwrap();

        assert_eq!(
            undo(&db, &DeleteBuffer::default(), T).await.unwrap_err(),
            AppError::NotFound("Nothing to undo".into())
        );
    }

    #[tokio::test]
    async fn a_new_delete_replaces_the_one_to_undo() {
        let db = Db::open_memory().await.unwrap();
        activity(&db, T, Some(T + HOUR), "Code", "first").await;
        let second = activity(&db, T + 2 * HOUR, Some(T + 3 * HOUR), "Code", "second").await;
        let buffer = DeleteBuffer::default();
        delete(&db, &buffer, None, T, T + HOUR, T + 4 * HOUR)
            .await
            .unwrap();
        delete(&db, &buffer, None, T + 2 * HOUR, T + 3 * HOUR, T + 4 * HOUR)
            .await
            .unwrap();

        undo(&db, &buffer, T + 4 * HOUR).await.unwrap();

        assert_eq!(activity_ids(&db).await, [second.id]);
    }

    #[tokio::test]
    async fn spans_open_at_the_delete_come_back_closed_at_the_delete() {
        let db = Db::open_memory().await.unwrap();
        let open = activity(&db, T, None, "Code", "open").await;
        let away_open = away_span(&db, T + HOUR, None).await;
        let buffer = DeleteBuffer::default();
        let deleted_at = T + 2 * HOUR;
        delete(&db, &buffer, None, T, T + 3 * HOUR, deleted_at)
            .await
            .unwrap();

        undo(&db, &buffer, deleted_at + 1_000).await.unwrap();

        assert_eq!(
            activities::list_for_range(&db, 0, Ms::MAX).await.unwrap(),
            [ActivityRow {
                end: Some(deleted_at),
                ..open
            }]
        );
        assert_eq!(
            away::list_for_range(&db, 0, Ms::MAX).await.unwrap(),
            [AwayRow {
                end: Some(deleted_at),
                ..away_open
            }]
        );
    }

    #[tokio::test]
    async fn an_open_span_that_started_after_the_delete_time_comes_back_empty_not_inverted() {
        let db = Db::open_memory().await.unwrap();
        let open = activity(&db, T + HOUR, None, "Code", "open").await;
        let buffer = DeleteBuffer::default();
        delete(&db, &buffer, None, T, T + 2 * HOUR, T)
            .await
            .unwrap();

        undo(&db, &buffer, T).await.unwrap();

        assert_eq!(
            activities::list_for_range(&db, 0, Ms::MAX).await.unwrap(),
            [ActivityRow {
                end: Some(open.start),
                ..open
            }]
        );
    }

    #[tokio::test]
    async fn the_result_reports_counts_and_when_the_undo_closes() {
        let db = Db::open_memory().await.unwrap();
        activity(&db, T, Some(T + HOUR), "Code", "a").await;
        away_span(&db, T, Some(T + HOUR)).await;
        let buffer = DeleteBuffer::default();

        let outcome = delete(&db, &buffer, None, T, T + HOUR, T + 2 * HOUR)
            .await
            .unwrap();

        assert_eq!(
            outcome.result,
            DeleteResult {
                activities: 1,
                away: 1,
                undo_until: T + 2 * HOUR + 30_000
            }
        );
        assert_eq!(
            undo(&db, &buffer, T + 2 * HOUR).await.unwrap().result,
            outcome.result
        );
    }

    #[tokio::test]
    async fn the_outcome_names_the_days_the_deleted_spans_touch() {
        let db = Db::open_memory().await.unwrap();
        let start = T - 50 * HOUR;
        activity(&db, start, Some(T + HOUR), "Code", "long").await;

        let outcome = delete(
            &db,
            &DeleteBuffer::default(),
            None,
            T,
            T + HOUR,
            T + 2 * HOUR,
        )
        .await
        .unwrap();

        assert_eq!(
            outcome.days,
            Day::range(&Day::of(start), &Day::of(T + HOUR - 1))
        );
        assert!(outcome.days.len() >= 3);
    }

    #[test]
    fn results_and_counts_serialize_in_camel_case() {
        assert_eq!(
            serde_json::to_value(DeleteResult {
                activities: 2,
                away: 1,
                undo_until: 5
            })
            .unwrap(),
            serde_json::json!({"activities": 2, "away": 1, "undoUntil": 5})
        );
        assert_eq!(
            serde_json::to_value(ActivityCount {
                activities: 2,
                away: 1
            })
            .unwrap(),
            serde_json::json!({"activities": 2, "away": 1})
        );
    }

    #[tokio::test]
    async fn deleting_the_running_span_makes_the_tracker_open_a_new_one() {
        let db = Db::open_memory().await.unwrap();
        let source = FakeSource::new();
        let tracker = Tracker::new(db.clone(), source.clone(), TrackerSettings::default(), T);
        source.show("Code", "main.rs");
        tracker.tick(T).await.unwrap();
        let running = tracker.status().current.unwrap();

        delete(
            &db,
            &DeleteBuffer::default(),
            Some(&tracker),
            T - HOUR,
            T + HOUR,
            T + 1_000,
        )
        .await
        .unwrap();
        assert_eq!(tracker.status().current, None);
        tracker.tick(T + 2_000).await.unwrap();

        let rows = activities::list_for_range(&db, 0, Ms::MAX).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_ne!(rows[0].id, running.id);
        assert_eq!((rows[0].start, rows[0].end), (T + 2_000, None));
        assert_eq!(tracker.status().current.as_ref(), Some(&rows[0]));
    }

    #[tokio::test]
    async fn deleting_other_spans_keeps_the_running_one() {
        let db = Db::open_memory().await.unwrap();
        let source = FakeSource::new();
        let tracker = Tracker::new(db.clone(), source.clone(), TrackerSettings::default(), T);
        source.show("Code", "main.rs");
        tracker.tick(T).await.unwrap();
        let running = tracker.status().current.unwrap();
        activity(&db, T - 3 * HOUR, Some(T - 2 * HOUR), "Code", "old").await;

        delete(
            &db,
            &DeleteBuffer::default(),
            Some(&tracker),
            T - 4 * HOUR,
            T - HOUR,
            T + 1_000,
        )
        .await
        .unwrap();

        assert_eq!(tracker.status().current.as_ref(), Some(&running));
        assert_eq!(activity_ids(&db).await, [running.id]);
    }
}
