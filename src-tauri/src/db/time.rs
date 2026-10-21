//! Instants are UTC milliseconds; a day is a local calendar day (ADR 0011). Calendar math lives here
//! only, with the time zone as a parameter, so tests never depend on the machine's zone.

use std::fmt;

use chrono::{Datelike, Days, Local, LocalResult, NaiveDate, NaiveDateTime, TimeZone};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};

/// UTC milliseconds.
pub type Ms = i64;

pub fn now_ms() -> Ms {
    chrono::Utc::now().timestamp_millis()
}

/// A local calendar day, `YYYY-MM-DD` on the wire. Deserializing validates it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(try_from = "String", into = "String")]
#[ts(export, export_to = "src/api/types.gen.ts", type = "string")]
pub struct Day(NaiveDate);

impl Day {
    /// Exactly `YYYY-MM-DD` naming a real date.
    pub fn parse(s: &str) -> AppResult<Day> {
        let shaped = s.len() == 10
            && s.bytes().enumerate().all(|(i, b)| {
                if i == 4 || i == 7 {
                    b == b'-'
                } else {
                    b.is_ascii_digit()
                }
            });
        shaped
            .then(|| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .flatten()
            .map(Day)
            .ok_or_else(|| AppError::InvalidInput(format!("Not a day (YYYY-MM-DD): {s}")))
    }

    pub fn date(&self) -> NaiveDate {
        self.0
    }

    pub fn today() -> Day {
        Day::of(now_ms())
    }

    pub fn of(ms: Ms) -> Day {
        Day::of_in(ms, &Local)
    }

    pub fn of_in<Tz: TimeZone>(ms: Ms, tz: &Tz) -> Day {
        let utc = chrono::DateTime::from_timestamp_millis(ms).unwrap_or_default();
        Day(utc.with_timezone(tz).date_naive())
    }

    /// `[start, end)` in local time; see `bounds_in`.
    pub fn bounds(&self) -> (Ms, Ms) {
        self.bounds_in(&Local)
    }

    /// `[start, end)` from this day's midnight to the next one, so a DST day is 23 or 25 h.
    pub fn bounds_in<Tz: TimeZone>(&self, tz: &Tz) -> (Ms, Ms) {
        (midnight(self.0, tz), midnight(self.next().0, tz))
    }

    pub fn prev(&self) -> Day {
        Day(self.0.pred_opt().expect("a day after the minimum date"))
    }

    pub fn next(&self) -> Day {
        Day(self.0.succ_opt().expect("a day before the maximum date"))
    }

    /// Every day from `from` to `to`, both included; empty when `from` is after `to`.
    pub fn range(from: &Day, to: &Day) -> Vec<Day> {
        from.0
            .iter_days()
            .take_while(|d| d <= &to.0)
            .map(Day)
            .collect()
    }

    /// The Monday of this day's week.
    pub fn week_start(&self) -> Day {
        let back = u64::from(self.0.weekday().num_days_from_monday());
        Day(self.0 - Days::new(back))
    }

    pub fn month_start(&self) -> Day {
        Day(self.0.with_day(1).expect("every month has a first day"))
    }
}

/// The first instant of a local date. An ambiguous midnight takes the earlier instant; a skipped one
/// (a DST gap at midnight) takes the first valid local time after the gap.
fn midnight<Tz: TimeZone>(date: NaiveDate, tz: &Tz) -> Ms {
    let mut local: NaiveDateTime = date.and_hms_opt(0, 0, 0).expect("midnight exists");
    loop {
        match tz.from_local_datetime(&local) {
            LocalResult::Single(t) | LocalResult::Ambiguous(t, _) => return t.timestamp_millis(),
            LocalResult::None => local += chrono::TimeDelta::minutes(1),
        }
    }
}

impl TryFrom<String> for Day {
    type Error = AppError;
    fn try_from(s: String) -> AppResult<Day> {
        Day::parse(&s)
    }
}

impl From<Day> for String {
    fn from(day: Day) -> String {
        day.to_string()
    }
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The part of a span inside `bounds`; an open span ends at `now`.
pub fn clip(start: Ms, end: Option<Ms>, bounds: (Ms, Ms), now: Ms) -> Option<(Ms, Ms)> {
    let from = start.max(bounds.0);
    let to = end.unwrap_or(now).min(bounds.1);
    (from < to).then_some((from, to))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;
    use chrono_tz::{America::Santiago, Europe::Vienna, UTC};

    const HOUR: Ms = 3_600_000;

    fn day(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    fn utc_ms(y: i32, m: u32, d: u32, h: u32) -> Ms {
        chrono::Utc
            .with_ymd_and_hms(y, m, d, h, 0, 0)
            .unwrap()
            .timestamp_millis()
    }

    #[test]
    fn parse_accepts_a_real_day() {
        assert_eq!(day("2026-10-21").to_string(), "2026-10-21");
    }

    #[test]
    fn parse_rejects_malformed_and_impossible_days() {
        for input in [
            "2026-13-01",
            "2026-02-30",
            "26-1-1",
            "2026-1-01",
            " 2026-10-21",
            "",
        ] {
            let err = Day::parse(input).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "{input:?}");
        }
    }

    #[test]
    fn day_serializes_as_a_string_and_validates_on_deserialize() {
        assert_eq!(
            serde_json::to_value(day("2026-10-21")).unwrap(),
            serde_json::json!("2026-10-21")
        );
        assert_eq!(
            serde_json::from_str::<Day>("\"2026-10-21\"").unwrap(),
            day("2026-10-21")
        );
        assert!(serde_json::from_str::<Day>("\"2026-02-30\"").is_err());
    }

    #[test]
    fn bounds_in_utc_span_24_hours_from_utc_midnight() {
        let (start, end) = day("2026-10-21").bounds_in(&UTC);
        assert_eq!(start, 1_792_540_800_000);
        assert_eq!(end - start, 24 * HOUR);
    }

    #[test]
    fn bounds_in_vienna_follow_dst() {
        for (d, start, hours) in [
            ("2026-06-15", utc_ms(2026, 6, 14, 22), 24),
            ("2026-03-29", utc_ms(2026, 3, 28, 23), 23),
            ("2026-10-25", utc_ms(2026, 10, 24, 22), 25),
        ] {
            let (s, e) = day(d).bounds_in(&Vienna);
            assert_eq!((s, e - s), (start, hours * HOUR), "{d}");
        }
    }

    #[test]
    fn a_dst_gap_at_midnight_starts_the_day_at_the_first_valid_time() {
        let (start, end) = day("2026-09-06").bounds_in(&Santiago);
        assert_eq!(start, utc_ms(2026, 9, 6, 4));
        assert_eq!(end - start, 23 * HOUR);
    }

    #[test]
    fn of_in_uses_the_local_calendar_day() {
        let late = utc_ms(2026, 10, 21, 23) + 30 * 60_000;
        assert_eq!(Day::of_in(late, &UTC), day("2026-10-21"));
        assert_eq!(Day::of_in(late, &Vienna), day("2026-10-22"));
    }

    #[test]
    fn prev_and_next_cross_month_and_year_ends() {
        for (from, prev, next) in [
            ("2026-03-01", "2026-02-28", "2026-03-02"),
            ("2026-01-31", "2026-01-30", "2026-02-01"),
            ("2026-01-01", "2025-12-31", "2026-01-02"),
            ("2025-12-31", "2025-12-30", "2026-01-01"),
        ] {
            assert_eq!(day(from).prev(), day(prev), "{from}");
            assert_eq!(day(from).next(), day(next), "{from}");
        }
    }

    #[test]
    fn range_includes_both_ends() {
        assert_eq!(
            Day::range(&day("2026-12-30"), &day("2027-01-01")),
            vec![day("2026-12-30"), day("2026-12-31"), day("2027-01-01")]
        );
        assert_eq!(
            Day::range(&day("2026-10-21"), &day("2026-10-21")),
            vec![day("2026-10-21")]
        );
        assert!(Day::range(&day("2026-10-22"), &day("2026-10-21")).is_empty());
    }

    #[test]
    fn week_start_is_the_monday() {
        for (d, monday) in [
            ("2026-10-21", "2026-10-19"),
            ("2026-10-25", "2026-10-19"),
            ("2026-10-19", "2026-10-19"),
            ("2026-01-01", "2025-12-29"),
        ] {
            assert_eq!(day(d).week_start(), day(monday), "{d}");
        }
    }

    #[test]
    fn month_start_is_the_first_of_the_month() {
        for (d, first) in [("2026-10-21", "2026-10-01"), ("2026-02-01", "2026-02-01")] {
            assert_eq!(day(d).month_start(), day(first), "{d}");
        }
    }

    #[test]
    fn clip_keeps_the_part_inside_the_bounds() {
        let bounds = (100, 200);
        for (start, end, now, expected) in [
            (120, Some(180), 1000, Some((120, 180))),
            (50, Some(150), 1000, Some((100, 150))),
            (150, Some(250), 1000, Some((150, 200))),
            (50, Some(250), 1000, Some((100, 200))),
            (150, None, 180, Some((150, 180))),
            (150, None, 1000, Some((150, 200))),
            (150, None, 100, None),
            (300, None, 1000, None),
            (10, Some(50), 1000, None),
            (250, Some(300), 1000, None),
            (50, Some(100), 1000, None),
            (200, Some(300), 1000, None),
        ] {
            assert_eq!(
                clip(start, end, bounds, now),
                expected,
                "{start}..{end:?} now {now}"
            );
        }
    }
}
