//! A report's range: presets and custom days, resolved to days and stepped by the sidebar's navigator.

use chrono::{Days, Months};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::db::time::Day;
use crate::error::{AppError, AppResult};

/// Presets stay relative, so a saved "this month" report shows the current month.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum Range {
    Today,
    Yesterday,
    ThisWeek,
    LastWeek,
    ThisMonth,
    LastMonth,
    Custom { from: Day, to: Day },
}

/// The first and last day of `range`; weeks start on Monday.
pub fn resolve(range: &Range, today: Day) -> AppResult<(Day, Day)> {
    let (from, to) = match *range {
        Range::Today => (today, today),
        Range::Yesterday => (today.prev(), today.prev()),
        Range::ThisWeek => {
            let start = today.week_start();
            (start, add_days(start, 6))
        }
        Range::LastWeek => {
            let start = add_days(today.week_start(), -7);
            (start, add_days(start, 6))
        }
        Range::ThisMonth => month(today.month_start()),
        Range::LastMonth => month(today.month_start().prev().month_start()),
        Range::Custom { from, to } => (from, to),
    };
    if from > to {
        return Err(AppError::InvalidInput(format!(
            "The range starts after it ends: {from} – {to}"
        )));
    }
    Ok((from, to))
}

/// What a range steps by: a calendar month, a Monday-to-Sunday week, or its own number of days.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    Month,
    Week,
    Days(i64),
}

fn unit((from, to): (Day, Day)) -> Unit {
    if from == from.month_start() && month(from).1 == to {
        Unit::Month
    } else if from == from.week_start() && add_days(from, 6) == to {
        Unit::Week
    } else {
        Unit::Days((to.date() - from.date()).num_days() + 1)
    }
}

const PRESETS: [Range; 6] = [
    Range::Today,
    Range::Yesterday,
    Range::ThisWeek,
    Range::LastWeek,
    Range::ThisMonth,
    Range::LastMonth,
];

/// The period of `range`'s unit `by` steps away: from the range itself, or with `anchor` from the period
/// that holds it (for days, the ones ending on it). A result that matches a preset is that preset.
pub fn shift(range: &Range, anchor: Option<Day>, by: i32, today: Day) -> AppResult<Range> {
    let current = resolve(range, today)?;
    let (from, to) = match unit(current) {
        Unit::Month => shift_month(anchor.map_or(current.0, |d| d.month_start()), by),
        Unit::Week => shift_week(anchor.map_or(current.0, |d| d.week_start()), by),
        Unit::Days(n) => shift_days(anchor.unwrap_or(current.1), n, by),
    };
    let preset = PRESETS
        .into_iter()
        .find(|p| resolve(p, today).is_ok_and(|r| r == (from, to)));
    Ok(preset.unwrap_or(Range::Custom { from, to }))
}

/// The month `by` months from the one starting on `start`.
fn shift_month(start: Day, by: i32) -> (Day, Day) {
    month(add_months(start, by))
}

/// The week `by` weeks from the one starting on `start`.
fn shift_week(start: Day, by: i32) -> (Day, Day) {
    let start = add_days(start, 7 * i64::from(by));
    (start, add_days(start, 6))
}

/// The `n` days ending on `end`, moved `by` times their length.
fn shift_days(end: Day, n: i64, by: i32) -> (Day, Day) {
    let end = add_days(end, n * i64::from(by));
    (add_days(end, 1 - n), end)
}

pub(super) fn add_days(day: Day, days: i64) -> Day {
    let date = if days >= 0 {
        day.date() + Days::new(days.unsigned_abs())
    } else {
        day.date() - Days::new(days.unsigned_abs())
    };
    Day::from_date(date)
}

fn add_months(day: Day, months: i32) -> Day {
    let date = if months >= 0 {
        day.date() + Months::new(months.unsigned_abs())
    } else {
        day.date() - Months::new(months.unsigned_abs())
    };
    Day::from_date(date)
}

