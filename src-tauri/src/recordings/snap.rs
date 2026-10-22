//! Snapping times to the `timeline.snapMinutes` grid.

use chrono::{Duration, Local, LocalResult, NaiveTime, TimeZone, Timelike};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::to_day::span_on_day;
use super::{MINUTE_MS, Span};
use crate::db::time::{Day, Ms};

/// The end of a span a drag moves; the other end keeps its place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum SnapEdge {
    Start,
    End,
}

/// `ms` moved to the nearest multiple of `minutes` counted from its local midnight, so DST days still land
/// on round wall-clock minutes. A tie rounds up.
pub fn snap(ms: Ms, minutes: u32) -> Ms {
    snap_in(ms, minutes, &Local)
}

pub fn snap_in<Tz: TimeZone>(ms: Ms, minutes: u32, tz: &Tz) -> Ms {
    let step = grid_step(minutes);
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

/// A drag's span on the grid. With a `moving` end (an edge drag) only that end snaps, to the nearest mark past
/// the other end, which keeps its time even when it is off the grid; without one (the span moved whole) both
/// ends snap (`snap_span`). The timeline sends marks from `ticks`, which this leaves unchanged.
pub fn snap_moving(start: Ms, end: Ms, minutes: u32, moving: Option<SnapEdge>) -> Span {
    match moving {
        None => snap_span(start, end, minutes),
        Some(SnapEdge::Start) => {
            let snapped = snap(start, minutes);
            Span {
                start: if snapped < end {
                    snapped
                } else {
                    mark_before(end, minutes)
                },
                end,
            }
        }
        Some(SnapEdge::End) => {
            let snapped = snap(end, minutes);
            Span {
                start,
                end: if snapped > start {
                    snapped
                } else {
                    mark_after(start, minutes)
                },
            }
        }
    }
}

/// The last mark before `ms`.
fn mark_before(ms: Ms, minutes: u32) -> Ms {
    snap(ms - 1 - grid_step(minutes) / 2, minutes)
}

/// The first mark after `ms`.
fn mark_after(ms: Ms, minutes: u32) -> Ms {
    snap(ms + grid_step(minutes) / 2, minutes)
}

/// Every mark of the `minutes` grid from `first` to `last`, both included, ascending: what the Day view's
/// pointer input picks from. Counted from each local midnight like `snap`, so a DST day has the marks its
/// wall clock shows (none in the skipped hour, the repeated hour's twice).
pub fn ticks(first: Ms, last: Ms, minutes: u32) -> Vec<Ms> {
    ticks_in(first, last, minutes, &Local)
}

pub(crate) fn ticks_in<Tz: TimeZone>(first: Ms, last: Ms, minutes: u32, tz: &Tz) -> Vec<Ms> {
    let (Some(from), Some(to)) = (
        chrono::DateTime::from_timestamp_millis(first),
        chrono::DateTime::from_timestamp_millis(last),
    ) else {
        return Vec::new();
    };
    let step = Duration::milliseconds(grid_step(minutes));
    let mut ticks = Vec::new();
    let mut date = from.with_timezone(tz).date_naive();
    while date <= to.with_timezone(tz).date_naive() {
        let mut wall = date.and_time(NaiveTime::MIN);
        while wall.date() == date {
            match tz.from_local_datetime(&wall) {
                LocalResult::Single(t) => ticks.push(t.timestamp_millis()),
                LocalResult::Ambiguous(a, b) => {
                    ticks.extend([a.timestamp_millis(), b.timestamp_millis()]);
                }
                LocalResult::None => {}
            }
            wall += step;
        }
        let Some(next) = date.succ_opt() else { break };
        date = next;
    }
    ticks.retain(|&t| (first..=last).contains(&t));
    ticks.sort_unstable();
    ticks.dedup();
    ticks
}

/// How long a recording made with New Recording lasts by default.
pub const NEW_RECORDING_MINUTES: i64 = 30;

/// The span a new recording without a selection proposes: the half hour up to `now`, snapped.
pub fn default_span(now: Ms, minutes: u32) -> Span {
    snap_span(now - NEW_RECORDING_MINUTES * MINUTE_MS, now, minutes)
}

/// `default_span` on `day` (the Week's and Month's +): on another day than `now`'s at the same clock times.
pub fn default_span_on(day: Option<Day>, now: Ms, minutes: u32) -> Span {
    let span = default_span(now, minutes);
    match day {
        Some(day) if day != Day::of(now) => span_on_day(span, day),
        _ => span,
    }
}

/// An end that snapped onto or before the start lands one step after it.
fn past_start(start: Ms, end: Ms, minutes: u32) -> Ms {
    if end <= start {
        start + grid_step(minutes)
    } else {
        end
    }
}

/// One step of the grid in milliseconds.
pub fn grid_step(minutes: u32) -> Ms {
    i64::from(minutes.max(1)) * MINUTE_MS
}

#[cfg(test)]
mod tests {
    use chrono_tz::Europe::Vienna;

    use super::*;
    use crate::db::time::same_time_on;
    use crate::recordings::testing::*;

    fn vienna(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> Ms {
        Vienna
            .with_ymd_and_hms(year, month, day, hour, minute, second)
            .earliest()
            .unwrap()
            .timestamp_millis()
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
    fn snap_moving_snaps_only_the_dragged_end_and_keeps_it_past_the_other() {
        use SnapEdge::{End, Start};
        let t = |minute| at(13, minute);
        let cases = [
            ("start inside", 1, (27, 50), Start, (27, 50)),
            ("start inside", 10, (27, 50), Start, (30, 50)),
            ("start inside", 15, (27, 50), Start, (30, 50)),
            ("start past the end", 1, (43, 38), Start, (37, 38)),
            ("start past the end", 10, (43, 38), Start, (30, 38)),
            ("start past the end", 15, (43, 38), Start, (30, 38)),
            ("start onto the end", 1, (38, 38), Start, (37, 38)),
            ("start onto the end", 10, (38, 38), Start, (30, 38)),
            ("start onto the end", 15, (38, 38), Start, (30, 38)),
            ("end inside", 1, (12, 38), End, (12, 38)),
            ("end inside", 10, (12, 38), End, (12, 40)),
            ("end inside", 15, (12, 38), End, (12, 45)),
            ("end before the start", 1, (20, 10), End, (20, 21)),
            ("end before the start", 10, (20, 10), End, (20, 30)),
            ("end before the start", 15, (20, 10), End, (20, 30)),
            ("end onto the start", 1, (20, 20), End, (20, 21)),
            ("end onto the start", 10, (20, 20), End, (20, 30)),
            ("end onto the start", 15, (20, 20), End, (20, 30)),
        ];
        for (name, minutes, (start, end), moving, (expected_start, expected_end)) in cases {
            assert_eq!(
                snap_moving(t(start), t(end), minutes, Some(moving)),
                Span {
                    start: t(expected_start),
                    end: t(expected_end)
                },
                "{name} at {minutes} min"
            );
        }
    }

    #[test]
    fn snap_moving_returns_a_span_on_the_grid_unchanged() {
        for minutes in [1, 5, 10, 15] {
            for moving in [None, Some(SnapEdge::Start), Some(SnapEdge::End)] {
                assert_eq!(
                    snap_moving(at(10, 0), at(11, 30), minutes, moving),
                    Span {
                        start: at(10, 0),
                        end: at(11, 30)
                    },
                    "{minutes} min, {moving:?}"
                );
            }
        }
    }

    #[test]
    fn snap_moving_keeps_an_off_grid_fixed_edge() {
        let kept_end = snap_moving(at(13, 3), at(13, 28), 5, Some(SnapEdge::Start));
        let kept_start = snap_moving(at(13, 28), at(14, 2), 5, Some(SnapEdge::End));

        assert_eq!(
            kept_end,
            Span {
                start: at(13, 5),
                end: at(13, 28)
            }
        );
        assert_eq!(
            kept_start,
            Span {
                start: at(13, 28),
                end: at(14, 0)
            }
        );
    }

    #[test]
    fn snap_moving_without_a_moving_end_is_snap_span_of_the_raw_ends() {
        let cases = [
            (at(10, 3), at(10, 58)),
            (at(10, 0), at(10, 1)),
            (at(10, 0), at(10, 0)),
            (at(10, 0), at(9, 0)),
            (at(13, 12), at(13, 38)),
        ];
        for minutes in [1, 5, 10, 15] {
            for (start, end) in cases {
                assert_eq!(
                    snap_moving(start, end, minutes, None),
                    snap_span(start, end, minutes),
                    "{minutes} min: {start}..{end}"
                );
            }
        }
    }

    fn day_ticks(month: u32, day: u32, minutes: u32) -> Vec<Ms> {
        ticks_in(
            vienna(2026, month, day, 0, 0, 0),
            vienna(2026, month, day + 1, 0, 0, 0),
            minutes,
            &Vienna,
        )
    }

    #[test]
    fn ticks_of_a_day_run_from_one_midnight_to_the_next_on_every_step() {
        let cases = [
            ("normal day", 6, 15, 5, 289),
            ("spring day", 3, 29, 5, 277),
            ("autumn day", 10, 25, 5, 301),
            ("1 min", 6, 15, 1, 1441),
            ("10 min", 6, 15, 10, 145),
            ("15 min", 6, 15, 15, 97),
            ("15 min spring", 3, 29, 15, 93),
            ("15 min autumn", 10, 25, 15, 101),
        ];
        for (name, month, day, minutes, count) in cases {
            let ticks = day_ticks(month, day, minutes);

            assert_eq!(ticks.len(), count, "{name}");
            assert_eq!(ticks[0], vienna(2026, month, day, 0, 0, 0), "{name}");
            assert_eq!(
                ticks[count - 1],
                vienna(2026, month, day + 1, 0, 0, 0),
                "{name}"
            );
            assert!(
                ticks
                    .windows(2)
                    .all(|w| w[1] - w[0] == i64::from(minutes) * MIN),
                "{name}"
            );
        }
    }

    #[test]
    fn ticks_of_the_autumn_day_hold_the_repeated_hour_twice() {
        let ticks = day_ticks(10, 25, 15);
        let first_pass = Vienna
            .with_ymd_and_hms(2026, 10, 25, 2, 30, 0)
            .earliest()
            .unwrap()
            .timestamp_millis();
        let second_pass = Vienna
            .with_ymd_and_hms(2026, 10, 25, 2, 30, 0)
            .latest()
            .unwrap()
            .timestamp_millis();

        assert!(ticks.contains(&first_pass));
        assert!(ticks.contains(&second_pass));
        assert_eq!(second_pass - first_pass, HOUR);
    }

    #[test]
    fn ticks_of_the_spring_day_skip_the_missing_hour() {
        let ticks = day_ticks(3, 29, 15);
        let before_gap = vienna(2026, 3, 29, 1, 45, 0);
        let after_gap = vienna(2026, 3, 29, 3, 0, 0);

        assert!(ticks.contains(&before_gap));
        assert!(ticks.contains(&after_gap));
        assert_eq!(after_gap - before_gap, 15 * MIN);
    }

    #[test]
    fn ticks_include_both_ends_when_they_are_on_the_grid_and_otherwise_stay_inside() {
        let at = |h, m, s| vienna(2026, 6, 15, h, m, s);

        let on_grid = ticks_in(at(10, 0, 0), at(10, 15, 0), 5, &Vienna);
        let off_grid = ticks_in(at(10, 2, 0), at(10, 18, 0), 5, &Vienna);
        let none = ticks_in(at(10, 1, 0), at(10, 4, 0), 5, &Vienna);

        assert_eq!(
            on_grid,
            [at(10, 0, 0), at(10, 5, 0), at(10, 10, 0), at(10, 15, 0)]
        );
        assert_eq!(off_grid, [at(10, 5, 0), at(10, 10, 0), at(10, 15, 0)]);
        assert_eq!(none, Vec::<Ms>::new());
    }

    #[test]
    fn every_tick_is_unchanged_by_snapping() {
        for (month, day, minutes) in [
            (6, 15, 5),
            (3, 29, 5),
            (10, 25, 5),
            (10, 25, 15),
            (6, 15, 1),
        ] {
            for tick in day_ticks(month, day, minutes) {
                assert_eq!(
                    snap_in(tick, minutes, &Vienna),
                    tick,
                    "{month}-{day} at {minutes} min: {tick}"
                );
            }
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
    fn default_span_on_no_day_or_today_is_the_default_span() {
        for now in [at(13, 12), at(13, 15), midnight().1 + 10 * MIN] {
            let expected = default_span(now, 5);
            assert_eq!(default_span_on(None, now, 5), expected, "{now}");
            assert_eq!(
                default_span_on(Some(Day::of(now)), now, 5),
                expected,
                "{now}"
            );
        }
    }

    #[test]
    fn default_span_on_another_day_keeps_the_clock_times_and_length() {
        let now = at(13, 12);
        let span = default_span(now, 5);
        for day in [
            Day::of(now).next(),
            Day::of(now).prev(),
            Day::of(now).next().next(),
        ] {
            let on_day = default_span_on(Some(day), now, 5);

            assert_eq!(on_day.start, same_time_on(span.start, day), "{day}");
            assert_eq!(on_day.end - on_day.start, span.end - span.start, "{day}");
            assert_eq!(Day::of(on_day.start), day, "{day}");
        }
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
}
