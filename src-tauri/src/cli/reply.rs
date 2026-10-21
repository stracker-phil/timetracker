//! What a subcommand prints for the app's reply, one fact per line so scripts can grep it.
//! `--json` prints the reply itself instead.

use chrono::{DateTime, TimeZone};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::activity::external::SOURCE_PREFIX;
use crate::db::activities::ActivityRow;
use crate::db::recordings::Recording;
use crate::db::time::{Day, Ms};
use crate::ipc::IpcMessage;
use crate::recordings::cli::Recorded;
use crate::remote::StatusReport;
use crate::reports::cli::Reported;
use crate::timers::RunningTimer;
use crate::timers::commands::TimerStarted;
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
        IpcMessage::Record { .. } => {
            let recorded = read::<Recorded>(result)?;
            let r = &recorded.recording;
            let end = r.end.unwrap_or(now);
            let note = if r.note.is_empty() {
                String::new()
            } else {
                format!(" · {}", r.note)
            };
            Some(format!(
                "recorded {}–{} ({}) for {}{note}",
                clock(r.start, tz),
                clock(end, tz),
                hours(end - r.start),
                recorded.project_name
            ))
        }
        IpcMessage::TimerStart { .. } => {
            let started = read::<TimerStarted>(result)?;
            Some(format!(
                "started {} at {}",
                started.project_name,
                clock(started.recording.start, tz)
            ))
        }
        IpcMessage::TimerStop => Some(match read::<Option<Recording>>(result)? {
            Some(r) => {
                let end = r.end.unwrap_or(now);
                format!(
                    "stopped {}–{} ({})",
                    clock(r.start, tz),
                    clock(end, tz),
                    hours(end - r.start)
                )
            }
            None => "no timer running".into(),
        }),
        IpcMessage::Report { .. } => Some(read::<Reported>(result)?.text.trim_end().to_string()),
        #[cfg(debug_assertions)]
        IpcMessage::DevSeed { .. } => {
            let stats = read::<crate::test_support::seed::SeedStats>(result)?;
            Some(format!(
                "seeded {} workdays: {} activity spans, {} away spans, {} recordings over {} projects",
                stats.days, stats.activities, stats.away_spans, stats.recordings, stats.projects
            ))
        }
        _ => None,
    })
}

