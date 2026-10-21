//! The palette's dates: date words and typed dates as the day, week or month they open.

use chrono::{Datelike, Days, Months, NaiveDate, Weekday};
use serde::Serialize;
use ts_rs::TS;

use super::period::{PeriodUnit, period};
use crate::db::time::Day;

/// A year without a February 29 is skipped; eight years back always reach a leap year.
const YEARS_BACK: i32 = 8;
/// A month name needs this many letters, so `ma` is neither March nor May.
const MONTH_PREFIX: usize = 3;

/// The date words an empty text offers.
const QUICK: [&str; 4] = ["Today", "Yesterday", "This week", "This month"];

const WEEKDAYS: [(&str, Weekday); 7] = [
    ("Monday", Weekday::Mon),
    ("Tuesday", Weekday::Tue),
    ("Wednesday", Weekday::Wed),
    ("Thursday", Weekday::Thu),
    ("Friday", Weekday::Fri),
    ("Saturday", Weekday::Sat),
    ("Sunday", Weekday::Sun),
];

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct DateJump {
    /// The date word (`Yesterday`, `Last week`, `Monday`, `October`); none for a typed date.
    pub name: Option<String>,
    pub unit: PeriodUnit,
    pub from: Day,
    pub to: Day,
}

fn jump(name: Option<&str>, unit: PeriodUnit, day: Day, today: Day) -> DateJump {
    let p = period(unit, day, today);
    DateJump {
        name: name.map(str::to_string),
        unit,
        from: p.from,
        to: p.to,
    }
}

/// Without a typed date, the date words that start with `text` (or one of whose words does), in a fixed
/// order; an empty `text` gives Today, Yesterday, This week and This month. Weekdays and months are the
/// latest such day or month on or before today.
pub fn jumps(text: &str, today: Day) -> Vec<DateJump> {
    let text = text.trim().to_lowercase();
    if let Some(day) = typed_date(&text, today) {
        return vec![jump(None, PeriodUnit::Day, day, today)];
    }
    let matches = |name: &str| {
        if text.is_empty() {
            QUICK.contains(&name)
        } else {
            starts_word(name, &text)
        }
    };
    words(today)
        .into_iter()
        .filter(|(name, ..)| matches(name))
        .map(|(name, unit, day)| jump(Some(name), unit, day, today))
        .collect()
}

fn starts_word(name: &str, text: &str) -> bool {
    let name = name.to_lowercase();
    name.starts_with(text) || name.split(' ').any(|word| word.starts_with(text))
}

fn words(today: Day) -> Vec<(&'static str, PeriodUnit, Day)> {
    let month_start = today.month_start();
    let mut list = vec![
        ("Today", PeriodUnit::Day, today),
        ("Yesterday", PeriodUnit::Day, today.prev()),
        ("This week", PeriodUnit::Week, today),
        ("Last week", PeriodUnit::Week, today.week_start().prev()),
        ("This month", PeriodUnit::Month, today),
        ("Last month", PeriodUnit::Month, month_start.prev()),
    ];
    for (name, weekday) in WEEKDAYS {
        let back = (7 + today.date().weekday().num_days_from_monday()
            - weekday.num_days_from_monday())
            % 7;
        let day = Day::from_date(today.date() - Days::new(u64::from(back)));
        list.push((name, PeriodUnit::Day, day));
    }
    for (i, name) in MONTHS.iter().enumerate() {
        let back = (12 + month_start.date().month0() - i as u32) % 12;
        list.push((
            name,
            PeriodUnit::Month,
            Day::from_date(month_start.date() - Months::new(back)),
        ));
    }
    list
}

/// `2026-10-12`, `12.10.`, `12.10.2026`, `oct 12`, `12 october 2026`; without a year the latest such date
/// on or before today.
fn typed_date(text: &str, today: Day) -> Option<Day> {
    if let Ok(day) = Day::parse(text) {
        return Some(day);
    }
    let (day, month, year) = dotted(text).or_else(|| named(text))?;
    match year {
        Some(year) => NaiveDate::from_ymd_opt(year, month, day).map(Day::from_date),
        None => latest(month, day, today),
    }
}

