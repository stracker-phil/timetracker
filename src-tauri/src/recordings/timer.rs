//! Timers: a recording without an end, started and stopped now. Both are ordinary undoable changes.

use super::ops::Op;
use super::{Change, MINUTE_MS, Recordings, check_tags, checked_note, snap, usable_project};
use crate::db::new_id;
use crate::db::recordings::{self, Recording};
use crate::db::time::Ms;
use crate::error::AppResult;

#[derive(Debug, Clone, PartialEq)]
pub struct TimerInput {
    pub project_id: String,
    pub note: String,
    pub tag_ids: Vec<String>,
}

/// Where a timer started at `start` ends when stopped at `now`: on the snap grid, and at least a minute
/// long, so a stopped timer never leaves an empty recording.
pub fn stop_time(start: Ms, now: Ms, snap_minutes: u32) -> Ms {
    snap(now, snap_minutes).max(start + MINUTE_MS)
}

impl Recordings<'_> {
    /// Starts timing now; a running timer stops at the instant the new one starts.
    pub async fn start_timer(&self, input: TimerInput, now: Ms) -> AppResult<Change<Recording>> {
        let mut undo = self.undo.lock().await;
        let note = checked_note(&input.note)?;
        let project = usable_project(self.db, &input.project_id, None).await?;
        check_tags(self.db, &input.tag_ids).await?;
        let running = recordings::running(self.db).await?;
        let start = match &running {
            Some(r) => stop_time(r.start, now, self.snap_minutes),
            None => self.free_start(now).await?,
        };
        let recording = Recording {
            id: new_id(),
            project_id: project.id,
            start,
            end: None,
            note,
            tag_ids: input.tag_ids,
            created_at: now,
            updated_at: now,
        };
        let label = format!("Start timer for {}", project.name);
        let create = Op::Create {
            recording: recording.clone(),
        };
        let op = match running {
            Some(before) => Op::Batch {
                label: label.clone(),
                ops: vec![stopped(before, start, now), create],
            },
            None => create,
        };
        self.commit(&mut undo, label, op, &recording.id).await
    }

    /// `now` on the snap grid; when that lies in the past, after any recording that ended since, so a
    /// timer started right after another recording never overlaps it.
    async fn free_start(&self, now: Ms) -> AppResult<Ms> {
        let snapped = snap(now, self.snap_minutes);
        if snapped >= now {
            return Ok(snapped);
        }
        let ended = recordings::overlapping(self.db, snapped, now, None).await?;
        Ok(ended
            .iter()
            .filter_map(|r| r.end.filter(|&end| end <= now))
            .fold(snapped, Ms::max))
    }

    /// Stops the running timer; `None` when none runs.
    pub async fn stop_timer(&self, now: Ms) -> AppResult<Change<Option<Recording>>> {
        let mut undo = self.undo.lock().await;
        let Some(running) = recordings::running(self.db).await? else {
            return Ok(Change {
                value: None,
                days: Vec::new(),
            });
        };
        let id = running.id.clone();
        let end = stop_time(running.start, now, self.snap_minutes);
        let change = self
            .commit(
                &mut undo,
                "Stop timer".into(),
                stopped(running, end, now),
                &id,
            )
            .await?;
        Ok(change.map(Some))
    }
}

