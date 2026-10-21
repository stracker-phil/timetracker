//! What a subcommand prints for the app's reply, one fact per line so scripts can grep it.
//! `--json` prints the reply itself instead.

use chrono::{DateTime, TimeZone};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::activity::external::SOURCE_PREFIX;
use crate::db::activities::ActivityRow;
use crate::db::time::{Day, Ms};
use crate::ipc::IpcMessage;
use crate::remote::StatusReport;
use crate::tracker::{TrackerState, TrackerStatus};

/// The text for `result`, the reply to `message`; `None` when the subcommand prints nothing.
pub fn render<Tz: TimeZone>(
    message: &IpcMessage,
    result: &Value,
    now: Ms,
    tz: &Tz,
) -> Result<Option<String>, String> {
    Ok(match message {
        IpcMessage::Status => Some(status(&read::<StatusReport>(result)?, now, tz)),
        IpcMessage::Pause { .. } => {
            let status = read::<TrackerStatus>(result)?;
            Some(format!(
                "tracker: {}",
                tracker_state(Some(&status), now, tz)
            ))
        }
        IpcMessage::ActivityAdd { .. } => {
            let ids = read::<Vec<i64>>(&result["ids"])?;
            Some(match ids.as_slice() {
                [id] => format!("added activity {id}"),
                ids => format!("added {} activities", ids.len()),
            })
        }
        IpcMessage::ActivityStart { .. } => {
            let row = read::<ActivityRow>(result)?;
            Some(format!(
                "started {} at {}",
                source_name(&row),
                clock(row.start, tz)
            ))
        }
        IpcMessage::ActivityStop { source, .. } => {
            Some(match read::<Option<ActivityRow>>(result)? {
                Some(row) => {
                    let end = row.end.unwrap_or(now);
                    format!(
                        "stopped {} at {} ({})",
                        source_name(&row),
                        clock(end, tz),
                        duration(end - row.start)
                    )
                }
                None => format!("nothing to stop for {source}"),
            })
        }
        _ => None,
    })
}

fn read<T: DeserializeOwned>(value: &Value) -> Result<T, String> {
    T::deserialize(value)
        .map_err(|e| format!("The running TimeTracker sent an unexpected reply: {e}"))
}

fn status<Tz: TimeZone>(report: &StatusReport, now: Ms, tz: &Tz) -> String {
    [
        format!("TimeTracker {}", report.version),
        format!(
            "tracker: {}",
            tracker_state(report.tracker.as_ref(), now, tz)
        ),
        format!("current: {}", current(report.tracker.as_ref(), now)),
        "timer: none".into(),
        format!(
            "window: {}",
            if report.window_visible {
                "visible"
            } else {
                "hidden"
            }
        ),
    ]
    .join("\n")
}

/// `Safari · Pull request #42 · 13 min`, or `none`.
fn current(tracker: Option<&TrackerStatus>, now: Ms) -> String {
    let Some(row) = tracker.and_then(|t| t.current.as_ref()) else {
        return "none".into();
    };
    let mut parts = vec![row.app.clone()];
    if !row.title.is_empty() {
        parts.push(row.title.clone());
    }
    parts.push(duration(now - row.start));
    parts.join(" · ")
}

/// `None` is a tracker that is off (no data store).
fn tracker_state<Tz: TimeZone>(status: Option<&TrackerStatus>, now: Ms, tz: &Tz) -> String {
    let Some(status) = status else {
        return "off".into();
    };
    match &status.state {
        TrackerState::Running => "running".into(),
        TrackerState::Paused { until: None } => "paused".into(),
        TrackerState::Paused { until: Some(until) } => {
            if *until == Day::of_in(now, tz).next().bounds_in(tz).0 {
                "paused until tomorrow".into()
            } else {
                format!("paused until {}", clock(*until, tz))
            }
        }
        TrackerState::Away { since, reason } => {
            format!("away since {} ({})", clock(*since, tz), reason.label())
        }
        TrackerState::NeedsPermission => "needs permission".into(),
    }
}

fn source_name(row: &ActivityRow) -> &str {
    row.source
        .strip_prefix(SOURCE_PREFIX)
        .unwrap_or(&row.source)
}

