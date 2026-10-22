//! Titles and date labels in the format the settings choose and the order the macOS locale reads dates in
//! (`DateOrder`): the Day view's day, the Week's range, the Reports title, the sidebar calendar's month.

use chrono::{DateTime, Datelike, Local, NaiveDate};

use super::locale::DateOrder;
use crate::db::time::{Day, Ms};
use crate::store::settings::{CalendarTitle, DayTitle};

/// `Wednesday, November 11, 2026`, or `Wed, Nov 11` (with the year when it is not `today`'s), in the
/// system's date order.
pub fn day_title(day: Day, format: DayTitle, today: Day) -> String {
    day_title_in(day, format, today, DateOrder::system())
}

/// `day_title` in `order`: `Wednesday, 11 November 2026` or `Wed, 11 Nov` day first.
pub fn day_title_in(day: Day, format: DayTitle, today: Day, order: DateOrder) -> String {
    let weekday = match format {
        DayTitle::Long => "%A",
        DayTitle::Short => "%a",
    };
    let year = format == DayTitle::Long || !same_year(day, today);
    format!(
        "{}, {}",
        day.date().format(weekday),
        date_in(day, format, year, order)
    )
}

/// `October 21, 2026` (`21 October 2026` day first): a date on its own.
pub fn date_label(day: Day) -> String {
    date_in(day, DayTitle::Long, true, DateOrder::system())
}

/// `Oct 21`, `Oct 21, 2026` or `October 21, 2026`, the day and the month in `order`.
fn date_in(day: Day, format: DayTitle, year: bool, order: DateOrder) -> String {
    let date = day.date();
    let month = month_of(date, format);
    match (order, year) {
        (DateOrder::MonthFirst, false) => format!("{month} {}", date.day()),
        (DateOrder::MonthFirst, true) => format!("{month} {}, {}", date.day(), date.year()),
        (DateOrder::DayFirst, false) => format!("{} {month}", date.day()),
        (DateOrder::DayFirst, true) => format!("{} {month} {}", date.day(), date.year()),
    }
}

fn month_of(date: NaiveDate, format: DayTitle) -> String {
    let pattern = match format {
        DayTitle::Long => "%B",
        DayTitle::Short => "%b",
    };
    date.format(pattern).to_string()
}

fn same_year(day: Day, today: Day) -> bool {
    day.date().year() == today.date().year()
}

/// `Oct 19 – 25, 2026`, `Oct 26 – Nov 1, 2026`, `Dec 29, 2025 – Jan 4, 2026` (`19 – 25 Oct 2026` day
/// first): the month and the year once where both days share them. `year` false drops a shared year.
pub fn range_label(from: Day, to: Day, format: DayTitle, year: bool) -> String {
    range_in(from, to, format, year, DateOrder::system())
}

/// `range_label` in `order`.
pub fn range_in(from: Day, to: Day, format: DayTitle, year: bool, order: DateOrder) -> String {
    let (first, last) = (from.date(), to.date());
    if first.year() != last.year() {
        return format!(
            "{} \u{2013} {}",
            date_in(from, format, true, order),
            date_in(to, format, true, order)
        );
    }
    let same_month = first.month() == last.month();
    let start = match (order, same_month) {
        (DateOrder::DayFirst, true) => first.day().to_string(),
        _ => date_in(from, format, false, order),
    };
    let end = match (order, same_month) {
        (DateOrder::MonthFirst, true) if year => format!("{}, {}", last.day(), last.year()),
        (DateOrder::MonthFirst, true) => last.day().to_string(),
        _ => date_in(to, format, year, order),
    };
    format!("{start} \u{2013} {end}")
}

/// `range_label` with the year where a title needs it: always in the long format, in the short one only when
/// a day lies outside `today`'s year (`Oct 19 – 25`, `Dec 29, 2025 – Jan 4, 2026`).
pub fn range_title(from: Day, to: Day, format: DayTitle, today: Day) -> String {
    let year = format == DayTitle::Long || !same_year(from, today) || !same_year(to, today);
    range_label(from, to, format, year)
}

/// `Wed, Oct 21 09:30 – Thu, Oct 22 10:00`: two local moments with their short day titles.
pub fn moments_label(from: Ms, to: Ms, today: Day) -> String {
    let moment = |ms: Ms| {
        let clock = DateTime::from_timestamp_millis(ms)
            .unwrap_or_default()
            .with_timezone(&Local)
            .format("%H:%M");
        format!("{} {clock}", day_title(Day::of(ms), DayTitle::Short, today))
    };
    format!("{} \u{2013} {}", moment(from), moment(to))
}

