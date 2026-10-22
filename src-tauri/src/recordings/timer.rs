//! Timers: a recording with an open span, started and stopped now. Both are ordinary undoable changes.

use super::checks::{check_tag, checked_note, usable_project};
use super::ops::Op;
use super::{Change, MINUTE_MS, Recordings, snap};
use crate::db::new_id;
use crate::db::recordings::{self, Recording, RecordingSpan};
use crate::db::time::{Day, Ms};
use crate::error::AppResult;
use crate::timers::open_start;

#[derive(Debug, Clone, PartialEq)]
pub struct TimerInput {
    pub project_id: String,
    pub note: String,
    pub tag_id: Option<String>,
}

/// Where a timer started at `start` ends when stopped at `now`: on the snap grid, and at least a minute
/// long, so a stopped timer never leaves an empty span.
pub fn stop_time(start: Ms, now: Ms, snap_minutes: u32) -> Ms {
    snap(now, snap_minutes).max(start + MINUTE_MS)
}

fn same_timer(recording: &Recording, project_id: &str, note: &str, tag_id: Option<&str>) -> bool {
    recording.project_id == project_id
        && recording.note == note
        && recording.tag_id.as_deref() == tag_id
}

impl Recordings<'_> {
    /// Starts timing now; a running timer stops at the instant the new one starts. A finished recording of
    /// today with the same project, note and tag continues with a new open span instead of a new recording.
    /// When the running timer already is that one, nothing changes.
    pub async fn start_timer(&self, input: TimerInput, now: Ms) -> AppResult<Change<Recording>> {
        let mut undo = self.undo.lock().await;
        let note = checked_note(&input.note)?;
        let project = usable_project(self.db, &input.project_id, None).await?;
        check_tag(self.db, input.tag_id.as_deref()).await?;
        let running = recordings::running(self.db).await?;
        if let Some(r) = &running
            && same_timer(r, &project.id, &note, input.tag_id.as_deref())
        {
            return Ok(Change {
                value: r.clone(),
                days: Vec::new(),
            });
        }
        let start = match &running {
            Some(r) => stop_time(open_start(r), now, self.snap_minutes),
            None => self.free_start(now).await?,
        };
        let continued = self
            .continued(&project.id, &note, input.tag_id.as_deref(), start, now)
            .await?;
        let timer = TimerInput {
            project_id: project.id,
            note,
            tag_id: input.tag_id,
        };
        let (timed, timing) = continuation_or_new(continued, timer, start, now);
        let label = format!("Start timer for {}", project.name);
        let op = match running {
            Some(before) => Op::Batch {
                label: label.clone(),
                ops: vec![stopped(before, start, now), timing],
            },
            None => timing,
        };
        self.commit(&mut undo, label, op, &timed.id).await
    }

    /// The latest finished recording of today with this project, note and tag that ends by `start`.
    async fn continued(
        &self,
        project_id: &str,
        note: &str,
        tag_id: Option<&str>,
        start: Ms,
        now: Ms,
    ) -> AppResult<Option<Recording>> {
        let (from, to) = Day::of(now).bounds();
        Ok(recordings::list_for_range(self.db, from, to)
            .await?
            .into_iter()
            .filter(|r| same_timer(r, project_id, note, tag_id))
            .filter(|r| r.end().is_some_and(|end| end <= start))
            .max_by_key(Recording::end))
    }

    /// `now` on the snap grid; when that lies in the past, after any span that ended since, so a timer
    /// started right after another recording never overlaps it.
    async fn free_start(&self, now: Ms) -> AppResult<Ms> {
        let snapped = snap(now, self.snap_minutes);
        if snapped >= now {
            return Ok(snapped);
        }
        let ended = recordings::overlapping(self.db, snapped, now, None).await?;
        Ok(ended
            .iter()
            .flat_map(|r| &r.spans)
            .filter_map(|s| s.end.filter(|&end| end > snapped && end <= now))
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
        let end = stop_time(open_start(&running), now, self.snap_minutes);
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

/// The timer's recording with an open span from `start`, and the op that writes it: `continued` with the
/// span added, else a new recording of `timer`.
fn continuation_or_new(
    continued: Option<Recording>,
    timer: TimerInput,
    start: Ms,
    now: Ms,
) -> (Recording, Op) {
    let open = RecordingSpan::new(start, None);
    match continued {
        Some(before) => {
            let mut spans = before.spans.clone();
            spans.push(open);
            let after = Recording {
                spans,
                updated_at: now,
                ..before.clone()
            };
            (after.clone(), Op::Update { before, after })
        }
        None => {
            let recording = Recording {
                id: new_id(),
                project_id: timer.project_id,
                note: timer.note,
                tag_id: timer.tag_id,
                created_at: now,
                updated_at: now,
                spans: vec![open],
            };
            (recording.clone(), Op::Create { recording })
        }
    }
}

/// `before` with its open span ending at `end`.
fn stopped(before: Recording, end: Ms, now: Ms) -> Op {
    let spans = before
        .spans
        .iter()
        .map(|s| RecordingSpan {
            end: s.end.or(Some(end)),
            ..s.clone()
        })
        .collect();
    let after = Recording {
        spans,
        updated_at: now,
        ..before.clone()
    };
    Op::Update { before, after }
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::super::{Overlap, RecordingInput, Span};
    use super::*;
    use crate::db::projects;
    use crate::db::time::Day;
    use crate::error::ErrorKind;

    fn timer(w: &World) -> TimerInput {
        TimerInput {
            project_id: w.project.id.clone(),
            note: String::new(),
            tag_id: None,
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
            tag_id: Some(w.alpha.id.clone()),
            ..timer(&w)
        };

        let change = w.service().start_timer(input, at(10, 3)).await.unwrap();

        let started = change.value;
        assert_eq!(started.project_id, w.project.id);
        assert_eq!(
            started.spans,
            [RecordingSpan {
                id: started.spans[0].id.clone(),
                start: at(10, 5),
                end: None,
            }]
        );
        assert_eq!(started.note, "Review");
        assert_eq!(started.tag_id, Some(w.alpha.id.clone()));
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
            (rows[0].id.as_str(), rows[0].start(), rows[0].end()),
            (first.id.as_str(), at(10, 0), Some(at(10, 30)))
        );
        assert_eq!(
            (rows[1].id.as_str(), rows[1].start(), rows[1].end()),
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
        let other = TimerInput {
            project_id: w.other.id.clone(),
            ..timer(&w)
        };
        w.service().start_timer(other, at(10, 32)).await.unwrap();

        let undone = w.service().undo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Start timer for Beta"));
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
        crate::test_support::recording(&w.db, &w.other.id, at(10, 0), Some(at(10, 31))).await;

        let started = w
            .service()
            .start_timer(timer(&w), at(10, 32))
            .await
            .unwrap()
            .value;

        assert_eq!((started.start(), started.end()), (at(10, 31), None));
    }

    #[tokio::test]
    async fn a_timer_starts_after_the_latest_of_several_recordings_that_ended_since_the_snapped_time()
     {
        let w = World::new().await;
        let p = &w.other.id;
        crate::test_support::recording(&w.db, p, at(10, 0), Some(at(10, 6))).await;
        crate::test_support::recording(&w.db, p, at(10, 6), Some(at(10, 6) + 30_000)).await;

        let started = w
            .service()
            .start_timer(timer(&w), at(10, 7))
            .await
            .unwrap()
            .value;

        assert_eq!((started.start(), started.end()), (at(10, 6) + 30_000, None));
    }

    #[tokio::test]
    async fn a_timer_without_earlier_recordings_starts_at_the_snapped_now() {
        let w = World::new().await;

        for (now, expected) in [(at(10, 7), at(10, 5)), (at(10, 8), at(10, 10))] {
            let started = w.service().start_timer(timer(&w), now).await.unwrap().value;
            assert_eq!(started.start(), expected, "{now}");
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
        crate::test_support::recording(&w.db, &w.other.id, at(10, 0), Some(at(10, 30))).await;

        let started = w
            .service()
            .start_timer(timer(&w), at(10, 32))
            .await
            .unwrap()
            .value;

        assert_eq!(started.start(), at(10, 30));
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
                    tag_id: Some("nope".into()),
                    ..timer(&w)
                },
                ErrorKind::InvalidInput,
                Some("tagId".to_string()),
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
        assert_eq!(times(&stopped), [(at(10, 0), at(10, 45))]);
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

        assert_eq!(stopped.end(), Some(at(10, 1)));
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
            (rows[0].id.as_str(), rows[0].end()),
            (started.id.as_str(), None)
        );
    }

    fn review(w: &World) -> TimerInput {
        TimerInput {
            note: "Review".into(),
            tag_id: Some(w.alpha.id.clone()),
            ..timer(w)
        }
    }

    async fn finished_review(w: &World, start: Ms, end: Ms) -> Recording {
        w.service()
            .create(
                RecordingInput {
                    note: "Review".into(),
                    tag_id: Some(w.alpha.id.clone()),
                    ..w.input(start, end)
                },
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value
    }

    #[tokio::test]
    async fn starting_the_timer_of_a_finished_recording_of_today_continues_it_with_an_open_span() {
        let w = World::new().await;
        let finished = finished_review(&w, at(10, 0), at(11, 0)).await;

        let change = w
            .service()
            .start_timer(review(&w), at(14, 3))
            .await
            .unwrap();

        let started = change.value;
        assert_eq!(started.id, finished.id);
        assert_eq!(
            started.spans,
            [
                finished.spans[0].clone(),
                RecordingSpan {
                    id: started.spans[1].id.clone(),
                    start: at(14, 5),
                    end: None,
                }
            ]
        );
        assert_eq!(
            (started.created_at, started.updated_at),
            (finished.created_at, at(14, 3))
        );
        assert!(change.days.contains(&Day::of(at(14, 5))));
        assert_eq!(w.rows().await, [started]);
        assert_eq!(
            w.status().await.undo_label.as_deref(),
            Some("Start timer for Acme")
        );

        w.service().undo().await.unwrap();

        assert_eq!(w.rows().await, [finished]);
    }

    #[tokio::test]
    async fn a_timer_with_another_project_note_or_tag_starts_a_new_recording() {
        let w = World::new().await;
        let finished = finished_review(&w, at(10, 0), at(11, 0)).await;
        let cases = [
            (
                "project",
                TimerInput {
                    project_id: w.other.id.clone(),
                    ..review(&w)
                },
            ),
            (
                "note",
                TimerInput {
                    note: "Other".into(),
                    ..review(&w)
                },
            ),
            (
                "tag",
                TimerInput {
                    tag_id: Some(w.beta.id.clone()),
                    ..review(&w)
                },
            ),
            (
                "no tag",
                TimerInput {
                    tag_id: None,
                    ..review(&w)
                },
            ),
        ];

        for (name, input) in cases {
            let started = w
                .service()
                .start_timer(input, at(14, 0))
                .await
                .unwrap()
                .value;

            assert_ne!(started.id, finished.id, "{name}");
            assert_eq!(times(&finished), [(at(10, 0), at(11, 0))], "{name}");
            w.service().undo().await.unwrap();
        }
        assert_eq!(w.rows().await, [finished]);
    }

    #[tokio::test]
    async fn a_finished_recording_of_another_day_is_not_continued() {
        let w = World::new().await;
        let yesterday = finished_review(&w, at(10, 0) - 48 * HOUR, at(11, 0) - 48 * HOUR).await;

        let started = w
            .service()
            .start_timer(review(&w), at(14, 0))
            .await
            .unwrap()
            .value;

        assert_ne!(started.id, yesterday.id);
        assert_eq!(w.rows().await, [yesterday, started]);
    }

    #[tokio::test]
    async fn a_matching_recording_with_a_later_span_is_not_continued_and_the_timer_is_refused() {
        let w = World::new().await;
        let finished = w
            .service()
            .create(
                RecordingInput {
                    note: "Review".into(),
                    tag_id: Some(w.alpha.id.clone()),
                    ..input_for(&w, &[(at(10, 0), at(11, 0)), (at(15, 0), at(16, 0))])
                },
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;

        let err = w
            .service()
            .start_timer(review(&w), at(14, 0))
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::Overlap);
        assert_eq!(w.rows().await, [finished]);
    }

    fn input_for(w: &World, spans: &[(Ms, Ms)]) -> RecordingInput {
        RecordingInput {
            spans: spans
                .iter()
                .map(|&(start, end)| Span { start, end })
                .collect(),
            ..w.input(0, 0)
        }
    }

    #[tokio::test]
    async fn starting_the_timer_that_already_runs_changes_nothing_and_adds_no_undo_entry() {
        let w = World::new().await;
        let running = w
            .service()
            .start_timer(review(&w), at(10, 0))
            .await
            .unwrap()
            .value;
        let status = w.status().await;

        let again = w
            .service()
            .start_timer(review(&w), at(10, 20))
            .await
            .unwrap();

        assert_eq!(again.value, running);
        assert!(again.days.is_empty());
        assert_eq!(w.rows().await, [running]);
        assert_eq!(w.status().await, status);
    }

    #[tokio::test]
    async fn a_different_running_timer_is_stopped_and_the_match_continued_in_one_undo_step() {
        let w = World::new().await;
        let finished = finished_review(&w, at(8, 0), at(9, 0)).await;
        let other = TimerInput {
            project_id: w.other.id.clone(),
            ..timer(&w)
        };
        let running = w
            .service()
            .start_timer(other, at(10, 0))
            .await
            .unwrap()
            .value;
        let before = w.rows().await;

        let continued = w
            .service()
            .start_timer(review(&w), at(10, 32))
            .await
            .unwrap()
            .value;

        assert_eq!(continued.id, finished.id);
        let rows = w.rows().await;
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (rows[0].id.as_str(), rows[0].spans.len(), rows[0].end()),
            (finished.id.as_str(), 2, None)
        );
        assert_eq!(rows[0].spans[1].start, at(10, 30));
        assert_eq!(
            (rows[1].id.as_str(), times(&rows[1])),
            (running.id.as_str(), vec![(at(10, 0), at(10, 30))])
        );
        assert_eq!(
            w.status().await.undo_label.as_deref(),
            Some("Start timer for Acme")
        );

        w.service().undo().await.unwrap();

        assert_eq!(w.rows().await, before);
        assert_eq!(
            w.status().await.undo_label.as_deref(),
            Some("Start timer for Beta")
        );
    }

    #[tokio::test]
    async fn stopping_a_continued_recording_closes_only_the_open_span() {
        let w = World::new().await;
        let finished = finished_review(&w, at(10, 0), at(11, 0)).await;
        w.service()
            .start_timer(review(&w), at(14, 3))
            .await
            .unwrap();

        let stopped = w
            .service()
            .stop_timer(at(14, 47))
            .await
            .unwrap()
            .value
            .unwrap();

        assert_eq!(stopped.spans[0], finished.spans[0]);
        assert_eq!(
            times(&stopped),
            [(at(10, 0), at(11, 0)), (at(14, 5), at(14, 45))]
        );
        assert_eq!(w.rows().await, [stopped]);
        assert_eq!(w.status().await.undo_label.as_deref(), Some("Stop timer"));
    }
}
