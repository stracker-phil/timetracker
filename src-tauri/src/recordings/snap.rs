//! Snapping times to the `timeline.snapMinutes` grid.

use chrono::{Duration, Local, LocalResult, NaiveTime, TimeZone, Timelike};

use super::{MINUTE_MS, Span};
use crate::db::time::Ms;

/// `ms` moved to the nearest multiple of `minutes` counted from its local midnight, so DST days still land
/// on round wall-clock minutes. A tie rounds up.
pub fn snap(ms: Ms, minutes: u32) -> Ms {
    snap_in(ms, minutes, &Local)
}

pub fn snap_in<Tz: TimeZone>(ms: Ms, minutes: u32, tz: &Tz) -> Ms {
    let step = i64::from(minutes.max(1)) * MINUTE_MS;
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

/// How long a recording made with New Recording lasts by default.
pub const NEW_RECORDING_MINUTES: i64 = 30;

/// The span a new recording without a selection proposes: the half hour up to `now`, snapped.
pub fn default_span(now: Ms, minutes: u32) -> Span {
    snap_span(now - NEW_RECORDING_MINUTES * MINUTE_MS, now, minutes)
}

/// An end that snapped onto or before the start lands one step after it.
fn past_start(start: Ms, end: Ms, minutes: u32) -> Ms {
    let step = i64::from(minutes.max(1)) * MINUTE_MS;
    if end <= start { start + step } else { end }
}

#[cfg(test)]
mod tests {
    use chrono_tz::Europe::Vienna;

    use super::*;
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