fn month(start: Day) -> (Day, Day) {
    let next = start.date() + Months::new(1);
    let last = Day::from_date(next).prev();
    (start, last)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::error::ErrorKind;
    use crate::reports::testing::*;

    #[test]
    fn presets_resolve_against_a_wednesday() {
        let today = day("2026-10-21");
        for (range, from, to) in [
            (Range::Today, "2026-10-21", "2026-10-21"),
            (Range::Yesterday, "2026-10-20", "2026-10-20"),
            (Range::ThisWeek, "2026-10-19", "2026-10-25"),
            (Range::LastWeek, "2026-10-12", "2026-10-18"),
            (Range::ThisMonth, "2026-10-01", "2026-10-31"),
            (Range::LastMonth, "2026-09-01", "2026-09-30"),
        ] {
            assert_eq!(
                resolve(&range, today).unwrap(),
                (day(from), day(to)),
                "{range:?}"
            );
        }
    }

    #[test]
    fn weeks_start_on_monday_and_end_on_sunday() {
        for today in ["2026-10-19", "2026-10-25"] {
            assert_eq!(
                resolve(&Range::ThisWeek, day(today)).unwrap(),
                (day("2026-10-19"), day("2026-10-25")),
                "{today}"
            );
        }
    }

    #[test]
    fn presets_cross_the_year_end() {
        for (range, today, from, to) in [
            (Range::LastMonth, "2026-01-15", "2025-12-01", "2025-12-31"),
            (Range::ThisWeek, "2026-01-01", "2025-12-29", "2026-01-04"),
            (Range::LastWeek, "2026-01-05", "2025-12-29", "2026-01-04"),
            (Range::Yesterday, "2026-01-01", "2025-12-31", "2025-12-31"),
        ] {
            assert_eq!(
                resolve(&range, day(today)).unwrap(),
                (day(from), day(to)),
                "{range:?} on {today}"
            );
        }
    }

    #[test]
    fn months_end_on_their_last_day_in_leap_and_common_years() {
        for (range, today, from, to) in [
            (Range::ThisMonth, "2028-02-10", "2028-02-01", "2028-02-29"),
            (Range::ThisMonth, "2026-02-10", "2026-02-01", "2026-02-28"),
            (Range::LastMonth, "2026-03-31", "2026-02-01", "2026-02-28"),
            (Range::LastMonth, "2028-03-01", "2028-02-01", "2028-02-29"),
            (Range::ThisMonth, "2026-12-31", "2026-12-01", "2026-12-31"),
        ] {
            assert_eq!(
                resolve(&range, day(today)).unwrap(),
                (day(from), day(to)),
                "{range:?} on {today}"
            );
        }
    }

    #[test]
    fn a_custom_range_resolves_to_itself_and_may_be_one_day() {
        let today = day("2026-10-21");
        for (from, to) in [("2026-10-01", "2026-10-07"), ("2026-10-05", "2026-10-05")] {
            let range = Range::Custom {
                from: day(from),
                to: day(to),
            };
            assert_eq!(resolve(&range, today).unwrap(), (day(from), day(to)));
        }
    }

    #[test]
    fn a_custom_range_that_ends_before_it_starts_is_invalid_input() {
        let range = Range::Custom {
            from: day("2026-10-07"),
            to: day("2026-10-01"),
        };
        let err = resolve(&range, day("2026-10-21")).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn ranges_have_a_tagged_camel_case_wire_shape() {
        for (range, wire) in [
            (Range::Today, json!({ "kind": "today" })),
            (Range::ThisWeek, json!({ "kind": "thisWeek" })),
            (Range::LastMonth, json!({ "kind": "lastMonth" })),
            (
                Range::Custom {
                    from: day("2026-10-01"),
                    to: day("2026-10-07"),
                },
                json!({ "kind": "custom", "from": "2026-10-01", "to": "2026-10-07" }),
            ),
        ] {
            assert_eq!(serde_json::to_value(range).unwrap(), wire, "{range:?}");
            assert_eq!(serde_json::from_value::<Range>(wire).unwrap(), range);
        }
    }

    #[test]
    fn shifting_a_preset_lands_on_the_neighbouring_preset_when_there_is_one() {
        for (range, by, expected) in [
            (Range::Today, -1, Range::Yesterday),
            (Range::ThisWeek, -1, Range::LastWeek),
            (Range::LastWeek, 1, Range::ThisWeek),
            (Range::ThisMonth, -1, Range::LastMonth),
            (Range::LastMonth, 1, Range::ThisMonth),
        ] {
            assert_eq!(
                shift(&range, None, by, day(D)).unwrap(),
                expected,
                "{range:?} {by}"
            );
        }
    }

    #[test]
    fn shifting_a_preset_past_the_presets_gives_a_custom_range() {
        for (range, by, expected) in [
            (Range::Today, 1, custom("2026-10-21", "2026-10-21")),
            (Range::ThisMonth, -2, custom("2026-08-01", "2026-08-31")),
            (Range::ThisMonth, 1, custom("2026-11-01", "2026-11-30")),
            (Range::ThisWeek, 1, custom("2026-10-26", "2026-11-01")),
        ] {
            assert_eq!(
                shift(&range, None, by, day(D)).unwrap(),
                expected,
                "{range:?} {by}"
            );
        }
    }

    #[test]
    fn a_custom_range_moves_by_its_own_length_in_days() {
        for (by, expected) in [
            (1, custom("2026-10-11", "2026-10-20")),
            (-1, custom("2026-09-21", "2026-09-30")),
            (2, custom("2026-10-21", "2026-10-30")),
        ] {
            assert_eq!(
                shift(&custom("2026-10-01", "2026-10-10"), None, by, day(D)).unwrap(),
                expected,
                "{by}"
            );
        }
    }

    #[test]
    fn a_custom_whole_calendar_month_steps_as_a_month() {
        let february = custom("2026-02-01", "2026-02-28");

        for (by, expected) in [
            (1, custom("2026-03-01", "2026-03-31")),
            (-1, custom("2026-01-01", "2026-01-31")),
        ] {
            assert_eq!(
                shift(&february, None, by, day(D)).unwrap(),
                expected,
                "{by}"
            );
        }
    }

    #[test]
    fn a_custom_monday_to_sunday_range_steps_as_a_week() {
        let week = custom("2026-01-05", "2026-01-11");

        for (by, expected) in [
            (1, custom("2026-01-12", "2026-01-18")),
            (-1, custom("2025-12-29", "2026-01-04")),
        ] {
            assert_eq!(shift(&week, None, by, day(D)).unwrap(), expected, "{by}");
        }
    }

    #[test]
    fn a_custom_range_that_matches_a_preset_becomes_that_preset() {
        let last_week = custom("2026-10-12", "2026-10-18");

        assert_eq!(shift(&last_week, None, 1, day(D)).unwrap(), Range::ThisWeek);
    }

    #[test]
    fn an_anchor_with_no_step_gives_the_period_of_the_ranges_unit_that_holds_it() {
        let anchor = Some(day("2026-10-22"));

        for (range, expected) in [
            (Range::ThisWeek, Range::ThisWeek),
            (Range::LastMonth, Range::ThisMonth),
            (Range::Yesterday, custom("2026-10-22", "2026-10-22")),
            (custom("2026-02-01", "2026-02-28"), Range::ThisMonth),
            (custom("2026-01-05", "2026-01-11"), Range::ThisWeek),
        ] {
            assert_eq!(
                shift(&range, anchor, 0, day(D)).unwrap(),
                expected,
                "{range:?}"
            );
        }
    }

    #[test]
    fn an_anchor_on_today_with_a_one_day_range_gives_today() {
        assert_eq!(
            shift(&Range::Yesterday, Some(day(D)), 0, day(D)).unwrap(),
            Range::Today
        );
    }

    #[test]
    fn an_anchor_with_a_custom_day_range_gives_the_days_ending_on_it() {
        assert_eq!(
            shift(&custom("2026-09-01", "2026-09-10"), Some(day(D)), 0, day(D)).unwrap(),
            custom("2026-10-11", "2026-10-20")
        );
    }

    #[test]
    fn shifting_an_invalid_custom_range_is_invalid_input() {
        let err = shift(&custom("2026-10-07", "2026-10-01"), None, 1, day(D)).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }
}
