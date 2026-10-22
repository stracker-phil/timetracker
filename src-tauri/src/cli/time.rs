//! The time words of the CLI and of deep links, read in the zone `tz` at `now`.

use chrono::{DateTime, FixedOffset, NaiveDateTime, NaiveTime, TimeZone};

use crate::db::recordings::Span;
use crate::db::time::{Day, Ms};
use crate::tracker::PauseUntil;

const MAX_PAUSE_MINUTES: u32 = 24 * 60;

/// `now`, a time of today (`09:30`, `09:30:15`), or a date and time (`2026-10-21T09:30[:15]`) with an
/// optional `Z` or `±hh:mm` offset. Without an offset the time is local.
pub fn instant<Tz: TimeZone>(text: &str, now: Ms, tz: &Tz) -> Result<Ms, String> {
    let text = text.trim();
    if text == "now" {
        return Ok(now);
    }
    if let Some(time) = with_offset(text) {
        return Ok(time.timestamp_millis());
    }
    let local = local(text, now, tz)
        .ok_or_else(|| format!("Not a time: `{text}` (use 2026-10-21T09:30, 09:30 or now)"))?;
    tz.from_local_datetime(&local)
        .earliest()
        .map(|t| t.timestamp_millis())
        .ok_or_else(|| format!("`{text}` does not exist in the local time zone"))
}

/// Two `instant`s joined by `-` or `–`: `14:25-15:00`, `2026-10-21T22:00-2026-10-22T01:00`.
pub fn span<Tz: TimeZone>(text: &str, now: Ms, tz: &Tz) -> Result<Span, String> {
    text.char_indices()
        .filter(|&(_, c)| c == '-' || c == '–')
        .find_map(|(at, c)| {
            let start = instant(&text[..at], now, tz).ok()?;
            let end = instant(&text[at + c.len_utf8()..], now, tz).ok()?;
            Some(Span { start, end })
        })
        .ok_or_else(|| format!("Not a span: `{text}` (use 14:25-15:00)"))
}

fn with_offset(text: &str) -> Option<DateTime<FixedOffset>> {
    let text = match text.strip_suffix('Z') {
        Some(utc) => format!("{utc}+00:00"),
        None => text.to_string(),
    };
    ["%Y-%m-%dT%H:%M:%S%:z", "%Y-%m-%dT%H:%M%:z"]
        .iter()
        .find_map(|format| DateTime::parse_from_str(&text, format).ok())
}

/// A date and time, or a time of `now`'s day in `tz`.
fn local<Tz: TimeZone>(text: &str, now: Ms, tz: &Tz) -> Option<NaiveDateTime> {
    if text.contains('T') {
        return ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"]
            .iter()
            .find_map(|format| NaiveDateTime::parse_from_str(text, format).ok());
    }
    ["%H:%M:%S", "%H:%M"]
        .iter()
        .find_map(|format| NaiveTime::parse_from_str(text, format).ok())
        .map(|time| Day::of_in(now, tz).date().and_time(time))
}

/// `today`, `yesterday` or `YYYY-MM-DD`.
pub fn day<Tz: TimeZone>(text: &str, now: Ms, tz: &Tz) -> Result<Day, String> {
    match text.trim() {
        "today" => Ok(Day::of_in(now, tz)),
        "yesterday" => Ok(Day::of_in(now, tz).prev()),
        other => Day::parse(other)
            .map_err(|_| format!("Not a day: `{other}` (use YYYY-MM-DD, today or yesterday)")),
    }
}

/// `<n>m` or `<n>h` (up to 24 h), `tomorrow` (the next local midnight) or `off` (until resumed).
pub fn pause(text: &str) -> Result<PauseUntil, String> {
    let text = text.trim();
    match text {
        "off" => Ok(PauseUntil::Resumed),
        "tomorrow" => Ok(PauseUntil::Tomorrow),
        _ => minutes(text)
            .map(PauseUntil::Minutes)
            .ok_or_else(|| format!("Pause for 15m, 1h, tomorrow or off, not `{text}`")),
    }
}

