//! Titles in the format the settings choose: the Day view's day and the sidebar calendar's month.

use chrono::Datelike;

use crate::db::time::Day;
use crate::store::settings::{CalendarTitle, DayTitle};

/// `Wednesday, November 11, 2026`, or `Wed, Nov 11` (with the year when it is not `today`'s).
pub fn day_title(day: Day, format: DayTitle, today: Day) -> String {
    let pattern = match format {
        DayTitle::Long => "%A, %B %-d, %Y",
        DayTitle::Short if day.date().year() == today.date().year() => "%a, %b %-d",
        DayTitle::Short => "%a, %b %-d, %Y",
    };
    day.date().format(pattern).to_string()
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
    fn a_month_name_is_abbreviated_or_full() {
        assert_eq!(month_name(day("2026-11-01"), CalendarTitle::Short), "Nov");
        assert_eq!(
            month_name(day("2026-11-01"), CalendarTitle::Full),
            "November"
        );
        assert_eq!(month_name(day("2026-05-01"), CalendarTitle::Short), "May");
    }
}
