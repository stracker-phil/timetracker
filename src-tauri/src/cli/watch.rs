//! What `timetracker watch` prints per event; `--json` prints the event's line itself.

use chrono::TimeZone;

use super::reply::{timer, tracker_state};
use crate::db::time::Ms;
use crate::events::DataKind;
use crate::watch::WatchEvent;

/// `timer: Acme · standup · 0:25`, `tracker: paused`, `data: recordings 2026-10-21`.
pub fn line<Tz: TimeZone>(event: &WatchEvent, now: Ms, tz: &Tz) -> String {
    match event {
        WatchEvent::Timer(state) => format!("timer: {}", timer(state, now)),
        WatchEvent::Tracker(status) => {
            format!("tracker: {}", tracker_state(Some(status), now, tz))
        }
        WatchEvent::Data(changed) => {
            let kind = DataKind::as_str(changed.kind);
            match changed.day {
                Some(day) => format!("data: {kind} {day}"),
                None => format!("data: {kind}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use chrono_tz::Europe::Vienna;

    use super::*;
    use crate::capture::PermissionStatus;
    use crate::db::time::Day;
    use crate::events::DataChanged;
    use crate::scripting::timer::TimerState;
    use crate::tracker::{TrackerState, TrackerStatus};

    fn ms(rfc3339: &str) -> Ms {
        DateTime::parse_from_rfc3339(rfc3339)
            .unwrap()
            .timestamp_millis()
    }

    fn now() -> Ms {
        ms("2026-10-21T12:00:00+02:00")
    }

    fn tracker(state: TrackerState) -> WatchEvent {
        WatchEvent::Tracker(TrackerStatus {
            state,
            current: None,
            permission: PermissionStatus::Granted,
            text: String::new(),
        })
    }

    fn data(kind: DataKind, day: Option<&str>) -> WatchEvent {
        WatchEvent::Data(DataChanged {
            kind,
            day: day.map(|d| Day::parse(d).unwrap()),
        })
    }

    #[test]
    fn a_running_timer_prints_its_project_note_and_elapsed_time() {
        let event = WatchEvent::Timer(TimerState {
            running: true,
            project: Some("Acme".into()),
            note: Some("standup".into()),
            started_at: Some(ms("2026-10-21T11:35:00+02:00")),
            ..TimerState::default()
        });

        assert_eq!(line(&event, now(), &Vienna), "timer: Acme · standup · 0:25");
    }

    #[test]
    fn a_stopped_timer_prints_none() {
        let event = WatchEvent::Timer(TimerState::default());

        assert_eq!(line(&event, now(), &Vienna), "timer: none");
    }

    #[test]
    fn the_tracker_prints_its_state_in_the_given_time_zone() {
        for (state, expected) in [
            (TrackerState::Running, "tracker: running"),
            (TrackerState::Paused { until: None }, "tracker: paused"),
            (
                TrackerState::Paused {
                    until: Some(ms("2026-10-21T14:30:00+02:00")),
                },
                "tracker: paused until 14:30",
            ),
            (TrackerState::NeedsPermission, "tracker: needs permission"),
        ] {
            assert_eq!(
                line(&tracker(state), now(), &Vienna),
                expected,
                "{expected}"
            );
        }
    }

    #[test]
    fn a_data_change_prints_its_kind_and_day() {
        assert_eq!(
            line(
                &data(DataKind::Recordings, Some("2026-10-21")),
                now(),
                &Vienna
            ),
            "data: recordings 2026-10-21"
        );
    }

    #[test]
    fn a_data_change_without_a_day_prints_only_its_kind() {
        for (kind, expected) in [
            (DataKind::Activity, "data: activity"),
            (DataKind::Recordings, "data: recordings"),
            (DataKind::Projects, "data: projects"),
            (DataKind::Tags, "data: tags"),
            (DataKind::Rules, "data: rules"),
            (DataKind::Settings, "data: settings"),
        ] {
            assert_eq!(
                line(&data(kind, None), now(), &Vienna),
                expected,
                "{expected}"
            );
        }
    }
}
