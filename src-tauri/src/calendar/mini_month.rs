//! The sidebar's month calendar: six Monday-first weeks with ISO week numbers, the days with recordings,
//! and the shown day, week or month highlighted.

use std::collections::BTreeSet;

use chrono::{Datelike, Months};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::period::PeriodUnit;
use super::titles::month_name;
use super::{add_days, month_range, week_days};
use crate::db::time::{Day, Ms};
use crate::db::{Db, recordings};
use crate::error::{AppError, AppResult};
use crate::reports::{Range, resolve};
use crate::store::settings::CalendarTitle;

/// Every mini month has six rows, so the sidebar never changes height.
const WEEKS: u64 = 6;

/// What the main view shows, which the calendar highlights.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum Shown {
    /// The Day, Week or Month view of `day`.
    Period { unit: PeriodUnit, day: Day },
    /// A report's range: highlighted when it is one day, one week or one month.
    Report { range: Range },
    /// A view without dates.
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct MiniMonth {
    pub month_start: Day,
    /// By the calendar title setting (`titles::month_name`).
    pub month_name: String,
    pub year: i32,
    /// This month is the highlighted range.
    pub month_selected: bool,
    pub weeks: Vec<MiniWeek>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct MiniWeek {
    /// The ISO week number.
    pub number: u32,
    /// Its Monday.
    pub start: Day,
    /// This week is the highlighted range.
    pub selected: bool,
    pub days: Vec<MiniDay>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct MiniDay {
    pub day: Day,
    pub day_of_month: u32,
    pub in_month: bool,
    pub is_today: bool,
    pub has_recordings: bool,
    /// This day is the highlighted range.
    pub selected: bool,
}

/// The highlighted range: its kind and first day.
pub type Highlight = (PeriodUnit, Day);

/// The sidebar's calendar `offset` months from `shown_day`'s, with the days that have recordings and `shown`
/// highlighted. Without a data store it still navigates, without dots.
pub async fn load_mini_month(
    db: Option<&Db>,
    shown_day: Day,
    offset: i32,
    shown: Shown,
    title: CalendarTitle,
    today: Day,
    now: Ms,
) -> AppResult<MiniMonth> {
    let month = add_months(shown_day.month_start(), offset)?;
    let (from, to) = mini_month_days(month);
    let days = Day::range(&from, &to);
    let spans = match db {
        Some(db) => recordings::spans_in(db, from.bounds().0, to.bounds().1).await?,
        None => Vec::new(),
    };
    let recorded = recorded_days(&spans, &days, now);
    Ok(mini_month(
        month,
        highlight(&shown, today)?,
        &days,
        &recorded,
        today,
        title,
    ))
}

fn add_months(first: Day, offset: i32) -> AppResult<Day> {
    let months = Months::new(offset.unsigned_abs());
    let date = if offset < 0 {
        first.date().checked_sub_months(months)
    } else {
        first.date().checked_add_months(months)
    };
    date.map(Day::from_date)
        .ok_or_else(|| AppError::InvalidInput(format!("No month {offset} months from {first}")))
}

/// The first and the last day the mini month of `month` covers.
pub fn mini_month_days(month: Day) -> (Day, Day) {
    let start = month.month_start().week_start();
    (start, add_days(start, WEEKS * 7 - 1))
}

/// The range `shown` highlights; a report range only when it is one day, week or month.
pub fn highlight(shown: &Shown, today: Day) -> AppResult<Option<Highlight>> {
    Ok(match *shown {
        Shown::Period { unit, day } => Some((unit, start_of(unit, day))),
        Shown::Report { range } => unit_of(resolve(&range, today)?),
        Shown::None => None,
    })
}

fn start_of(unit: PeriodUnit, day: Day) -> Day {
    match unit {
        PeriodUnit::Day => day,
        PeriodUnit::Week => day.week_start(),
        PeriodUnit::Month => day.month_start(),
    }
}

fn unit_of((from, to): (Day, Day)) -> Option<Highlight> {
    let unit = if from == to {
        PeriodUnit::Day
    } else if week_days(from) == (from, to) {
        PeriodUnit::Week
    } else if month_range(from) == (from, to) {
        PeriodUnit::Month
    } else {
        return None;
    };
    Some((unit, from))
}

/// The days of `days` that share time with a span; an open span runs until `now`.
pub fn recorded_days(spans: &[(Ms, Option<Ms>)], days: &[Day], now: Ms) -> BTreeSet<Day> {
    days.iter()
        .copied()
        .filter(|day| {
            let (start, end) = day.bounds();
            spans
                .iter()
                .any(|&(s, e)| s < end && e.unwrap_or(now) > start)
        })
        .collect()
}

/// `days` are the `mini_month_days` of `month`.
pub fn mini_month(
    month: Day,
    range: Option<Highlight>,
    days: &[Day],
    recorded: &BTreeSet<Day>,
    today: Day,
    title: CalendarTitle,
) -> MiniMonth {
    let first = month.month_start();
    let is = |unit: PeriodUnit, day: Day| range == Some((unit, day));
    let days: Vec<MiniDay> = days
        .iter()
        .map(|&day| MiniDay {
            day,
            day_of_month: day.date().day(),
            in_month: day.month_start() == first,
            is_today: day == today,
            has_recordings: recorded.contains(&day),
            selected: is(PeriodUnit::Day, day),
        })
        .collect();
    MiniMonth {
        month_start: first,
        month_name: month_name(first, title),
        year: first.date().year(),
        month_selected: is(PeriodUnit::Month, first),
        weeks: days
            .chunks(7)
            .map(|week| MiniWeek {
                number: week[0].day.date().iso_week().week(),
                start: week[0].day,
                selected: is(PeriodUnit::Week, week[0].day),
                days: week.to_vec(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::time::now_ms;
    use crate::error::ErrorKind;

    const TODAY: &str = "2026-11-11";

    fn day(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    fn period(unit: PeriodUnit, s: &str) -> Shown {
        Shown::Period { unit, day: day(s) }
    }

    fn custom(from: &str, to: &str) -> Shown {
        Shown::Report {
            range: Range::Custom {
                from: day(from),
                to: day(to),
            },
        }
    }

    async fn load(
        db: Option<&Db>,
        month: &str,
        shown: Shown,
        title: CalendarTitle,
    ) -> AppResult<MiniMonth> {
        load_mini_month(db, day(month), 0, shown, title, day(TODAY), now_ms()).await
    }

    async fn grid(month: &str, shown: Shown) -> MiniMonth {
        load(None, month, shown, CalendarTitle::Short)
            .await
            .unwrap()
    }

    fn all_days(month: &MiniMonth) -> Vec<MiniDay> {
        month.weeks.iter().flat_map(|w| w.days.clone()).collect()
    }

    fn starts(month: &MiniMonth) -> Vec<(Day, u32)> {
        month.weeks.iter().map(|w| (w.start, w.number)).collect()
    }

    fn selected_days(month: &MiniMonth) -> Vec<Day> {
        all_days(month)
            .into_iter()
            .filter(|d| d.selected)
            .map(|d| d.day)
            .collect()
    }

    fn selected_weeks(month: &MiniMonth) -> Vec<Day> {
        month
            .weeks
            .iter()
            .filter(|w| w.selected)
            .map(|w| w.start)
            .collect()
    }

    #[tokio::test]
    async fn a_month_always_shows_six_monday_first_weeks_of_seven_days() {
        for (given, first, last) in [
            ("2026-11-15", "2026-10-26", "2026-12-06"),
            ("2027-01-20", "2026-12-28", "2027-02-07"),
            ("2027-02-10", "2027-02-01", "2027-03-14"),
            ("2028-02-10", "2028-01-31", "2028-03-12"),
            ("2026-12-01", "2026-11-30", "2027-01-10"),
        ] {
            let month = grid(given, Shown::None).await;

            let days: Vec<Day> = all_days(&month).iter().map(|d| d.day).collect();
            assert_eq!(month.weeks.len(), 6, "{given}");
            assert!(month.weeks.iter().all(|w| w.days.len() == 7), "{given}");
            assert_eq!(days, Day::range(&day(first), &day(last)), "{given}");
            assert_eq!(
                mini_month_days(day(given)),
                (day(first), day(last)),
                "{given}"
            );
        }
    }

    #[tokio::test]
    async fn weeks_carry_their_monday_and_iso_week_number() {
        for (given, expected) in [
            (
                "2026-12-15",
                vec![
                    ("2026-11-30", 49),
                    ("2026-12-07", 50),
                    ("2026-12-14", 51),
                    ("2026-12-21", 52),
                    ("2026-12-28", 53),
                    ("2027-01-04", 1),
                ],
            ),
            (
                "2027-01-15",
                vec![
                    ("2026-12-28", 53),
                    ("2027-01-04", 1),
                    ("2027-01-11", 2),
                    ("2027-01-18", 3),
                    ("2027-01-25", 4),
                    ("2027-02-01", 5),
                ],
            ),
            (
                "2026-01-15",
                vec![
                    ("2025-12-29", 1),
                    ("2026-01-05", 2),
                    ("2026-01-12", 3),
                    ("2026-01-19", 4),
                    ("2026-01-26", 5),
                    ("2026-02-02", 6),
                ],
            ),
        ] {
            let month = grid(given, Shown::None).await;

            let expected: Vec<(Day, u32)> = expected.iter().map(|&(d, n)| (day(d), n)).collect();
            assert_eq!(starts(&month), expected, "{given}");
        }
    }

    #[tokio::test]
    async fn days_outside_the_month_are_flagged_and_numbered_by_day_of_month() {
        let month = grid("2026-11-15", Shown::None).await;

        let days = all_days(&month);
        assert_eq!(days.iter().filter(|d| d.in_month).count(), 30);
        assert_eq!(days[0].day, day("2026-10-26"));
        assert_eq!((days[0].day_of_month, days[0].in_month), (26, false));
        assert_eq!((days[6].day_of_month, days[6].in_month), (1, true));
        assert_eq!((days[35].day_of_month, days[35].in_month), (30, true));
        assert_eq!((days[36].day_of_month, days[36].in_month), (1, false));
    }

    #[tokio::test]
    async fn only_today_is_flagged_today() {
        let current = grid("2026-11-20", Shown::None).await;
        let distant = grid("2020-02-10", Shown::None).await;

        let flagged: Vec<Day> = all_days(&current)
            .into_iter()
            .filter(|d| d.is_today)
            .map(|d| d.day)
            .collect();
        assert_eq!(flagged, [day(TODAY)]);
        assert!(all_days(&distant).iter().all(|d| !d.is_today));
    }

    #[tokio::test]
    async fn the_offset_moves_the_calendar_by_whole_months_from_the_shown_day() {
        for (offset, first) in [
            (0, "2026-11-01"),
            (1, "2026-12-01"),
            (2, "2027-01-01"),
            (-1, "2026-10-01"),
            (-11, "2025-12-01"),
        ] {
            let month = load_mini_month(
                None,
                day("2026-11-30"),
                offset,
                Shown::None,
                CalendarTitle::Short,
                day(TODAY),
                now_ms(),
            )
            .await
            .unwrap();

            assert_eq!(month.month_start, day(first), "{offset}");
            assert_eq!(month.weeks.len(), 6, "{offset}");
        }
    }

    #[tokio::test]
    async fn the_month_is_named_by_the_calendar_title_with_its_year() {
        for (title, name) in [
            (CalendarTitle::Short, "Nov"),
            (CalendarTitle::Full, "November"),
        ] {
            let month = load(None, "2026-11-15", Shown::None, title).await.unwrap();

            assert_eq!(month.month_name, name);
            assert_eq!(month.year, 2026);
            assert_eq!(month.month_start, day("2026-11-01"));
        }
    }

    #[tokio::test]
    async fn no_recordings_leave_every_day_without_a_dot() {
        let month = grid("2026-11-15", Shown::None).await;

        assert!(all_days(&month).iter().all(|d| !d.has_recordings));
    }

    #[tokio::test]
    async fn nothing_is_marked_when_nothing_is_shown() {
        let month = grid("2026-11-15", Shown::None).await;

        assert!(selected_days(&month).is_empty());
        assert!(selected_weeks(&month).is_empty());
        assert!(!month.month_selected);
    }

    #[tokio::test]
    async fn a_shown_day_marks_exactly_that_day() {
        let month = grid("2026-11-15", period(PeriodUnit::Day, "2026-11-11")).await;

        assert_eq!(selected_days(&month), [day("2026-11-11")]);
        assert!(selected_weeks(&month).is_empty());
        assert!(!month.month_selected);
    }

    #[tokio::test]
    async fn a_shown_day_of_a_neighboring_month_is_marked_in_the_grid() {
        let month = grid("2026-11-15", period(PeriodUnit::Day, "2026-10-28")).await;

        assert_eq!(selected_days(&month), [day("2026-10-28")]);
        assert!(!month.month_selected);
    }

    #[tokio::test]
    async fn a_shown_week_marks_exactly_its_row_from_any_of_its_days() {
        for given in ["2026-11-09", "2026-11-11", "2026-11-15"] {
            let month = grid("2026-11-15", period(PeriodUnit::Week, given)).await;

            assert_eq!(selected_weeks(&month), [day("2026-11-09")], "{given}");
            assert!(selected_days(&month).is_empty(), "{given}");
            assert!(!month.month_selected, "{given}");
        }
    }

    #[tokio::test]
    async fn a_shown_week_in_a_neighboring_months_rows_is_still_marked() {
        let month = grid("2026-11-15", period(PeriodUnit::Week, "2026-12-02")).await;

        assert_eq!(selected_weeks(&month), [day("2026-11-30")]);
    }

    #[tokio::test]
    async fn a_shown_month_marks_the_title_from_any_of_its_days() {
        for given in ["2026-11-01", "2026-11-20", "2026-11-30"] {
            let month = grid("2026-11-15", period(PeriodUnit::Month, given)).await;

            assert!(month.month_selected, "{given}");
            assert!(selected_days(&month).is_empty(), "{given}");
            assert!(selected_weeks(&month).is_empty(), "{given}");
        }
    }

    #[tokio::test]
    async fn a_shown_month_other_than_the_displayed_one_leaves_the_title_unmarked() {
        for given in ["2026-10-20", "2026-12-01"] {
            let month = grid("2026-11-15", period(PeriodUnit::Month, given)).await;

            assert!(!month.month_selected, "{given}");
        }
    }

    #[tokio::test]
    async fn a_report_range_of_one_week_marks_that_week() {
        let month = grid("2026-11-15", custom("2026-11-09", "2026-11-15")).await;

        assert_eq!(selected_weeks(&month), [day("2026-11-09")]);
        assert!(selected_days(&month).is_empty());
    }

    #[tokio::test]
    async fn an_invalid_report_range_is_rejected() {
        let err = load(
            None,
            "2026-11-15",
            custom("2026-11-15", "2026-11-09"),
            CalendarTitle::Short,
        )
        .await
        .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn a_shown_period_highlights_its_unit_and_first_day() {
        let today = day("2026-11-11");
        for (shown, expected) in [
            (
                period(PeriodUnit::Day, "2026-11-11"),
                (PeriodUnit::Day, "2026-11-11"),
            ),
            (
                period(PeriodUnit::Week, "2026-11-11"),
                (PeriodUnit::Week, "2026-11-09"),
            ),
            (
                period(PeriodUnit::Week, "2026-11-15"),
                (PeriodUnit::Week, "2026-11-09"),
            ),
            (
                period(PeriodUnit::Week, "2026-11-09"),
                (PeriodUnit::Week, "2026-11-09"),
            ),
            (
                period(PeriodUnit::Month, "2026-11-20"),
                (PeriodUnit::Month, "2026-11-01"),
            ),
        ] {
            assert_eq!(
                highlight(&shown, today).unwrap(),
                Some((expected.0, day(expected.1))),
                "{shown:?}"
            );
        }
    }

    #[test]
    fn named_report_ranges_that_are_a_day_week_or_month_are_highlighted() {
        let today = day("2026-11-11");
        for (range, expected) in [
            (Range::Today, (PeriodUnit::Day, "2026-11-11")),
            (Range::Yesterday, (PeriodUnit::Day, "2026-11-10")),
            (Range::ThisWeek, (PeriodUnit::Week, "2026-11-09")),
            (Range::LastWeek, (PeriodUnit::Week, "2026-11-02")),
            (Range::ThisMonth, (PeriodUnit::Month, "2026-11-01")),
            (Range::LastMonth, (PeriodUnit::Month, "2026-10-01")),
        ] {
            assert_eq!(
                highlight(&Shown::Report { range }, today).unwrap(),
                Some((expected.0, day(expected.1))),
                "{range:?}"
            );
        }
    }

    #[test]
    fn custom_report_ranges_are_highlighted_only_when_they_are_a_day_week_or_month() {
        let today = day("2026-11-11");
        for (from, to, expected) in [
            (
                "2026-11-04",
                "2026-11-04",
                Some((PeriodUnit::Day, "2026-11-04")),
            ),
            (
                "2026-11-09",
                "2026-11-15",
                Some((PeriodUnit::Week, "2026-11-09")),
            ),
            (
                "2026-11-01",
                "2026-11-30",
                Some((PeriodUnit::Month, "2026-11-01")),
            ),
            (
                "2028-02-01",
                "2028-02-29",
                Some((PeriodUnit::Month, "2028-02-01")),
            ),
            ("2026-11-09", "2026-11-11", None),
            ("2026-11-10", "2026-11-16", None),
            ("2026-11-01", "2026-11-29", None),
        ] {
            assert_eq!(
                highlight(&custom(from, to), today).unwrap(),
                expected.map(|(unit, first)| (unit, day(first))),
                "{from} - {to}"
            );
        }
    }

    #[test]
    fn a_view_without_dates_highlights_nothing() {
        assert_eq!(highlight(&Shown::None, day("2026-11-11")).unwrap(), None);
    }

    #[test]
    fn a_reversed_report_range_is_invalid_input() {
        let err = highlight(&custom("2026-11-15", "2026-11-09"), day("2026-11-11")).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }

    mod recorded {
        use super::*;

        const HOUR: Ms = 3_600_000;

        fn days(from: &str, to: &str) -> Vec<Day> {
            Day::range(&day(from), &day(to))
        }

        #[test]
        fn a_span_over_midnight_marks_both_days() {
            let midnight = day("2026-11-11").bounds().1;
            let spans = [(midnight - HOUR, Some(midnight + HOUR))];

            let marked = recorded_days(&spans, &days("2026-11-09", "2026-11-13"), midnight * 2);

            assert_eq!(
                marked.into_iter().collect::<Vec<_>>(),
                [day("2026-11-11"), day("2026-11-12")]
            );
        }

        #[test]
        fn a_span_ending_exactly_at_midnight_does_not_mark_the_next_day() {
            let midnight = day("2026-11-11").bounds().1;
            let spans = [(midnight - HOUR, Some(midnight))];

            let marked = recorded_days(&spans, &days("2026-11-09", "2026-11-13"), midnight * 2);

            assert_eq!(marked.into_iter().collect::<Vec<_>>(), [day("2026-11-11")]);
        }

        #[test]
        fn an_open_span_marks_the_days_up_to_now() {
            let start = day("2026-11-10").bounds().0 + 9 * HOUR;
            let now = day("2026-11-12").bounds().0 + HOUR;

            let marked = recorded_days(&[(start, None)], &days("2026-11-08", "2026-11-14"), now);

            assert_eq!(
                marked.into_iter().collect::<Vec<_>>(),
                days("2026-11-10", "2026-11-12")
            );
        }

        #[test]
        fn spans_outside_the_given_days_mark_nothing() {
            let before = day("2026-11-01").bounds().0 + HOUR;
            let spans = [(before, Some(before + HOUR))];

            let marked = recorded_days(&spans, &days("2026-11-09", "2026-11-13"), before * 2);

            assert!(marked.is_empty());
        }

        #[test]
        fn no_spans_mark_no_days() {
            assert!(recorded_days(&[], &days("2026-11-09", "2026-11-13"), 0).is_empty());
        }
    }

    mod stored {
        use super::*;
        use crate::test_support::{project, recording, temp_db};

        fn at(s: &str, hour: i64) -> Ms {
            day(s).bounds().0 + hour * 3_600_000
        }

        #[tokio::test]
        #[ignore = "needs a temp SQLite file"]
        async fn days_with_recordings_get_dots_and_spans_outside_the_six_weeks_are_excluded() {
            let (_dir, db) = temp_db().await;
            let p = project(&db, "Portal").await;
            recording(&db, &p.id, at("2026-11-10", 9), Some(at("2026-11-10", 10))).await;
            recording(&db, &p.id, at("2026-11-14", 23), Some(at("2026-11-15", 1))).await;
            recording(&db, &p.id, at("2026-10-25", 9), Some(at("2026-10-25", 10))).await;
            recording(&db, &p.id, at("2026-12-07", 9), Some(at("2026-12-07", 10))).await;
            recording(&db, &p.id, at("2026-10-25", 22), Some(at("2026-10-26", 0))).await;

            let month = load(Some(&db), "2026-11-15", Shown::None, CalendarTitle::Short)
                .await
                .unwrap();

            let dotted: Vec<Day> = all_days(&month)
                .into_iter()
                .filter(|d| d.has_recordings)
                .map(|d| d.day)
                .collect();
            assert_eq!(
                dotted,
                [day("2026-11-10"), day("2026-11-14"), day("2026-11-15")]
            );
        }

        #[tokio::test]
        #[ignore = "needs a temp SQLite file"]
        async fn dots_and_the_highlight_come_together() {
            let (_dir, db) = temp_db().await;
            let p = project(&db, "Portal").await;
            recording(&db, &p.id, at("2026-11-11", 9), Some(at("2026-11-11", 10))).await;

            let month = load(
                Some(&db),
                "2026-11-15",
                period(PeriodUnit::Day, "2026-11-11"),
                CalendarTitle::Full,
            )
            .await
            .unwrap();

            let shown = all_days(&month)
                .into_iter()
                .find(|d| d.day == day("2026-11-11"))
                .unwrap();
            assert_eq!((shown.has_recordings, shown.selected), (true, true));
            assert_eq!(month.month_name, "November");
        }
    }
}