fn clock<Tz: TimeZone>(ms: Ms, tz: &Tz) -> String {
    DateTime::from_timestamp_millis(ms)
        .unwrap_or_default()
        .with_timezone(tz)
        .naive_local()
        .format("%H:%M")
        .to_string()
}

/// Whole minutes: `45 min`, `2 h`, `1 h 5 min`.
fn duration(ms: Ms) -> String {
    let minutes = ms.max(0) / 60_000;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

#[cfg(test)]
mod tests {
    use chrono_tz::Europe::Vienna;
    use serde_json::json;

    use super::*;
    use crate::activity::ActivityInput;
    use crate::capture::PermissionStatus;
    use crate::db::away::AwayReason;
    use crate::tracker::PauseUntil;

    const MINUTE: Ms = 60_000;

    fn ms(rfc3339: &str) -> Ms {
        DateTime::parse_from_rfc3339(rfc3339)
            .unwrap()
            .timestamp_millis()
    }

    fn now() -> Ms {
        ms("2026-10-21T12:00:00+02:00")
    }

    fn row(source: &str, app: &str, title: &str, start: Ms, end: Option<Ms>) -> ActivityRow {
        ActivityRow {
            id: 1,
            source: source.into(),
            start,
            end,
            app: app.into(),
            bundle_id: None,
            title: title.into(),
            url: None,
            path: None,
            private: false,
        }
    }

    fn tracker(state: TrackerState, current: Option<ActivityRow>) -> TrackerStatus {
        TrackerStatus {
            state,
            current,
            permission: PermissionStatus::Granted,
            text: String::new(),
        }
    }

    fn report(tracker: Option<TrackerStatus>, window_visible: bool) -> Value {
        serde_json::to_value(StatusReport {
            version: "1.2.3".into(),
            tracker,
            timer: None,
            window_visible,
        })
        .unwrap()
    }

    fn render_at_now(message: &IpcMessage, result: &Value) -> Result<Option<String>, String> {
        render(message, result, now(), &Vienna)
    }

    fn status_lines(report: &Value) -> Vec<String> {
        render_at_now(&IpcMessage::Status, report)
            .unwrap()
            .unwrap()
            .lines()
            .map(String::from)
            .collect()
    }

    fn activity_input() -> ActivityInput {
        ActivityInput {
            source: "planner".into(),
            title: "T".into(),
            start: now(),
            end: None,
            app: None,
            url: None,
            path: None,
        }
    }

    #[test]
    fn status_prints_five_lines_in_order() {
        let current = row("tracker", "Safari", "Docs", now() - 12 * MINUTE, None);
        let result = report(Some(tracker(TrackerState::Running, Some(current))), true);
        assert_eq!(
            status_lines(&result),
            [
                "TimeTracker 1.2.3",
                "tracker: running",
                "current: Safari · Docs · 12 min",
                "timer: none",
                "window: visible",
            ]
        );
    }

    #[test]
    fn status_without_a_title_skips_the_empty_part() {
        let current = row("tracker", "Finder", "", now() - 90 * MINUTE, None);
        let result = report(Some(tracker(TrackerState::Running, Some(current))), false);
        let lines = status_lines(&result);
        assert_eq!(lines[2], "current: Finder · 1 h 30 min");
        assert_eq!(lines[4], "window: hidden");
    }

    #[test]
    fn status_without_a_current_activity_says_none() {
        let result = report(Some(tracker(TrackerState::Running, None)), true);
        assert_eq!(status_lines(&result)[2], "current: none");
    }

    #[test]
    fn status_names_every_tracker_state() {
        let tomorrow = Day::of_in(now(), &Vienna).next().bounds_in(&Vienna).0;
        let cases = [
            (TrackerState::Running, "running"),
            (TrackerState::Paused { until: None }, "paused"),
            (
                TrackerState::Paused {
                    until: Some(now() + 30 * MINUTE),
                },
                "paused until 12:30",
            ),
            (
                TrackerState::Paused {
                    until: Some(tomorrow),
                },
                "paused until tomorrow",
            ),
            (
                TrackerState::Away {
                    since: now() - 15 * MINUTE,
                    reason: AwayReason::Idle,
                },
                "away since 11:45 (idle)",
            ),
            (TrackerState::NeedsPermission, "needs permission"),
        ];
        for (state, expected) in cases {
            let result = report(Some(tracker(state.clone(), None)), true);
            assert_eq!(
                status_lines(&result)[1],
                format!("tracker: {expected}"),
                "{state:?}"
            );
        }
    }

    #[test]
    fn status_of_an_app_without_a_data_store_says_the_tracker_is_off() {
        let lines = status_lines(&report(None, true));
        assert_eq!(
            lines,
            [
                "TimeTracker 1.2.3",
                "tracker: off",
                "current: none",
                "timer: none",
                "window: visible",
            ]
        );
    }

    #[test]
    fn pause_prints_the_tracker_state() {
        let result = serde_json::to_value(tracker(
            TrackerState::Paused {
                until: Some(ms("2026-10-21T14:30:00+02:00")),
            },
            None,
        ))
        .unwrap();
        let message = IpcMessage::Pause {
            until: PauseUntil::Minutes(150),
        };
        assert_eq!(
            render_at_now(&message, &result),
            Ok(Some("tracker: paused until 14:30".into()))
        );
    }

    #[test]
    fn add_prints_how_many_activities_were_written() {
        let message = IpcMessage::ActivityAdd { items: vec![] };
        for (ids, expected) in [
            (json!([4711]), "added activity 4711"),
            (json!([1, 2, 3]), "added 3 activities"),
        ] {
            assert_eq!(
                render_at_now(&message, &json!({ "ids": ids })),
                Ok(Some(expected.into())),
                "{ids}"
            );
        }
    }

    #[test]
    fn start_prints_the_source_without_its_prefix_and_the_start_time() {
        let started = row(
            "external:planner",
            "planner",
            "Break",
            ms("2026-10-21T10:00:00+02:00"),
            None,
        );
        let message = IpcMessage::ActivityStart {
            input: activity_input(),
        };
        assert_eq!(
            render_at_now(&message, &serde_json::to_value(started).unwrap()),
            Ok(Some("started planner at 10:00".into()))
        );
    }

    #[test]
    fn stop_prints_the_end_time_and_the_duration() {
        let message = IpcMessage::ActivityStop {
            source: "planner".into(),
            end: None,
        };
        let start = ms("2026-10-21T10:00:00+02:00");
        for (end_minutes, expected) in [
            (45, "stopped planner at 10:45 (45 min)"),
            (65, "stopped planner at 11:05 (1 h 5 min)"),
            (120, "stopped planner at 12:00 (2 h)"),
            (0, "stopped planner at 10:00 (0 min)"),
        ] {
            let stopped = row(
                "external:planner",
                "planner",
                "Break",
                start,
                Some(start + end_minutes * MINUTE),
            );
            assert_eq!(
                render_at_now(&message, &serde_json::to_value(stopped).unwrap()),
                Ok(Some(expected.into())),
                "{end_minutes}"
            );
        }
    }

    #[test]
    fn stop_without_an_open_span_says_nothing_was_stopped() {
        let message = IpcMessage::ActivityStop {
            source: "planner".into(),
            end: None,
        };
        assert_eq!(
            render_at_now(&message, &Value::Null),
            Ok(Some("nothing to stop for planner".into()))
        );
    }

    #[test]
    fn commands_without_output_print_nothing() {
        for message in [
            IpcMessage::Hide,
            IpcMessage::Quit,
            IpcMessage::Resume,
            IpcMessage::Show {
                day: None,
                focus: false,
            },
            IpcMessage::Activate { hidden: false },
        ] {
            assert_eq!(
                render_at_now(&message, &Value::Null),
                Ok(None),
                "{message:?}"
            );
        }
    }

    #[test]
    fn a_reply_of_the_wrong_shape_is_an_error() {
        let cases = [
            (IpcMessage::Status, json!({ "nope": 1 })),
            (
                IpcMessage::Pause {
                    until: PauseUntil::Resumed,
                },
                json!("paused"),
            ),
            (IpcMessage::ActivityAdd { items: vec![] }, json!({})),
            (
                IpcMessage::ActivityStart {
                    input: activity_input(),
                },
                json!([1]),
            ),
            (
                IpcMessage::ActivityStop {
                    source: "planner".into(),
                    end: None,
                },
                json!(5),
            ),
        ];
        for (message, result) in cases {
            let err = render_at_now(&message, &result).unwrap_err();
            assert!(
                err.starts_with("The running TimeTracker sent an unexpected reply"),
                "{message:?}: {err}"
            );
        }
    }
}