fn stopped(before: Recording, end: Ms, now: Ms) -> Op {
    let after = Recording {
        end: Some(end),
        updated_at: now,
        ..before.clone()
    };
    Op::Update { before, after }
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;
    use crate::db::projects;
    use crate::db::time::Day;
    use crate::error::ErrorKind;

    fn timer(w: &World) -> TimerInput {
        TimerInput {
            project_id: w.project.id.clone(),
            note: String::new(),
            tag_ids: Vec::new(),
        }
    }

    #[test]
    fn a_timer_stops_on_the_snap_grid_and_at_least_a_minute_after_its_start() {
        let start = at(10, 0);
        let cases = [
            (5, at(10, 7) + 29_000, at(10, 5)),
            (5, at(10, 7) + 30_000, at(10, 10)),
            (5, at(10, 5), at(10, 5)),
            (15, at(10, 22), at(10, 15)),
            (15, at(10, 23), at(10, 30)),
            (5, at(10, 1), at(10, 1)),
            (5, at(10, 0) + 20_000, at(10, 1)),
            (5, at(9, 58), at(10, 1)),
            (1, at(10, 1), at(10, 1)),
            (1, at(10, 1) - 1, at(10, 1)),
            (1, at(10, 2), at(10, 2)),
        ];
        for (snap_minutes, now, expected) in cases {
            assert_eq!(
                stop_time(start, now, snap_minutes),
                expected,
                "{snap_minutes} min, now {now}"
            );
        }
    }

    #[tokio::test]
    async fn starting_a_timer_creates_a_running_recording_at_the_snapped_now() {
        let w = World::new().await;
        let input = TimerInput {
            note: "  Review  ".into(),
            tag_ids: vec![w.alpha.id.clone(), w.beta.id.clone()],
            ..timer(&w)
        };

        let change = w.service().start_timer(input, at(10, 3)).await.unwrap();

        let started = change.value;
        assert_eq!(started.project_id, w.project.id);
        assert_eq!((started.start, started.end), (at(10, 5), None));
        assert_eq!(started.note, "Review");
        assert_eq!(started.tag_ids, vec![w.alpha.id.clone(), w.beta.id.clone()]);
        assert_eq!(
            (started.created_at, started.updated_at),
            (at(10, 3), at(10, 3))
        );
        assert_eq!(change.days.first(), Some(&Day::of(at(10, 5))));
        assert_eq!(w.rows().await, vec![started]);
        assert_eq!(
            w.status().await.undo_label.as_deref(),
            Some("Start timer for Acme")
        );
    }

    #[tokio::test]
    async fn starting_a_timer_stops_the_running_one_where_the_new_one_starts() {
        let w = World::new().await;
        let first = w
            .service()
            .start_timer(timer(&w), at(10, 0))
            .await
            .unwrap()
            .value;
        let second_input = TimerInput {
            project_id: w.other.id.clone(),
            ..timer(&w)
        };

        let second = w
            .service()
            .start_timer(second_input, at(10, 32))
            .await
            .unwrap()
            .value;

        let rows = w.rows().await;
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (rows[0].id.as_str(), rows[0].end),
            (first.id.as_str(), Some(at(10, 30)))
        );
        assert_eq!(
            (rows[1].id.as_str(), rows[1].start, rows[1].end),
            (second.id.as_str(), at(10, 30), None)
        );
        assert_eq!(
            w.status().await.undo_label.as_deref(),
            Some("Start timer for Beta")
        );
    }

    #[tokio::test]
    async fn one_undo_removes_the_new_timer_and_the_first_runs_again() {
        let w = World::new().await;
        let first = w
            .service()
            .start_timer(timer(&w), at(10, 0))
            .await
            .unwrap()
            .value;
        w.service()
            .start_timer(timer(&w), at(10, 32))
            .await
            .unwrap();

        let undone = w.service().undo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Start timer for Acme"));
        assert_eq!(w.rows().await, vec![first]);
    }

    #[tokio::test]
    async fn a_timer_cannot_start_inside_a_recording_and_nothing_is_written() {
        let w = World::new().await;
        let existing = w.create(at(10, 0), at(11, 0)).await;

        let err = w
            .service()
            .start_timer(timer(&w), at(10, 30))
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::Overlap);
        assert_eq!(w.rows().await, vec![existing]);
        assert_eq!(
            w.status().await.undo_label.as_deref(),
            Some("Record 1 h 00 min for Acme")
        );
    }

    #[tokio::test]
    async fn a_timer_started_just_after_a_recording_ended_starts_where_it_ended() {
        let w = World::new().await;
        crate::test_support::recording(&w.db, &w.project.id, at(10, 0), Some(at(10, 31))).await;

        let started = w
            .service()
            .start_timer(timer(&w), at(10, 32))
            .await
            .unwrap()
            .value;

        assert_eq!((started.start, started.end), (at(10, 31), None));
    }

    #[tokio::test]
    async fn a_timer_starts_after_the_latest_of_several_recordings_that_ended_since_the_snapped_time()
     {
        let w = World::new().await;
        let p = &w.project.id;
        crate::test_support::recording(&w.db, p, at(10, 0), Some(at(10, 6))).await;
        crate::test_support::recording(&w.db, p, at(10, 6), Some(at(10, 6) + 30_000)).await;

        let started = w
            .service()
            .start_timer(timer(&w), at(10, 7))
            .await
            .unwrap()
            .value;

        assert_eq!((started.start, started.end), (at(10, 6) + 30_000, None));
    }

    #[tokio::test]
    async fn a_timer_without_earlier_recordings_starts_at_the_snapped_now() {
        let w = World::new().await;

        for (now, expected) in [(at(10, 7), at(10, 5)), (at(10, 8), at(10, 10))] {
            let started = w.service().start_timer(timer(&w), now).await.unwrap().value;
            assert_eq!(started.start, expected, "{now}");
            w.service().undo().await.unwrap();
        }
    }

    #[tokio::test]
    async fn a_timer_started_while_a_recording_still_covers_now_is_refused() {
        let w = World::new().await;
        let existing = w.create(at(10, 0), at(11, 0)).await;

        let err = w
            .service()
            .start_timer(timer(&w), at(10, 7))
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::Overlap);
        assert_eq!(w.rows().await, vec![existing]);
    }

    #[tokio::test]
    async fn a_timer_started_right_after_a_recording_on_the_grid_starts_at_the_grid() {
        let w = World::new().await;
        w.create(at(10, 0), at(10, 30)).await;

        let started = w
            .service()
            .start_timer(timer(&w), at(10, 32))
            .await
            .unwrap()
            .value;

        assert_eq!(started.start, at(10, 30));
    }

    #[tokio::test]
    async fn a_timer_note_with_a_line_break_is_refused_naming_the_note() {
        let w = World::new().await;
        let input = TimerInput {
            note: "a\nb".into(),
            ..timer(&w)
        };

        let err = w.service().start_timer(input, at(10, 0)).await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(field_of(&err), Some("note".into()));
        assert!(w.rows().await.is_empty());
    }

    #[tokio::test]
    async fn a_timer_cannot_start_for_an_archived_unknown_project_or_an_unknown_tag() {
        let w = World::new().await;
        projects::set_archived(&w.db, &w.other.id, true)
            .await
            .unwrap();
        let cases = [
            (
                "archived project",
                TimerInput {
                    project_id: w.other.id.clone(),
                    ..timer(&w)
                },
                ErrorKind::InvalidInput,
                Some("projectId".to_string()),
            ),
            (
                "unknown project",
                TimerInput {
                    project_id: "nope".into(),
                    ..timer(&w)
                },
                ErrorKind::NotFound,
                None,
            ),
            (
                "unknown tag",
                TimerInput {
                    tag_ids: vec!["nope".into()],
                    ..timer(&w)
                },
                ErrorKind::InvalidInput,
                Some("tagIds".to_string()),
            ),
        ];
        for (name, input, kind, field) in cases {
            let err = w.service().start_timer(input, at(10, 0)).await.unwrap_err();

            assert_eq!(err.kind(), kind, "{name}");
            assert_eq!(field_of(&err), field, "{name}");
        }
        assert!(w.rows().await.is_empty());
    }

    #[tokio::test]
    async fn stopping_without_a_timer_changes_nothing() {
        let w = World::new().await;
        w.create(at(10, 0), at(11, 0)).await;

        let change = w.service().stop_timer(at(12, 0)).await.unwrap();

        assert_eq!(change.value, None);
        assert!(change.days.is_empty());
        assert_eq!(
            w.status().await.undo_label.as_deref(),
            Some("Record 1 h 00 min for Acme")
        );
    }

    #[tokio::test]
    async fn stopping_ends_the_timer_on_the_grid_and_labels_the_change() {
        let w = World::new().await;
        w.service().start_timer(timer(&w), at(10, 0)).await.unwrap();

        let change = w.service().stop_timer(at(10, 47)).await.unwrap();

        let stopped = change.value.expect("a stopped recording");
        assert_eq!((stopped.start, stopped.end), (at(10, 0), Some(at(10, 45))));
        assert_eq!(stopped.updated_at, at(10, 47));
        assert_eq!(change.days.first(), Some(&Day::of(at(10, 0))));
        assert_eq!(w.rows().await, vec![stopped]);
        assert_eq!(w.status().await.undo_label.as_deref(), Some("Stop timer"));
    }

    #[tokio::test]
    async fn a_timer_stopped_after_twenty_seconds_lasts_one_minute() {
        let w = World::new().await;
        w.service().start_timer(timer(&w), at(10, 0)).await.unwrap();

        let stopped = w
            .service()
            .stop_timer(at(10, 0) + 20_000)
            .await
            .unwrap()
            .value
            .unwrap();

        assert_eq!(stopped.end, Some(at(10, 1)));
    }

    #[tokio::test]
    async fn undoing_a_stop_makes_the_timer_run_again() {
        let w = World::new().await;
        let started = w
            .service()
            .start_timer(timer(&w), at(10, 0))
            .await
            .unwrap()
            .value;
        w.service().stop_timer(at(10, 47)).await.unwrap();

        let undone = w.service().undo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Stop timer"));
        let rows = w.rows().await;
        assert_eq!(
            (rows[0].id.as_str(), rows[0].end),
            (started.id.as_str(), None)
        );
    }
}