/// `1:30`.
fn hours(ms: Ms) -> String {
    let minutes = ms.max(0) / 60_000;
    format!("{}:{:02}", minutes / 60, minutes % 60)
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
        format!("timer: {}", timer(report.timer.as_ref(), now)),
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
/// `Acme · standup · 0:25`, or `none`.
fn timer(timer: Option<&RunningTimer>, now: Ms) -> String {
    let Some(t) = timer else {
        return "none".into();
    };
    let mut parts = vec![t.project_name.clone()];
    if !t.recording.note.is_empty() {
        parts.push(t.recording.note.clone());
    }
    parts.push(hours(now - t.recording.start));
    parts.join(" · ")
}

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
    use crate::db::recordings::Recording;
    use crate::recordings::Overlap;
    use crate::recordings::cli::RecordRequest;
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

    fn record_message() -> IpcMessage {
        IpcMessage::Record {
            request: RecordRequest {
                project: "ac".into(),
                start: 0,
                end: 1,
                note: String::new(),
                tags: vec![],
                overlap: Overlap::Cancel,
            },
        }
    }

    fn recorded(note: &str, start: &str, end: Option<&str>) -> Value {
        serde_json::to_value(Recorded {
            recording: Recording {
                id: "r1".into(),
                project_id: "p1".into(),
                start: ms(start),
                end: end.map(ms),
                note: note.into(),
                tag_ids: vec![],
                created_at: 0,
                updated_at: 0,
            },
            project_name: "Acme".into(),
        })
        .unwrap()
    }

    #[test]
    fn record_prints_the_span_the_duration_the_project_and_the_note() {
        let result = recorded(
            "fix login",
            "2026-10-21T09:00:00+02:00",
            Some("2026-10-21T10:30:00+02:00"),
        );

        assert_eq!(
            render_at_now(&record_message(), &result),
            Ok(Some(
                "recorded 09:00–10:30 (1:30) for Acme · fix login".into()
            ))
        );
    }

    #[test]
    fn record_without_a_note_prints_no_note_part() {
        let result = recorded(
            "",
            "2026-10-21T09:00:00+02:00",
            Some("2026-10-21T09:45:00+02:00"),
        );

        assert_eq!(
            render_at_now(&record_message(), &result),
            Ok(Some("recorded 09:00–09:45 (0:45) for Acme".into()))
        );
    }

    #[test]
    fn record_zero_pads_the_minutes_of_the_duration() {
        let result = recorded(
            "",
            "2026-10-21T09:00:00+02:00",
            Some("2026-10-21T11:05:00+02:00"),
        );

        assert_eq!(
            render_at_now(&record_message(), &result),
            Ok(Some("recorded 09:00–11:05 (2:05) for Acme".into()))
        );
    }

    fn timer_recording(note: &str, start: &str, end: Option<&str>) -> Recording {
        Recording {
            id: "r1".into(),
            project_id: "p1".into(),
            start: ms(start),
            end: end.map(ms),
            note: note.into(),
            tag_ids: vec![],
            created_at: 0,
            updated_at: 0,
        }
    }

    fn timer_message() -> IpcMessage {
        IpcMessage::TimerStart {
            request: crate::timers::commands::TimerStartRequest {
                project: "ac".into(),
                note: String::new(),
                tags: vec![],
            },
        }
    }

    #[test]
    fn timer_start_prints_the_project_and_the_start_time() {
        let result = serde_json::to_value(TimerStarted {
            recording: timer_recording("", "2026-10-21T09:15:00+02:00", None),
            project_name: "Acme".into(),
        })
        .unwrap();

        assert_eq!(
            render_at_now(&timer_message(), &result),
            Ok(Some("started Acme at 09:15".into()))
        );
    }

    #[test]
    fn timer_stop_prints_the_span_and_the_duration() {
        let result = serde_json::to_value(timer_recording(
            "",
            "2026-10-21T09:15:00+02:00",
            Some("2026-10-21T10:40:00+02:00"),
        ))
        .unwrap();

        assert_eq!(
            render_at_now(&IpcMessage::TimerStop, &result),
            Ok(Some("stopped 09:15–10:40 (1:25)".into()))
        );
    }

    #[test]
    fn timer_stop_without_a_timer_says_none_runs() {
        assert_eq!(
            render_at_now(&IpcMessage::TimerStop, &Value::Null),
            Ok(Some("no timer running".into()))
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    fn dev_seed_prints_what_was_seeded() {
        let result = json!({
            "days": 520,
            "activities": 260000,
            "awaySpans": 1560,
            "recordings": 3120,
            "projects": 5,
            "tags": 5,
        });

        assert_eq!(
            render_at_now(&IpcMessage::DevSeed { years: 2, yes: false }, &result),
            Ok(Some(
                "seeded 520 workdays: 260000 activity spans, 1560 away spans, 3120 recordings over 5 projects"
                    .into()
            ))
        );
    }

    #[test]
    fn timer_replies_of_the_wrong_shape_are_errors() {
        for (message, result) in [
            (timer_message(), json!({ "projectName": "Acme" })),
            (IpcMessage::TimerStop, json!("stopped")),
        ] {
            let err = render_at_now(&message, &result).unwrap_err();
            assert!(
                err.starts_with("The running TimeTracker sent an unexpected reply"),
                "{message:?}: {err}"
            );
        }
    }

    #[test]
    fn status_shows_the_running_timer_with_its_note_and_elapsed_time() {
        let start = now() - 25 * MINUTE;
        for (note, expected) in [
            ("standup", "timer: Acme · standup · 0:25"),
            ("", "timer: Acme · 0:25"),
        ] {
            let result = serde_json::to_value(StatusReport {
                version: "1.2.3".into(),
                tracker: None,
                timer: Some(RunningTimer {
                    recording: Recording {
                        start,
                        ..timer_recording(note, "2026-10-21T11:35:00+02:00", None)
                    },
                    project_name: "Acme".into(),
                    color: "blue".into(),
                    shade: 0,
                    elapsed_ms: 25 * MINUTE,
                }),
                window_visible: true,
            })
            .unwrap();

            assert_eq!(status_lines(&result)[3], expected, "{note:?}");
        }
    }

    fn report_message() -> IpcMessage {
        IpcMessage::Report {
            request: crate::reports::cli::ReportRequest {
                range: crate::reports::Range::Today,
                group_by: crate::store::settings::Group::Project,
                rounding: crate::store::settings::Rounding::None,
                clients: vec![],
                projects: vec![],
                tags: vec![],
            },
        }
    }

    #[test]
    fn report_prints_the_reply_text_without_the_trailing_newline() {
        let result = json!({
            "text": "Wed 2026-10-21\n\nAcme / Portal  1:30 h\n\nTotal  1:30 h\n",
            "from": "2026-10-21",
            "totalMs": 5_400_000,
        });
        assert_eq!(
            render_at_now(&report_message(), &result),
            Ok(Some(
                "Wed 2026-10-21\n\nAcme / Portal  1:30 h\n\nTotal  1:30 h".into()
            ))
        );
    }

    #[test]
    fn a_report_reply_without_text_is_an_error() {
        let err = render_at_now(&report_message(), &json!({ "totalMs": 1 })).unwrap_err();
        assert!(
            err.starts_with("The running TimeTracker sent an unexpected reply"),
            "{err}"
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
            (record_message(), json!({ "projectName": "Acme" })),
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
