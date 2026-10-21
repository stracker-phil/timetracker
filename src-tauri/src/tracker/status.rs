use chrono::{DateTime, TimeZone};

use super::TrackerState;
use crate::db::activities::ActivityRow;
use crate::db::time::{Day, Ms};

/// The status line shared by the tray and the sidebar, with times in `tz`.
pub fn status_text<Tz: TimeZone>(
    state: &TrackerState,
    current: Option<&ActivityRow>,
    now: Ms,
    tz: &Tz,
) -> String
where
    Tz::Offset: std::fmt::Display,
{
    match state {
        TrackerState::Running => match current {
            Some(activity) => format!("Tracking · {}", activity.app),
            None => "Tracking".into(),
        },
        TrackerState::Away { since, .. } => format!("Away since {}", clock(*since, tz)),
        TrackerState::Paused { until: None } => "Paused".into(),
        TrackerState::Paused { until: Some(until) } => {
            if *until == Day::of_in(now, tz).next().bounds_in(tz).0 {
                "Paused until tomorrow".into()
            } else {
                format!("Paused until {}", clock(*until, tz))
            }
        }
        TrackerState::NeedsPermission => "Needs permission".into(),
    }
}

fn clock<Tz: TimeZone>(ms: Ms, tz: &Tz) -> String
where
    Tz::Offset: std::fmt::Display,
{
    DateTime::from_timestamp_millis(ms)
        .unwrap_or_default()
        .with_timezone(tz)
        .format("%H:%M")
        .to_string()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use chrono_tz::Europe::Vienna;

    use super::*;
    use crate::db::away::AwayReason;

    /// 14:00 in Vienna (UTC+2).
    fn now() -> Ms {
        Utc.with_ymd_and_hms(2026, 10, 21, 12, 0, 0)
            .unwrap()
            .timestamp_millis()
    }

    fn text(state: TrackerState, current: Option<&ActivityRow>) -> String {
        status_text(&state, current, now(), &Vienna)
    }

    fn activity(app: &str) -> ActivityRow {
        ActivityRow {
            id: 1,
            source: "tracker".into(),
            start: now(),
            end: None,
            app: app.into(),
            bundle_id: None,
            title: String::new(),
            url: None,
            path: None,
            private: false,
        }
    }

    #[test]
    fn running_names_the_current_app() {
        assert_eq!(
            text(TrackerState::Running, Some(&activity("Code"))),
            "Tracking · Code"
        );
    }

    #[test]
    fn running_without_an_activity_just_says_tracking() {
        assert_eq!(text(TrackerState::Running, None), "Tracking");
    }

    #[test]
    fn away_shows_the_local_start_time() {
        let state = TrackerState::Away {
            since: now() - 3_600_000,
            reason: AwayReason::Idle,
        };
        assert_eq!(text(state, None), "Away since 13:00");
    }

    #[test]
    fn an_open_ended_pause_says_only_paused() {
        assert_eq!(text(TrackerState::Paused { until: None }, None), "Paused");
    }

    #[test]
    fn a_pause_until_the_next_local_midnight_says_tomorrow() {
        let midnight = Day::of_in(now(), &Vienna).next().bounds_in(&Vienna).0;
        let state = TrackerState::Paused {
            until: Some(midnight),
        };
        assert_eq!(text(state, None), "Paused until tomorrow");
    }

    #[test]
    fn a_pause_until_another_time_shows_the_local_time() {
        let state = TrackerState::Paused {
            until: Some(now() + 90 * 60_000),
        };
        assert_eq!(text(state, None), "Paused until 15:30");
    }

    #[test]
    fn missing_permission_is_named() {
        assert_eq!(
            text(TrackerState::NeedsPermission, None),
            "Needs permission"
        );
    }
}