type Parts = (u32, u32, Option<i32>);

fn number<T: std::str::FromStr>(text: &str, digits: std::ops::RangeInclusive<usize>) -> Option<T> {
    (digits.contains(&text.len()) && text.bytes().all(|b| b.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}

/// `12.10`, `12.10.` or `12.10.2026`.
fn dotted(text: &str) -> Option<Parts> {
    let parts: Vec<&str> = text.split('.').collect();
    let (day, month, year) = match parts.as_slice() {
        [d, m] | [d, m, ""] => (d, m, None),
        [d, m, y] => (d, m, Some(number(y, 4..=4)?)),
        _ => return None,
    };
    Some((number(day, 1..=2)?, number(month, 1..=2)?, year))
}

/// A day number and a month name in either order (`oct 12`, `12. october`), then an optional year.
fn named(text: &str) -> Option<Parts> {
    let tokens: Vec<&str> = text
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|t| !t.is_empty())
        .collect();
    let (first, second, year) = match tokens.as_slice() {
        [a, b] => (a, b, None),
        [a, b, y] => (a, b, Some(number(y, 4..=4)?)),
        _ => return None,
    };
    let day_number = |t: &str| number::<u32>(t.strip_suffix('.').unwrap_or(t), 1..=2);
    let (day, month) = match (day_number(first), day_number(second)) {
        (Some(d), None) => (d, month_named(second)?),
        (None, Some(d)) => (d, month_named(first)?),
        _ => return None,
    };
    Some((day, month, year))
}

fn month_named(text: &str) -> Option<u32> {
    if text.len() < MONTH_PREFIX {
        return None;
    }
    let found = MONTHS
        .iter()
        .position(|name| name.to_lowercase().starts_with(text))?;
    Some(found as u32 + 1)
}

fn latest(month: u32, day: u32, today: Day) -> Option<Day> {
    let year = today.date().year();
    (0..=YEARS_BACK)
        .filter_map(|back| NaiveDate::from_ymd_opt(year - back, month, day))
        .find(|date| *date <= today.date())
        .map(Day::from_date)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(text: &str) -> Day {
        Day::parse(text).unwrap()
    }

    fn today() -> Day {
        day("2026-10-21")
    }

    fn word(name: &str, unit: PeriodUnit, from: &str, to: &str) -> DateJump {
        DateJump {
            name: Some(name.to_string()),
            unit,
            from: day(from),
            to: day(to),
        }
    }

    fn typed(date: &str) -> DateJump {
        DateJump {
            name: None,
            unit: PeriodUnit::Day,
            from: day(date),
            to: day(date),
        }
    }

    fn names(text: &str) -> Vec<String> {
        jumps(text, today())
            .into_iter()
            .map(|j| j.name.unwrap())
            .collect()
    }

    #[test]
    fn empty_text_offers_today_yesterday_this_week_and_this_month() {
        assert_eq!(
            jumps("", today()),
            vec![
                word("Today", PeriodUnit::Day, "2026-10-21", "2026-10-21"),
                word("Yesterday", PeriodUnit::Day, "2026-10-20", "2026-10-20"),
                word("This week", PeriodUnit::Week, "2026-10-19", "2026-10-25"),
                word("This month", PeriodUnit::Month, "2026-10-01", "2026-10-31"),
            ]
        );
    }

    #[test]
    fn a_word_prefix_finds_the_date_words_starting_with_it() {
        assert_eq!(
            jumps("yes", today()),
            vec![word(
                "Yesterday",
                PeriodUnit::Day,
                "2026-10-20",
                "2026-10-20"
            )]
        );
        assert_eq!(
            names("t"),
            ["Today", "This week", "This month", "Tuesday", "Thursday"]
        );
    }

    #[test]
    fn a_later_word_of_a_name_matches_too() {
        assert_eq!(
            jumps("week", today()),
            vec![
                word("This week", PeriodUnit::Week, "2026-10-19", "2026-10-25"),
                word("Last week", PeriodUnit::Week, "2026-10-12", "2026-10-18"),
            ]
        );
        assert_eq!(
            jumps("mo", today()),
            vec![
                word("This month", PeriodUnit::Month, "2026-10-01", "2026-10-31"),
                word("Last month", PeriodUnit::Month, "2026-09-01", "2026-09-30"),
                word("Monday", PeriodUnit::Day, "2026-10-19", "2026-10-19"),
            ]
        );
    }

    #[test]
    fn a_weekday_is_its_latest_occurrence_on_or_before_today() {
        for (text, name, date) in [
            ("monday", "Monday", "2026-10-19"),
            ("wed", "Wednesday", "2026-10-21"),
            ("thursday", "Thursday", "2026-10-15"),
        ] {
            assert_eq!(
                jumps(text, today()),
                vec![word(name, PeriodUnit::Day, date, date)],
                "{text}"
            );
        }
    }

    #[test]
    fn a_month_name_is_its_latest_month_on_or_before_this_one() {
        for (text, name, from, to) in [
            ("oct", "October", "2026-10-01", "2026-10-31"),
            ("nov", "November", "2025-11-01", "2025-11-30"),
            ("january", "January", "2026-01-01", "2026-01-31"),
        ] {
            assert_eq!(
                jumps(text, today()),
                vec![word(name, PeriodUnit::Month, from, to)],
                "{text}"
            );
        }
    }

    #[test]
    fn typed_dates_without_a_year_open_the_day_in_the_current_year() {
        for text in [
            "2026-10-12",
            "12.10.",
            "12.10",
            "oct 12",
            "12 oct",
            "October 12",
            "12. october",
        ] {
            assert_eq!(jumps(text, today()), vec![typed("2026-10-12")], "{text}");
        }
    }

    #[test]
    fn typed_dates_with_a_year_open_that_year() {
        for text in ["12.10.2025", "12 oct 2025"] {
            assert_eq!(jumps(text, today()), vec![typed("2025-10-12")], "{text}");
        }
    }

    #[test]
    fn a_typed_date_after_today_falls_back_to_the_previous_year() {
        assert_eq!(jumps("24.12.", today()), vec![typed("2025-12-24")]);
    }

    #[test]
    fn february_29_falls_back_to_the_latest_leap_year() {
        assert_eq!(jumps("29.2.", today()), vec![typed("2024-02-29")]);
    }

    #[test]
    fn text_that_is_no_date_or_word_finds_nothing() {
        for text in ["31.02.", "12", "xyz", "ma 12"] {
            assert_eq!(jumps(text, today()), Vec::<DateJump>::new(), "{text}");
        }
    }

    #[test]
    fn case_and_surrounding_whitespace_are_ignored() {
        assert_eq!(jumps("  TODAY ", today()), jumps("today", today()));
        assert_eq!(
            jumps("  TODAY ", today()),
            vec![word("Today", PeriodUnit::Day, "2026-10-21", "2026-10-21")]
        );
    }

    #[test]
    fn a_jump_serializes_in_camel_case_with_a_null_name_for_a_typed_date() {
        assert_eq!(
            serde_json::to_value(typed("2026-10-12")).unwrap(),
            serde_json::json!({
                "name": null,
                "unit": "day",
                "from": "2026-10-12",
                "to": "2026-10-12",
            })
        );
        assert_eq!(
            serde_json::to_value(word(
                "This week",
                PeriodUnit::Week,
                "2026-10-19",
                "2026-10-25"
            ))
            .unwrap(),
            serde_json::json!({
                "name": "This week",
                "unit": "week",
                "from": "2026-10-19",
                "to": "2026-10-25",
            })
        );
    }
}