/// `October 2026` or `Oct 2026`, whatever the order.
pub fn month_title(month: Day, format: DayTitle) -> String {
    format!("{} {}", month_of(month.date(), format), month.date().year())
}

/// The Reports title for the days `from..=to` in the Day title's format: a day reads as the Day view's
/// title, a calendar month as its name and year, any other range as `range_title`.
pub fn period_title(from: Day, to: Day, format: DayTitle, today: Day) -> String {
    if from == to {
        return day_title(from, format, today);
    }
    if from == from.month_start() && to.month_start() == from && to.next().month_start() != from {
        return month_title(from, format);
    }
    range_title(from, to, format, today)
}

/// `Nov` or `November`; the calendar shows the year beside it.
pub fn month_name(month: Day, format: CalendarTitle) -> String {
    let pattern = match format {
        CalendarTitle::Short => "%b",
        CalendarTitle::Full => "%B",
    };
    month.date().format(pattern).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    #[test]
    fn a_long_day_title_names_the_weekday_month_and_year() {
        for (given, expected) in [
            ("2026-11-11", "Wednesday, November 11, 2026"),
            ("2026-01-05", "Monday, January 5, 2026"),
        ] {
            assert_eq!(
                day_title(day(given), DayTitle::Long, day("2026-11-11")),
                expected,
                "{given}"
            );
        }
    }

    #[test]
    fn a_short_day_title_drops_the_year_only_in_the_current_year() {
        for (given, today, expected) in [
            ("2026-11-11", "2026-03-02", "Wed, Nov 11"),
            ("2025-11-12", "2026-03-02", "Wed, Nov 12, 2025"),
            ("2027-01-05", "2026-12-31", "Tue, Jan 5, 2027"),
        ] {
            assert_eq!(
                day_title(day(given), DayTitle::Short, day(today)),
                expected,
                "{given} on {today}"
            );
        }
    }

    #[test]
    fn a_day_title_follows_the_date_order() {
        for (given, format, today, order, expected) in [
            (
                "2026-11-11",
                DayTitle::Long,
                "2026-11-11",
                DateOrder::MonthFirst,
                "Wednesday, November 11, 2026",
            ),
            (
                "2026-11-11",
                DayTitle::Long,
                "2026-11-11",
                DateOrder::DayFirst,
                "Wednesday, 11 November 2026",
            ),
            (
                "2026-11-11",
                DayTitle::Short,
                "2026-03-02",
                DateOrder::MonthFirst,
                "Wed, Nov 11",
            ),
            (
                "2026-11-11",
                DayTitle::Short,
                "2026-03-02",
                DateOrder::DayFirst,
                "Wed, 11 Nov",
            ),
            (
                "2025-11-12",
                DayTitle::Short,
                "2026-03-02",
                DateOrder::MonthFirst,
                "Wed, Nov 12, 2025",
            ),
            (
                "2025-11-12",
                DayTitle::Short,
                "2026-03-02",
                DateOrder::DayFirst,
                "Wed, 12 Nov 2025",
            ),
        ] {
            assert_eq!(
                day_title_in(day(given), format, day(today), order),
                expected,
                "{given} {format:?} {order:?}"
            );
        }
    }

    #[test]
    fn a_range_shares_the_month_and_year_where_both_days_do() {
        let (first, last) = (DateOrder::MonthFirst, DateOrder::DayFirst);
        for (from, to, format, year, order, expected) in [
            (
                "2026-10-19",
                "2026-10-25",
                DayTitle::Short,
                true,
                first,
                "Oct 19 – 25, 2026",
            ),
            (
                "2026-10-19",
                "2026-10-25",
                DayTitle::Short,
                false,
                first,
                "Oct 19 – 25",
            ),
            (
                "2026-10-19",
                "2026-10-25",
                DayTitle::Long,
                true,
                first,
                "October 19 – 25, 2026",
            ),
            (
                "2026-10-19",
                "2026-10-25",
                DayTitle::Short,
                true,
                last,
                "19 – 25 Oct 2026",
            ),
            (
                "2026-10-19",
                "2026-10-25",
                DayTitle::Short,
                false,
                last,
                "19 – 25 Oct",
            ),
            (
                "2026-10-26",
                "2026-11-01",
                DayTitle::Short,
                true,
                first,
                "Oct 26 – Nov 1, 2026",
            ),
            (
                "2026-10-26",
                "2026-11-01",
                DayTitle::Short,
                false,
                first,
                "Oct 26 – Nov 1",
            ),
            (
                "2026-10-26",
                "2026-11-01",
                DayTitle::Short,
                true,
                last,
                "26 Oct – 1 Nov 2026",
            ),
            (
                "2026-10-26",
                "2026-11-01",
                DayTitle::Short,
                false,
                last,
                "26 Oct – 1 Nov",
            ),
            (
                "2025-12-29",
                "2026-01-04",
                DayTitle::Short,
                false,
                first,
                "Dec 29, 2025 – Jan 4, 2026",
            ),
            (
                "2025-12-29",
                "2026-01-04",
                DayTitle::Short,
                true,
                last,
                "29 Dec 2025 – 4 Jan 2026",
            ),
        ] {
            assert_eq!(
                range_in(day(from), day(to), format, year, order),
                expected,
                "{from}..{to} {format:?} year {year} {order:?}"
            );
        }
    }

    #[test]
    fn a_period_title_reads_as_a_day_a_month_or_a_range() {
        for (from, to, format, today, expected) in [
            (
                "2026-11-11",
                "2026-11-11",
                DayTitle::Long,
                "2026-11-11",
                "Wednesday, November 11, 2026",
            ),
            (
                "2026-11-11",
                "2026-11-11",
                DayTitle::Short,
                "2026-03-02",
                "Wed, Nov 11",
            ),
            (
                "2026-10-01",
                "2026-10-31",
                DayTitle::Long,
                "2026-10-22",
                "October 2026",
            ),
            (
                "2026-10-01",
                "2026-10-31",
                DayTitle::Short,
                "2026-10-22",
                "Oct 2026",
            ),
            (
                "2026-10-01",
                "2026-10-14",
                DayTitle::Long,
                "2026-10-22",
                "October 1 – 14, 2026",
            ),
            (
                "2026-10-01",
                "2026-10-14",
                DayTitle::Short,
                "2026-10-22",
                "Oct 1 – 14",
            ),
            (
                "2026-10-01",
                "2026-10-14",
                DayTitle::Short,
                "2027-01-03",
                "Oct 1 – 14, 2026",
            ),
            (
                "2026-10-01",
                "2026-11-30",
                DayTitle::Short,
                "2026-12-01",
                "Oct 1 – Nov 30",
            ),
        ] {
            assert_eq!(
                period_title(day(from), day(to), format, day(today)),
                expected,
                "{from}..{to} {format:?} on {today}"
            );
        }
    }

    #[test]
    fn a_month_title_names_the_month_and_year() {
        assert_eq!(
            month_title(day("2026-10-01"), DayTitle::Long),
            "October 2026"
        );
        assert_eq!(month_title(day("2026-10-01"), DayTitle::Short), "Oct 2026");
    }

    #[test]
    fn a_date_label_spells_out_the_month_and_year() {
        assert_eq!(date_label(day("2026-10-21")), "October 21, 2026");
    }

    #[test]
    fn a_moments_label_joins_two_short_day_titles_with_their_clock_times() {
        let label = moments_label(1_792_573_800_000, 1_792_664_400_000, day("2026-10-22"));
        let parts: Vec<&str> = label.split(" \u{2013} ").collect();
        assert_eq!(parts.len(), 2, "{label}");
        for part in parts {
            let (title, clock) = part.rsplit_once(' ').unwrap();
            assert!(title.contains(", "), "{label}");
            let bytes = clock.as_bytes();
            assert!(
                clock.len() == 5
                    && bytes[2] == b':'
                    && clock.replace(':', "").bytes().all(|b| b.is_ascii_digit()),
                "{label}"
            );
        }
    }

    #[test]
    fn a_month_name_is_abbreviated_or_full() {
        assert_eq!(month_name(day("2026-11-01"), CalendarTitle::Short), "Nov");
        assert_eq!(
            month_name(day("2026-11-01"), CalendarTitle::Full),
            "November"
        );
        assert_eq!(month_name(day("2026-05-01"), CalendarTitle::Short), "May");
    }
}
