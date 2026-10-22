//! The sidebar's date navigator and date picker: which days a shown period covers, and a month to pick from.

use chrono::{Months, TimeDelta};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{month_days, month_range, week_days};
use crate::db::time::Day;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum PeriodUnit {
    Day,
    Week,
    Month,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Period {
    pub from: Day,
    pub to: Day,
    pub holds_today: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct PickerMonth {
    pub month_start: Day,
    /// Monday to Sunday rows covering the month.
    pub weeks: Vec<Vec<PickerDay>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct PickerDay {
    pub day: Day,
    /// The day belongs to the month before or after.
    pub outside: bool,
    pub is_today: bool,
    /// Inside the selected days.
    pub selected: bool,
}

/// The day, week (Monday to Sunday) or month that holds `day`.
pub fn period(unit: PeriodUnit, day: Day, today: Day) -> Period {
    let (from, to) = match unit {
        PeriodUnit::Day => (day, day),
        PeriodUnit::Week => week_days(day),
        PeriodUnit::Month => month_range(day),
    };
    Period {
        from,
        to,
        holds_today: from <= today && today <= to,
    }
}

/// The day `by` days, weeks or months from `day` (back when negative); a month step lands on the 1st.
pub fn step(unit: PeriodUnit, day: Day, by: i32) -> AppResult<Day> {
    let date = day.date();
    let stepped = match unit {
        PeriodUnit::Day => date.checked_add_signed(TimeDelta::days(i64::from(by))),
        PeriodUnit::Week => date.checked_add_signed(TimeDelta::weeks(i64::from(by))),
        PeriodUnit::Month => {
            let first = day.month_start().date();
            let months = Months::new(by.unsigned_abs());
            if by < 0 {
                first.checked_sub_months(months)
            } else {
                first.checked_add_months(months)
            }
        }
    };
    stepped
        .map(Day::from_date)
        .ok_or_else(|| AppError::InvalidInput(format!("No date {by} {unit:?} steps from {day}")))
}

/// The whole weeks covering `day`'s month, with the days `from..=to` of `selected` marked.
pub fn picker_month(day: Day, selected: Option<(Day, Day)>, today: Day) -> PickerMonth {
    let first = day.month_start();
    let (start, end) = month_days(day);
    let days: Vec<PickerDay> = Day::range(&start, &end)
        .into_iter()
        .map(|d| PickerDay {
            day: d,
            outside: d.month_start() != first,
            is_today: d == today,
            selected: selected.is_some_and(|(from, to)| from <= d && d <= to),
        })
        .collect();
    PickerMonth {
        month_start: first,
        weeks: days.chunks(7).map(<[PickerDay]>::to_vec).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    fn span(unit: PeriodUnit, d: &str, today: &str) -> Period {
        period(unit, day(d), day(today))
    }

    fn flat(month: &PickerMonth) -> Vec<PickerDay> {
        month.weeks.iter().flatten().copied().collect()
    }

    #[test]
    fn a_day_period_is_the_day_itself() {
        let p = span(PeriodUnit::Day, "2026-10-20", "2026-10-20");

        assert_eq!(
            p,
            Period {
                from: day("2026-10-20"),
                to: day("2026-10-20"),
                holds_today: true,
            }
        );
    }

    #[test]
    fn periods_cover_the_day_week_or_month_that_holds_the_day() {
        for (unit, d, from, to) in [
            (PeriodUnit::Week, "2026-10-22", "2026-10-19", "2026-10-25"),
            (PeriodUnit::Week, "2026-10-25", "2026-10-19", "2026-10-25"),
            (PeriodUnit::Week, "2026-12-30", "2026-12-28", "2027-01-03"),
            (PeriodUnit::Week, "2027-01-01", "2026-12-28", "2027-01-03"),
            (PeriodUnit::Month, "2026-10-22", "2026-10-01", "2026-10-31"),
            (PeriodUnit::Month, "2028-02-10", "2028-02-01", "2028-02-29"),
            (PeriodUnit::Month, "2027-02-15", "2027-02-01", "2027-02-28"),
            (PeriodUnit::Month, "2026-12-31", "2026-12-01", "2026-12-31"),
        ] {
            let p = span(unit, d, "2000-01-01");

            assert_eq!((p.from, p.to), (day(from), day(to)), "{unit:?} {d}");
        }
    }

    #[test]
    fn a_period_holds_today_from_its_first_to_its_last_day() {
        for (unit, today, expected) in [
            (PeriodUnit::Week, "2026-10-18", false),
            (PeriodUnit::Week, "2026-10-19", true),
            (PeriodUnit::Week, "2026-10-25", true),
            (PeriodUnit::Week, "2026-10-26", false),
            (PeriodUnit::Month, "2026-09-30", false),
            (PeriodUnit::Month, "2026-10-01", true),
            (PeriodUnit::Month, "2026-10-31", true),
            (PeriodUnit::Month, "2026-11-01", false),
            (PeriodUnit::Day, "2026-10-21", false),
        ] {
            let p = span(unit, "2026-10-22", today);

            assert_eq!(p.holds_today, expected, "{unit:?} {today}");
        }
    }

    #[test]
    fn stepping_moves_by_days_weeks_or_months_in_both_directions() {
        for (unit, from, by, expected) in [
            (PeriodUnit::Day, "2026-10-22", 1, "2026-10-23"),
            (PeriodUnit::Day, "2026-10-22", -1, "2026-10-21"),
            (PeriodUnit::Day, "2026-10-22", 0, "2026-10-22"),
            (PeriodUnit::Day, "2026-10-31", 1, "2026-11-01"),
            (PeriodUnit::Day, "2028-02-28", 1, "2028-02-29"),
            (PeriodUnit::Day, "2026-10-22", 10, "2026-11-01"),
            (PeriodUnit::Week, "2026-10-22", 1, "2026-10-29"),
            (PeriodUnit::Week, "2026-10-22", -1, "2026-10-15"),
            (PeriodUnit::Week, "2026-10-22", -3, "2026-10-01"),
            (PeriodUnit::Month, "2026-10-22", 1, "2026-11-01"),
            (PeriodUnit::Month, "2026-10-22", -1, "2026-09-01"),
            (PeriodUnit::Month, "2026-10-22", 0, "2026-10-01"),
            (PeriodUnit::Month, "2026-10-31", 1, "2026-11-01"),
            (PeriodUnit::Month, "2026-03-31", -1, "2026-02-01"),
            (PeriodUnit::Month, "2026-10-22", 16, "2028-02-01"),
        ] {
            assert_eq!(
                step(unit, day(from), by).unwrap(),
                day(expected),
                "{unit:?} {from} {by}"
            );
        }
    }

    #[test]
    fn stepping_crosses_the_year_end() {
        for (unit, from, by, expected) in [
            (PeriodUnit::Day, "2026-12-31", 1, "2027-01-01"),
            (PeriodUnit::Day, "2027-01-01", -1, "2026-12-31"),
            (PeriodUnit::Week, "2026-12-30", 1, "2027-01-06"),
            (PeriodUnit::Week, "2027-01-02", -1, "2026-12-26"),
            (PeriodUnit::Month, "2026-12-15", 1, "2027-01-01"),
            (PeriodUnit::Month, "2027-01-15", -1, "2026-12-01"),
            (PeriodUnit::Month, "2026-11-30", 14, "2028-01-01"),
        ] {
            assert_eq!(
                step(unit, day(from), by).unwrap(),
                day(expected),
                "{unit:?} {from} {by}"
            );
        }
    }

    #[test]
    fn stepping_past_the_last_representable_date_is_invalid_input() {
        let last = Day::from_date(chrono::NaiveDate::MAX);
        let first = Day::from_date(chrono::NaiveDate::MIN);
        for (unit, from, by) in [
            (PeriodUnit::Day, last, 1),
            (PeriodUnit::Week, last, 1),
            (PeriodUnit::Month, last, 1),
            (PeriodUnit::Day, first, -1),
            (PeriodUnit::Week, first, -1),
            (PeriodUnit::Month, first, -1),
            (PeriodUnit::Day, day("2026-10-22"), i32::MAX),
            (PeriodUnit::Month, day("2026-10-22"), i32::MIN),
        ] {
            let err = step(unit, from, by).unwrap_err();

            assert_eq!(
                err.kind(),
                crate::error::ErrorKind::InvalidInput,
                "{unit:?} {by}"
            );
        }
    }

    #[test]
    fn a_period_serializes_with_camel_case_keys() {
        let p = span(PeriodUnit::Week, "2026-10-22", "2026-10-22");

        assert_eq!(
            serde_json::to_value(p).unwrap(),
            serde_json::json!({
                "from": "2026-10-19",
                "to": "2026-10-25",
                "holdsToday": true
            })
        );
    }

    #[test]
    fn the_picker_month_is_whole_monday_to_sunday_weeks() {
        for (d, first, last, weeks) in [
            ("2026-10-22", "2026-09-28", "2026-11-01", 5),
            ("2028-02-10", "2028-01-31", "2028-03-05", 5),
            ("2027-02-15", "2027-02-01", "2027-02-28", 4),
        ] {
            let month = picker_month(day(d), None, day("2000-01-01"));

            let days = flat(&month);
            assert_eq!(month.weeks.len(), weeks, "{d}");
            assert!(month.weeks.iter().all(|w| w.len() == 7), "{d}");
            assert_eq!(days.first().unwrap().day, day(first), "{d}");
            assert_eq!(days.last().unwrap().day, day(last), "{d}");
            assert_eq!(month.month_start, day(d).month_start(), "{d}");
        }
    }

    #[test]
    fn days_of_the_neighbouring_months_are_outside() {
        let month = picker_month(day("2026-10-22"), None, day("2000-01-01"));

        let outside: Vec<Day> = flat(&month)
            .into_iter()
            .filter(|d| d.outside)
            .map(|d| d.day)
            .collect();
        assert_eq!(
            outside,
            [
                day("2026-09-28"),
                day("2026-09-29"),
                day("2026-09-30"),
                day("2026-11-01")
            ]
        );
    }

    #[test]
    fn a_month_that_fills_its_weeks_has_no_outside_days() {
        let month = picker_month(day("2027-02-15"), None, day("2000-01-01"));

        assert!(flat(&month).iter().all(|d| !d.outside));
    }

    #[test]
    fn only_today_is_marked_as_today() {
        for today in ["2026-10-22", "2026-09-29"] {
            let month = picker_month(day("2026-10-22"), None, day(today));

            let marked: Vec<Day> = flat(&month)
                .into_iter()
                .filter(|d| d.is_today)
                .map(|d| d.day)
                .collect();
            assert_eq!(marked, [day(today)], "{today}");
        }
    }

    #[test]
    fn today_outside_the_grid_marks_nothing() {
        let month = picker_month(day("2026-10-22"), None, day("2026-12-01"));

        assert!(flat(&month).iter().all(|d| !d.is_today));
    }

    #[test]
    fn a_selected_span_marks_exactly_its_days() {
        let month = picker_month(
            day("2026-10-22"),
            Some((day("2026-10-10"), day("2026-10-12"))),
            day("2000-01-01"),
        );

        let marked: Vec<Day> = flat(&month)
            .into_iter()
            .filter(|d| d.selected)
            .map(|d| d.day)
            .collect();
        assert_eq!(
            marked,
            [day("2026-10-10"), day("2026-10-11"), day("2026-10-12")]
        );
    }

    #[test]
    fn a_selected_span_reaches_into_the_outside_days() {
        let month = picker_month(
            day("2026-10-22"),
            Some((day("2026-09-20"), day("2026-09-29"))),
            day("2000-01-01"),
        );

        let marked: Vec<Day> = flat(&month)
            .into_iter()
            .filter(|d| d.selected)
            .map(|d| d.day)
            .collect();
        assert_eq!(marked, [day("2026-09-28"), day("2026-09-29")]);
    }

    #[test]
    fn no_selection_marks_nothing() {
        let month = picker_month(day("2026-10-22"), None, day("2026-10-22"));

        assert!(flat(&month).iter().all(|d| !d.selected));
    }

    #[test]
    fn a_picker_day_serializes_with_camel_case_keys() {
        let month = picker_month(day("2027-02-15"), None, day("2027-02-01"));

        assert_eq!(
            serde_json::to_value(month.weeks[0][0]).unwrap(),
            serde_json::json!({
                "day": "2027-02-01",
                "outside": false,
                "isToday": true,
                "selected": false
            })
        );
    }
}