fn minutes(text: &str) -> Option<u32> {
    let (number, per_unit) = match text.strip_suffix('m') {
        Some(number) => (number, 1),
        None => (text.strip_suffix('h')?, 60),
    };
    number
        .parse::<u32>()
        .ok()?
        .checked_mul(per_unit)
        .filter(|m| (1..=MAX_PAUSE_MINUTES).contains(m))
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use chrono_tz::Europe::Vienna;

    use super::*;

    fn ms(rfc3339: &str) -> Ms {
        DateTime::parse_from_rfc3339(rfc3339)
            .unwrap()
            .timestamp_millis()
    }

    /// 23:30 UTC on the 20th is already 01:30 on the 21st in Vienna.
    fn near_midnight() -> Ms {
        ms("2026-10-20T23:30:00Z")
    }

    #[test]
    fn date_and_time_without_offset_is_local() {
        for (text, expected) in [
            ("2026-10-21T09:30", "2026-10-21T07:30:00Z"),
            ("2026-10-21T09:30:15", "2026-10-21T07:30:15Z"),
            (" 2026-10-21T09:30 ", "2026-10-21T07:30:00Z"),
            ("2026-01-15T09:30", "2026-01-15T08:30:00Z"),
        ] {
            assert_eq!(
                instant(text, near_midnight(), &Vienna),
                Ok(ms(expected)),
                "{text}"
            );
        }
    }

    #[test]
    fn explicit_offsets_override_the_zone() {
        for (text, expected) in [
            ("2026-10-21T09:30Z", "2026-10-21T09:30:00Z"),
            ("2026-10-21T09:30:15Z", "2026-10-21T09:30:15Z"),
            ("2026-10-21T09:30+05:30", "2026-10-21T04:00:00Z"),
            ("2026-10-21T09:30-03:00", "2026-10-21T12:30:00Z"),
            ("2026-10-21T09:30:15+02:00", "2026-10-21T07:30:15Z"),
        ] {
            assert_eq!(
                instant(text, near_midnight(), &Vienna),
                Ok(ms(expected)),
                "{text}"
            );
        }
    }

    #[test]
    fn a_bare_time_is_today_in_the_zone() {
        for (text, expected) in [
            ("09:30", "2026-10-21T07:30:00Z"),
            ("09:30:15", "2026-10-21T07:30:15Z"),
        ] {
            assert_eq!(
                instant(text, near_midnight(), &Vienna),
                Ok(ms(expected)),
                "{text}"
            );
        }
    }

    #[test]
    fn now_is_the_given_instant() {
        assert_eq!(
            instant("now", near_midnight(), &Vienna),
            Ok(near_midnight())
        );
        assert_eq!(
            instant(" now ", near_midnight(), &Vienna),
            Ok(near_midnight())
        );
    }

    #[test]
    fn an_ambiguous_local_time_takes_the_earlier_instant() {
        assert_eq!(
            instant("2026-10-25T02:30", near_midnight(), &Vienna),
            Ok(ms("2026-10-25T00:30:00Z"))
        );
    }

    #[test]
    fn text_that_is_not_a_time_is_an_error() {
        for text in [
            "",
            "garbage",
            "25:00",
            "09:60",
            "2026-13-01T10:00",
            "2026-10-32T10:00",
            "2026-10-21 09:30",
            "2026-10-21",
            "2026-10-21T09",
        ] {
            let err = instant(text, near_midnight(), &Vienna).unwrap_err();
            assert_eq!(
                err,
                format!("Not a time: `{text}` (use 2026-10-21T09:30, 09:30 or now)"),
                "{text}"
            );
        }
    }

    #[test]
    fn a_time_in_the_daylight_saving_gap_is_an_error() {
        let err = instant("2026-03-29T02:30", near_midnight(), &Vienna).unwrap_err();
        assert_eq!(
            err,
            "`2026-03-29T02:30` does not exist in the local time zone"
        );
    }

    #[test]
    fn a_span_is_two_times_joined_by_a_hyphen_or_an_en_dash() {
        let expected = Span {
            start: ms("2026-10-21T12:25:00Z"),
            end: ms("2026-10-21T13:00:00Z"),
        };
        for text in [
            "14:25-15:00",
            "14:25–15:00",
            " 14:25 - 15:00 ",
            "14:25 – 15:00",
        ] {
            assert_eq!(span(text, near_midnight(), &Vienna), Ok(expected), "{text}");
        }
    }

    #[test]
    fn a_span_may_use_dates_offsets_seconds_and_now() {
        for (text, start, end) in [
            (
                "2026-10-21T22:00-2026-10-22T01:00",
                "2026-10-21T20:00:00Z",
                "2026-10-21T23:00:00Z",
            ),
            (
                "2026-10-21T09:30Z-2026-10-21T10:30Z",
                "2026-10-21T09:30:00Z",
                "2026-10-21T10:30:00Z",
            ),
            (
                "2026-10-21T09:30+02:00-2026-10-21T10:30:15+02:00",
                "2026-10-21T07:30:00Z",
                "2026-10-21T08:30:15Z",
            ),
            (
                "2026-10-21T09:30-03:00–2026-10-21T18:00",
                "2026-10-21T12:30:00Z",
                "2026-10-21T16:00:00Z",
            ),
            (
                "14:25:30-2026-10-22T01:00",
                "2026-10-21T12:25:30Z",
                "2026-10-21T23:00:00Z",
            ),
            ("14:25-now", "2026-10-21T12:25:00Z", "2026-10-20T23:30:00Z"),
        ] {
            assert_eq!(
                span(text, near_midnight(), &Vienna),
                Ok(Span {
                    start: ms(start),
                    end: ms(end)
                }),
                "{text}"
            );
        }
    }

    #[test]
    fn text_that_is_not_a_span_is_an_error() {
        for text in [
            "",
            "14:25",
            "garbage",
            "14:25-",
            "-15:00",
            "14:25-garbage",
            "14:25-25:00",
        ] {
            assert_eq!(
                span(text, near_midnight(), &Vienna),
                Err(format!("Not a span: `{text}` (use 14:25-15:00)")),
                "{text}"
            );
        }
    }

    #[test]
    fn today_and_yesterday_follow_the_zone() {
        let day_of = |text: &str| Day::parse(text).unwrap();
        assert_eq!(
            day("today", near_midnight(), &Vienna),
            Ok(day_of("2026-10-21"))
        );
        assert_eq!(
            day("yesterday", near_midnight(), &Vienna),
            Ok(day_of("2026-10-20"))
        );
        assert_eq!(
            day(" today ", near_midnight(), &chrono::Utc),
            Ok(day_of("2026-10-20"))
        );
    }

    #[test]
    fn a_date_names_that_day() {
        assert_eq!(
            day("2026-02-28", near_midnight(), &Vienna),
            Ok(Day::parse("2026-02-28").unwrap())
        );
    }

    #[test]
    fn an_invalid_day_names_the_accepted_forms() {
        for text in ["", "tomorrow", "2026-13-01", "2026-02-30", "21.10.2026"] {
            assert_eq!(
                day(text, near_midnight(), &Vienna),
                Err(format!(
                    "Not a day: `{text}` (use YYYY-MM-DD, today or yesterday)"
                )),
                "{text}"
            );
        }
    }

    #[test]
    fn pause_reads_minutes_hours_and_the_words() {
        for (text, expected) in [
            ("1m", PauseUntil::Minutes(1)),
            ("15m", PauseUntil::Minutes(15)),
            ("90m", PauseUntil::Minutes(90)),
            ("1h", PauseUntil::Minutes(60)),
            ("24h", PauseUntil::Minutes(1440)),
            ("1440m", PauseUntil::Minutes(1440)),
            (" 15m ", PauseUntil::Minutes(15)),
            ("tomorrow", PauseUntil::Tomorrow),
            ("off", PauseUntil::Resumed),
        ] {
            assert_eq!(pause(text), Ok(expected), "{text}");
        }
    }

    #[test]
    fn pause_rejects_everything_else() {
        for text in [
            "0m",
            "0h",
            "25h",
            "1441m",
            "soon",
            "1d",
            "",
            "m",
            "h",
            "-5m",
            "1.5h",
            "15",
            "99999999999h",
        ] {
            assert_eq!(
                pause(text),
                Err(format!("Pause for 15m, 1h, tomorrow or off, not `{text}`")),
                "{text}"
            );
        }
    }
}
