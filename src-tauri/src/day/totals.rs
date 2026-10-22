//! The day's recorded, tracked and away time.

use super::{DayInput, DayTotals, TRACKER_SOURCE};
use crate::db::activities::ActivityRow;
use crate::db::time::{Ms, clip};

/// Recorded, tracked and away time inside the day; `names` is not read.
pub fn totals(input: &DayInput) -> DayTotals {
    let clipped = |start: Ms, end: Option<Ms>| clip(start, end, input.bounds, input.now);
    DayTotals {
        recorded_ms: input
            .recordings
            .iter()
            .flat_map(|r| &r.spans)
            .filter_map(|s| clipped(s.start, s.end))
            .map(|(start, end)| end - start)
            .sum(),
        tracked_ms: tracked_ms(&input.activities, input.bounds, input.now),
        away_ms: union_length(
            input
                .away
                .iter()
                .filter_map(|a| clipped(a.start, a.end))
                .collect(),
        ),
    }
}

/// Sorted spans with overlapping and touching ones merged.
pub(super) fn union(mut spans: Vec<(Ms, Ms)>) -> Vec<(Ms, Ms)> {
    spans.sort_unstable();
    let mut merged: Vec<(Ms, Ms)> = Vec::with_capacity(spans.len());
    for (start, end) in spans {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

/// Tracker activity inside `bounds`, overlaps counted once; external sources are not tracked time.
pub fn tracked_ms(activities: &[ActivityRow], bounds: (Ms, Ms), now: Ms) -> Ms {
    union_length(
        activities
            .iter()
            .filter(|a| a.source == TRACKER_SOURCE)
            .filter_map(|a| clip(a.start, a.end, bounds, now))
            .collect(),
    )
}

pub(super) fn union_length(spans: Vec<(Ms, Ms)>) -> Ms {
    union(spans).iter().map(|(start, end)| end - start).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::day::testing::*;
    use crate::day::view;
    use crate::db::away::AwayReason;

    #[test]
    fn external_activity_adds_no_tracked_time_between_tracker_spans() {
        let rows = vec![
            code(1, 0, 10),
            from_source(code(2, 10, 40), "external:planner"),
            code(3, 40, 50),
        ];

        let view = view(&input(rows, vec![]), &settings());

        assert_eq!(view.totals.tracked_ms, mins(20));
    }

    #[test]
    fn overlapping_tracker_spans_count_once_and_external_ones_not_at_all() {
        let rows = vec![
            code(1, 0, 60),
            code(2, 30, 90),
            from_source(code(3, 0, 120), "external:planner"),
        ];

        let view = view(&input(rows, vec![]), &settings());

        assert_eq!(
            view.totals,
            DayTotals {
                recorded_ms: 0,
                tracked_ms: mins(90),
                away_ms: 0
            }
        );
    }

    #[test]
    fn tracked_ms_counts_overlapping_tracker_activity_once() {
        let activities = rows(&[
            (0, 60, "Code", "a"),
            (30, 90, "Mail", "b"),
            (120, 150, "Code", "c"),
        ]);

        assert_eq!(
            tracked_ms(&activities, (0, DAY_LENGTH), NOW),
            mins(90) + mins(30)
        );
    }

    #[test]
    fn tracked_ms_excludes_external_sources() {
        let activities = vec![
            activity(1, 0, Some(mins(60)), "Code", "a"),
            from_source(
                activity(2, mins(30), Some(mins(120)), "Cal", "m"),
                "external:calendar",
            ),
        ];

        assert_eq!(tracked_ms(&activities, (0, DAY_LENGTH), NOW), mins(60));
    }

    #[test]
    fn tracked_ms_is_clipped_to_the_bounds() {
        let activities = rows(&[
            (0, 60, "Code", "a"),
            (100, 200, "Code", "b"),
            (300, 400, "Code", "c"),
        ]);

        assert_eq!(
            tracked_ms(&activities, (mins(30), mins(150)), NOW),
            mins(30) + mins(50)
        );
    }

    #[test]
    fn tracked_ms_of_an_open_span_runs_to_now_within_the_bounds() {
        let activities = vec![activity(1, mins(60), None, "Code", "a")];

        for (now, bounds_end, expected) in [
            (mins(90), DAY_LENGTH, mins(30)),
            (3 * DAY_LENGTH, DAY_LENGTH, DAY_LENGTH - mins(60)),
            (mins(30), DAY_LENGTH, 0),
        ] {
            assert_eq!(
                tracked_ms(&activities, (0, bounds_end), now),
                expected,
                "now {now}"
            );
        }
    }

    #[test]
    fn tracked_ms_of_nothing_is_zero() {
        assert_eq!(tracked_ms(&[], (0, DAY_LENGTH), NOW), 0);
    }

    fn busy_input() -> DayInput {
        DayInput {
            bounds: (HOUR, 3 * HOUR),
            now: 2 * HOUR + mins(30),
            recordings: vec![
                recording("r1", 0, Some(mins(90))),
                recording("r2", mins(100), Some(mins(110))),
                recording("r3", mins(140), None),
                recording("r4", 4 * HOUR, Some(5 * HOUR)),
            ],
            ..input(
                vec![
                    activity(1, 0, Some(mins(90)), "Code", ""),
                    activity(2, mins(80), Some(mins(100)), "Mail", ""),
                    activity(3, mins(150), Some(mins(170)), "Code", ""),
                    activity(4, 4 * HOUR, Some(5 * HOUR), "Code", ""),
                ],
                vec![
                    away(1, mins(70), Some(mins(100)), AwayReason::Idle),
                    away(2, mins(90), Some(mins(130)), AwayReason::Sleep),
                    away(3, mins(140), None, AwayReason::Lock),
                ],
            )
        }
    }

    #[test]
    fn totals_count_recorded_tracked_and_away_time_inside_the_day() {
        assert_eq!(
            totals(&busy_input()),
            DayTotals {
                recorded_ms: mins(30) + mins(10) + mins(10),
                tracked_ms: mins(40) + mins(20),
                away_ms: mins(60) + mins(10),
            }
        );
    }

    #[test]
    fn a_running_recording_in_totals_ends_at_now() {
        let input = DayInput {
            now: 2 * HOUR,
            ..input_with_recordings(vec![recording("r1", HOUR, None)])
        };

        assert_eq!(totals(&input).recorded_ms, HOUR);
    }

    #[test]
    fn overlapping_away_spans_count_once_in_totals() {
        let input = input(
            vec![],
            vec![
                away(1, mins(60), Some(mins(120)), AwayReason::Idle),
                away(2, mins(90), Some(mins(180)), AwayReason::Sleep),
            ],
        );

        assert_eq!(totals(&input).away_ms, mins(120));
    }

    #[test]
    fn totals_of_an_empty_day_are_zero() {
        assert_eq!(totals(&input(vec![], vec![])), DayTotals::default());
    }

    #[test]
    fn totals_equal_the_totals_of_the_view() {
        for input in [busy_input(), input(vec![], vec![])] {
            assert_eq!(totals(&input), view(&input, &settings()).totals);
        }
    }
}
