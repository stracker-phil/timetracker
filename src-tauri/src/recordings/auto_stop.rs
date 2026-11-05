//! A timer never passes a calendar day (ADR 0042): a forgotten one stops by the midnight after its open
//! start, and earlier where the Mac fell asleep for more than an hour.

use chrono::{Local, TimeZone};

use super::timer::{stop_time, stopping};
use super::{Change, MINUTE_MS, Recordings};
use crate::db::away;
use crate::db::time::{Day, Ms};
use crate::error::AppResult;
use crate::timers::{self, Timer};

/// A sleep longer than this stops the timer where it began.
pub const SLEEP_STOP_MS: Ms = 60 * MINUTE_MS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutoStopReason {
    Midnight,
    Sleep,
}

impl AutoStopReason {
    fn label(self) -> &'static str {
        match self {
            AutoStopReason::Midnight => "Stop timer at midnight",
            AutoStopReason::Sleep => "Stop timer (the Mac slept)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AutoStop {
    end: Ms,
    reason: AutoStopReason,
}

impl AutoStop {
    fn slept_at(start: Ms) -> Self {
        AutoStop {
            end: start,
            reason: AutoStopReason::Sleep,
        }
    }
}

/// The midnight that ends the local day of `at`.
fn day_end_in<Tz: TimeZone>(at: Ms, tz: &Tz) -> Ms {
    Day::of_in(at, tz).bounds_in(tz).1
}

/// `stop_time`, never past the midnight of the open span's day.
pub(super) fn capped_stop_time(open_start: Ms, now: Ms, snap_minutes: u32) -> Ms {
    stop_time(open_start, now, snap_minutes).min(day_end_in(open_start, &Local))
}

/// Where a timer whose open span starts at `open_start` ends by `now`, the earliest of: the start of the
/// first sleep from `open_start` on that lasts more than `SLEEP_STOP_MS` (an open sleep lasts until `now`),
/// and, once `now` has passed the midnight after `open_start`, that midnight or the start of a sleep across
/// it. `None` while the timer may run on. `sleeps` are `(start, end)`, `None` while asleep.
fn auto_stop_in<Tz: TimeZone>(
    open_start: Ms,
    now: Ms,
    sleeps: &[(Ms, Option<Ms>)],
    tz: &Tz,
) -> Option<AutoStop> {
    let since_start = sleeps
        .iter()
        .map(|&(start, end)| (start, end.unwrap_or(now)))
        .filter(|&(start, _)| start >= open_start);
    let long_sleep = since_start
        .clone()
        .filter(|&(start, end)| end - start > SLEEP_STOP_MS)
        .map(|(start, _)| start)
        .min()
        .map(AutoStop::slept_at);
    let midnight = day_end_in(open_start, tz);
    let at_midnight = (now >= midnight).then(|| {
        since_start
            .filter(|&(start, end)| start < midnight && end >= midnight)
            .map(|(start, _)| start)
            .min()
            .map_or(
                AutoStop {
                    end: midnight,
                    reason: AutoStopReason::Midnight,
                },
                AutoStop::slept_at,
            )
    });
    [long_sleep, at_midnight]
        .into_iter()
        .flatten()
        .min_by_key(|stop| stop.end)
}

impl Recordings<'_> {
    /// Stops the running timer where `auto_stop_in` ends it, as one undoable change; `None` while it may run
    /// on or none runs. The sleeps are those the tracker recorded since the open start, plus `unseen`, a
    /// stretch the app's minute ticks missed (a sleep the paused tracker did not record), unless a recorded
    /// sleep starts in it.
    pub async fn auto_stop_timer(
        &self,
        now: Ms,
        unseen: Option<(Ms, Ms)>,
    ) -> AppResult<Change<Option<Timer>>> {
        let mut undo = self.undo.lock().await;
        let Some(running) = timers::current(self.db).await? else {
            return Ok(Change::unchanged(None));
        };
        let open_start = running.open_start();
        let mut sleeps = away::sleeps_since(self.db, open_start).await?;
        if let Some((from, to)) = unseen
            && !sleeps
                .iter()
                .any(|&(start, _)| (from..=to).contains(&start))
        {
            sleeps.push((from, Some(to)));
        }
        let Some(stop) = auto_stop_in(open_start, now, &sleeps, &Local) else {
            return Ok(Change::unchanged(None));
        };
        let end = capped_stop_time(open_start, stop.end, self.snap_minutes);
        let timer = running.clone();
        let days = self
            .apply_and_push(
                &mut undo,
                stop.reason.label().into(),
                stopping(running, end, now),
            )
            .await?;
        Ok(Change {
            value: Some(self.reread(&timer).await?),
            days,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono_tz::{Europe::Vienna, UTC};

    use super::super::testing::*;
    use super::super::timer::TimerInput;
    use super::*;
    use crate::db::away::{AwayReason, NewAway};
    use crate::db::recordings::Span;
    use crate::db::stored_suggestions::{self, StoredSuggestion};
    use crate::db::{Db, new_id};

    const MIDNIGHT: AutoStopReason = AutoStopReason::Midnight;
    const SLEEP: AutoStopReason = AutoStopReason::Sleep;

    fn stop(end: Ms, reason: AutoStopReason) -> Option<AutoStop> {
        Some(AutoStop { end, reason })
    }

    fn vienna(month: u32, day: u32, hour: u32) -> Ms {
        Vienna
            .with_ymd_and_hms(2026, month, day, hour, 0, 0)
            .single()
            .expect("a valid local time")
            .timestamp_millis()
    }

    #[test]
    fn a_timer_stops_where_a_long_sleep_began_or_at_midnight() {
        let midnight = at(24, 0);
        let cases = [
            (
                "a sleep over an hour",
                at(14, 0),
                at(18, 5),
                vec![(at(17, 0), Some(at(18, 5)))],
                stop(at(17, 0), SLEEP),
            ),
            (
                "a sleep under an hour",
                at(14, 0),
                at(18, 0),
                vec![(at(17, 0), Some(at(17, 50)))],
                None,
            ),
            (
                "a sleep of exactly an hour",
                at(14, 0),
                at(18, 0),
                vec![(at(17, 0), Some(at(18, 0)))],
                None,
            ),
            (
                "a sleep over midnight",
                at(14, 0),
                at(31, 0),
                vec![(at(23, 15), Some(at(31, 0)))],
                stop(at(23, 15), SLEEP),
            ),
            (
                "a short sleep across midnight",
                at(14, 0),
                at(24, 10),
                vec![(at(23, 50), Some(at(24, 10)))],
                stop(at(23, 50), SLEEP),
            ),
            (
                "a long sleep before the start",
                at(14, 0),
                at(15, 0),
                vec![(at(12, 0), Some(at(15, 0)))],
                None,
            ),
            (
                "an open sleep over an hour",
                at(14, 0),
                at(18, 30),
                vec![(at(17, 0), None)],
                stop(at(17, 0), SLEEP),
            ),
            (
                "an open sleep under an hour",
                at(14, 0),
                at(17, 30),
                vec![(at(17, 0), None)],
                None,
            ),
            (
                "no sleep before midnight",
                at(14, 0),
                at(23, 59),
                vec![],
                None,
            ),
            (
                "no sleep, two days after the start",
                at(14, 0) - 48 * HOUR,
                at(14, 0),
                vec![],
                stop(midnight - 48 * HOUR, MIDNIGHT),
            ),
            (
                "no sleep, exactly at midnight",
                at(14, 0),
                midnight,
                vec![],
                stop(midnight, MIDNIGHT),
            ),
            (
                "a sleep after midnight",
                at(14, 0),
                at(27, 0),
                vec![(at(25, 0), Some(at(27, 0)))],
                stop(midnight, MIDNIGHT),
            ),
            (
                "the earliest of two long sleeps",
                at(8, 0),
                at(20, 0),
                vec![(at(15, 0), Some(at(17, 0))), (at(10, 0), Some(at(12, 0)))],
                stop(at(10, 0), SLEEP),
            ),
        ];
        for (name, open_start, now, sleeps, expected) in cases {
            assert_eq!(
                auto_stop_in(open_start, now, &sleeps, &UTC),
                expected,
                "{name}"
            );
        }
    }

    #[test]
    fn the_midnight_of_a_daylight_saving_day_ends_a_23_or_25_hour_day() {
        for (month, day, hours) in [(3, 29, 23), (10, 25, 25)] {
            let start = vienna(month, day, 10);
            let midnight = vienna(month, day + 1, 0);

            assert_eq!(
                midnight - vienna(month, day, 0),
                hours * HOUR,
                "{month}-{day}"
            );
            assert_eq!(
                auto_stop_in(start, midnight - MIN, &[], &Vienna),
                None,
                "{month}-{day} a minute before"
            );
            assert_eq!(
                auto_stop_in(start, midnight, &[], &Vienna),
                stop(midnight, MIDNIGHT),
                "{month}-{day}"
            );
        }
    }

    #[test]
    fn reasons_are_labeled_for_the_undo_entry() {
        assert_eq!(MIDNIGHT.label(), "Stop timer at midnight");
        assert_eq!(SLEEP.label(), "Stop timer (the Mac slept)");
    }

    #[test]
    fn a_stop_never_passes_the_midnight_of_the_open_start() {
        let (_, midnight, _) = midnight();
        let start = midnight - 2 * HOUR;
        for (now, expected) in [
            (start + 30 * MIN, start + 30 * MIN),
            (midnight - 7 * MIN, midnight - 5 * MIN),
            (midnight + HOUR, midnight),
            (midnight + 30 * HOUR, midnight),
        ] {
            assert_eq!(capped_stop_time(start, now, 5), expected, "{now}");
        }
    }

    fn timer(w: &World) -> TimerInput {
        TimerInput {
            project_id: w.project.id.clone(),
            note: String::new(),
            tag_id: None,
        }
    }

    fn other(w: &World) -> TimerInput {
        TimerInput {
            project_id: w.other.id.clone(),
            ..timer(w)
        }
    }

    async fn sleep(db: &Db, start: Ms, end: Option<Ms>) {
        away::insert(
            db,
            NewAway {
                start,
                reason: AwayReason::Sleep,
            },
        )
        .await
        .unwrap();
        if let Some(end) = end {
            away::close_open(db, end).await.unwrap();
        }
    }

    async fn running_suggestion(w: &World, start: Ms) -> StoredSuggestion {
        match w
            .service()
            .start_suggested_timer(timer(w), "cli", start)
            .await
            .unwrap()
            .value
        {
            Timer::Suggestion(s) => s,
            Timer::Recording(r) => panic!("expected a suggestion, got recording {}", r.id),
        }
    }

    #[tokio::test]
    async fn without_a_timer_nothing_stops() {
        let w = World::new().await;
        let (_, midnight, _) = midnight();

        let change = w
            .service()
            .auto_stop_timer(midnight + HOUR, None)
            .await
            .unwrap();

        assert_eq!(change.value, None);
        assert!(change.days.is_empty());
        assert_eq!(w.status().await, no_status());
    }

    #[tokio::test]
    async fn a_timer_that_is_not_due_keeps_running_and_nothing_is_written() {
        let w = World::new().await;
        let (_, midnight, _) = midnight();
        let start = midnight - 6 * HOUR;
        let running = running_with(&w, &[], start).await;
        sleep(&w.db, start + HOUR, Some(start + 90 * MIN)).await;

        let change = w
            .service()
            .auto_stop_timer(midnight - 3 * HOUR, None)
            .await
            .unwrap();

        assert_eq!(change.value, None);
        assert!(change.days.is_empty());
        assert_eq!(w.rows().await, [running]);
        assert_eq!(w.status().await, no_status());
    }

    #[tokio::test]
    async fn a_recording_timer_left_over_midnight_stops_at_midnight_and_undo_runs_it_again() {
        let w = World::new().await;
        let (_, midnight, _) = midnight();
        let start = midnight - 2 * HOUR;
        let running = running_with(&w, &[(start - HOUR, start - 30 * MIN)], start).await;

        let change = w
            .service()
            .auto_stop_timer(midnight + HOUR, None)
            .await
            .unwrap();

        let stopped = change
            .value
            .and_then(recording_of)
            .expect("a stopped timer");
        assert_eq!(stopped.id, running.id);
        assert_eq!(
            times(&stopped),
            [(start - HOUR, start - 30 * MIN), (start, midnight)]
        );
        assert!(change.days.contains(&Day::of(start)));
        assert_eq!(w.rows().await, [stopped]);
        assert_eq!(w.status().await, undo_label("Stop timer at midnight"));

        let undone = w.service().undo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Stop timer at midnight"));
        assert_eq!(w.rows().await, [running]);
    }

    #[tokio::test]
    async fn a_recording_timer_stops_where_a_long_sleep_began_snapped_and_at_least_a_minute_long() {
        let (_, midnight, _) = midnight();
        let start = midnight - 6 * HOUR;
        for (began, expected) in [
            (start + 20_000, start + MIN),
            (start + 3 * MIN, start + 5 * MIN),
            (start + 30 * MIN, start + 30 * MIN),
        ] {
            let w = World::new().await;
            running_with(&w, &[], start).await;
            sleep(&w.db, began, Some(midnight - 3 * HOUR)).await;

            let change = w
                .service()
                .auto_stop_timer(midnight - 3 * HOUR, None)
                .await
                .unwrap();

            let stopped = change
                .value
                .and_then(recording_of)
                .expect("a stopped timer");
            assert_eq!(times(&stopped), [(start, expected)], "{began}");
            assert_eq!(
                w.status().await,
                undo_label("Stop timer (the Mac slept)"),
                "{began}"
            );
        }
    }

    #[tokio::test]
    async fn an_open_sleep_across_midnight_stops_the_timer_at_its_start() {
        let w = World::new().await;
        let (_, midnight, _) = midnight();
        let start = midnight - 6 * HOUR;
        running_with(&w, &[], start).await;
        sleep(&w.db, midnight - 10 * MIN, None).await;

        let change = w
            .service()
            .auto_stop_timer(midnight + 10 * MIN, None)
            .await
            .unwrap();

        let stopped = change
            .value
            .and_then(recording_of)
            .expect("a stopped timer");
        assert_eq!(times(&stopped), [(start, midnight - 10 * MIN)]);
        assert_eq!(w.status().await, undo_label("Stop timer (the Mac slept)"));
    }

    #[tokio::test]
    async fn a_suggested_timer_left_over_midnight_stops_at_midnight_and_undo_runs_it_again() {
        let w = World::new().await;
        let (_, midnight, _) = midnight();
        let start = midnight - 3 * HOUR;
        let running = running_suggestion(&w, start).await;

        let change = w
            .service()
            .auto_stop_timer(midnight + HOUR, None)
            .await
            .unwrap();

        let Some(Timer::Suggestion(stopped)) = change.value else {
            panic!("expected a stopped suggestion");
        };
        assert_eq!(
            stopped,
            StoredSuggestion {
                spans: vec![Span {
                    start,
                    end: midnight
                }],
                open_start: None,
                updated_at: midnight + HOUR,
                ..running.clone()
            }
        );
        assert_eq!(
            stored_suggestions::get(&w.db, &running.id).await.unwrap(),
            stopped
        );
        assert_eq!(timers::current(&w.db).await.unwrap(), None);
        assert_eq!(
            w.status().await.undo_label.as_deref(),
            Some("Stop timer at midnight")
        );

        w.service().undo().await.unwrap();

        assert_eq!(
            timers::current(&w.db).await.unwrap(),
            Some(Timer::Suggestion(running))
        );
    }

    #[tokio::test]
    async fn a_suggested_timer_stops_at_a_long_sleep_with_the_sleep_label() {
        let w = World::new().await;
        let (_, midnight, _) = midnight();
        let start = midnight - 6 * HOUR;
        let running = running_suggestion(&w, start).await;
        sleep(&w.db, midnight - 4 * HOUR, Some(midnight - 2 * HOUR)).await;

        let change = w
            .service()
            .auto_stop_timer(midnight - 2 * HOUR, None)
            .await
            .unwrap();

        let Some(Timer::Suggestion(stopped)) = change.value else {
            panic!("expected a stopped suggestion");
        };
        assert_eq!(stopped.id, running.id);
        assert_eq!(
            stopped.spans,
            [Span {
                start,
                end: midnight - 4 * HOUR
            }]
        );
        assert_eq!(stopped.open_start, None);
        assert_eq!(w.status().await, undo_label("Stop timer (the Mac slept)"));
    }

    #[tokio::test]
    async fn an_unseen_gap_over_an_hour_stops_the_timer_where_it_began_on_the_grid() {
        let w = World::new().await;
        let (_, midnight, _) = midnight();
        let start = midnight - 10 * HOUR;
        let from = midnight - 7 * HOUR - MIN;
        let to = from + 66 * MIN;
        running_with(&w, &[], start).await;

        let change = w
            .service()
            .auto_stop_timer(to, Some((from, to)))
            .await
            .unwrap();

        let stopped = change
            .value
            .and_then(recording_of)
            .expect("a stopped timer");
        assert_eq!(times(&stopped), [(start, midnight - 7 * HOUR)]);
        assert_eq!(w.status().await, undo_label("Stop timer (the Mac slept)"));
    }

    #[tokio::test]
    async fn an_unseen_gap_of_an_hour_or_less_leaves_the_timer_running() {
        let w = World::new().await;
        let (_, midnight, _) = midnight();
        let from = midnight - 7 * HOUR;
        let running = running_with(&w, &[], midnight - 10 * HOUR).await;

        let change = w
            .service()
            .auto_stop_timer(from + HOUR, Some((from, from + HOUR)))
            .await
            .unwrap();

        assert_eq!(change.value, None);
        assert_eq!(w.rows().await, [running]);
    }

    #[tokio::test]
    async fn an_unseen_gap_is_ignored_when_a_recorded_sleep_starts_inside_it() {
        let w = World::new().await;
        let (_, midnight, _) = midnight();
        let start = midnight - 10 * HOUR;
        let from = midnight - 7 * HOUR - MIN;
        let to = midnight - 5 * HOUR;
        running_with(&w, &[], start).await;
        sleep(&w.db, midnight - 7 * HOUR + 10 * MIN, None).await;

        let change = w
            .service()
            .auto_stop_timer(to, Some((from, to)))
            .await
            .unwrap();

        let stopped = change
            .value
            .and_then(recording_of)
            .expect("a stopped timer");
        assert_eq!(times(&stopped), [(start, midnight - 7 * HOUR + 10 * MIN)]);
    }

    #[tokio::test]
    async fn stopping_a_timer_from_yesterday_ends_it_at_yesterdays_midnight() {
        let w = World::new().await;
        let (_, midnight, _) = midnight();
        let start = midnight - 2 * HOUR;
        running_with(&w, &[], start).await;

        let stopped = w
            .service()
            .stop_timer(midnight + 9 * HOUR)
            .await
            .unwrap()
            .value
            .and_then(recording_of)
            .expect("a stopped timer");

        assert_eq!(times(&stopped), [(start, midnight)]);
        assert_eq!(w.status().await, undo_label("Stop timer"));
    }

    #[tokio::test]
    async fn stopping_a_suggested_timer_from_yesterday_ends_it_at_yesterdays_midnight() {
        let w = World::new().await;
        let (_, midnight, _) = midnight();
        let start = midnight - 2 * HOUR;
        let running = running_suggestion(&w, start).await;

        w.service().stop_timer(midnight + 9 * HOUR).await.unwrap();

        assert_eq!(
            stored_suggestions::get(&w.db, &running.id)
                .await
                .unwrap()
                .spans,
            [Span {
                start,
                end: midnight
            }]
        );
    }

    #[tokio::test]
    async fn starting_a_timer_while_yesterdays_runs_ends_the_old_one_at_midnight_and_starts_now() {
        let w = World::new().await;
        let (_, midnight, _) = midnight();
        let start = midnight - 2 * HOUR;
        running_with(&w, &[], start).await;

        let started = w
            .service()
            .start_timer(other(&w), midnight + 9 * HOUR + 2 * MIN)
            .await
            .unwrap()
            .value;

        assert_eq!(
            (started.start(), started.end()),
            (midnight + 9 * HOUR, None)
        );
        let rows = w.rows().await;
        assert_eq!(rows.len(), 2);
        assert_eq!(times(&rows[0]), [(start, midnight)]);
        assert_eq!(rows[1], started);
    }

    #[tokio::test]
    async fn starting_a_suggested_timer_while_yesterdays_runs_ends_the_old_one_at_midnight() {
        let w = World::new().await;
        let (_, midnight, _) = midnight();
        let start = midnight - 2 * HOUR;
        running_with(&w, &[], start).await;

        let started = w
            .service()
            .start_suggested_timer(other(&w), "cli", midnight + 9 * HOUR + 2 * MIN)
            .await
            .unwrap()
            .value;

        let Timer::Suggestion(started) = started else {
            panic!("expected a suggestion");
        };
        assert_eq!(started.open_start, Some(midnight + 9 * HOUR));
        assert_eq!(times(&w.rows().await[0]), [(start, midnight)]);
    }

    #[tokio::test]
    async fn a_timer_does_not_continue_a_recording_that_started_yesterday() {
        let w = World::new().await;
        let (_, midnight, _) = midnight();
        let overnight = crate::test_support::recording(
            &w.db,
            &w.project.id,
            midnight - 3 * HOUR,
            Some(midnight + HOUR),
        )
        .await;

        let started = w
            .service()
            .start_timer(timer(&w), midnight + 3 * HOUR)
            .await
            .unwrap()
            .value;

        assert_ne!(started.id, overnight.id);
        assert_eq!(w.rows().await, [overnight, started]);
    }

    #[tokio::test]
    async fn a_suggested_timer_does_not_continue_a_suggestion_with_a_span_before_midnight() {
        let w = World::new().await;
        let (_, midnight, today) = midnight();
        let overnight = StoredSuggestion {
            id: new_id(),
            project_id: Some(w.project.id.clone()),
            note: String::new(),
            tag_id: None,
            source: "cli".into(),
            day: today,
            spans: vec![Span {
                start: midnight - HOUR,
                end: midnight + HOUR,
            }],
            created_at: 1_000,
            updated_at: 2_000,
            open_start: None,
        };
        let mut conn = w.db.pool().acquire().await.unwrap();
        stored_suggestions::put_on(&mut conn, &overnight)
            .await
            .unwrap();
        drop(conn);

        let started = running_suggestion(&w, midnight + 3 * HOUR).await;

        assert_ne!(started.id, overnight.id);
        assert!(started.spans.is_empty());
        assert_eq!(
            stored_suggestions::get(&w.db, &overnight.id).await.unwrap(),
            overnight
        );
    }
}
