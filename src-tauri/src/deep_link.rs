//! `timetracker://` links from other apps. macOS hands a link to the running app, or starts it first.
//! `show` links are queued like a launch, so they wait until the page listens; activity links are
//! written at once. A link that fails shows as an error in the window.

use chrono::TimeZone;
use tauri::AppHandle;
use tauri_plugin_deep_link::DeepLinkExt;
use url::Url;

use crate::activity::ActivityInput;
use crate::cli::time;
use crate::db::time::{Ms, now_ms};
use crate::ipc::IpcMessage;
use crate::{launch, remote};

pub fn listen(handle: &AppHandle) {
    let app = handle.clone();
    handle.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            log::info!("Received a deep link");
            open(&app, url.as_str());
        }
    });
}

fn open(app: &AppHandle, url: &str) {
    match link_message(url, now_ms(), &chrono::Local) {
        message @ (IpcMessage::Show { .. } | IpcMessage::Error { .. }) => {
            launch::queue(app, message)
        }
        message => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = remote::handle(app.clone(), message).await {
                    launch::queue(&app, IpcMessage::Error { message: e.message });
                }
            });
        }
    }
}

/// The message a link stands for, or an `Error` naming what is wrong with it. Times read as in the CLI.
pub fn link_message<Tz: TimeZone>(url: &str, now: Ms, tz: &Tz) -> IpcMessage {
    parse_link(url, now, tz).unwrap_or_else(|message| IpcMessage::Error {
        message: format!("A timetracker:// link failed: {message}"),
    })
}

fn parse_link<Tz: TimeZone>(url: &str, now: Ms, tz: &Tz) -> Result<IpcMessage, String> {
    let url = Url::parse(url).map_err(|e| e.to_string())?;
    let param = |name: &str| {
        url.query_pairs()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned())
            .filter(|value| !value.is_empty())
    };
    let missing = |name: &str| format!("`{name}` is missing");
    let required = |name: &str| param(name).ok_or_else(|| missing(name));
    let instant = |name: &str| {
        param(name)
            .map(|value| time::instant(&value, now, tz))
            .transpose()
    };
    let required_instant = |name: &str| instant(name)?.ok_or_else(|| missing(name));
    let input = |start: Ms| -> Result<ActivityInput, String> {
        Ok(ActivityInput {
            source: required("source")?,
            title: required("title")?,
            start,
            end: instant("end")?,
            app: param("app"),
            url: param("url"),
            path: param("path"),
        })
    };
    let path = url.path().trim_end_matches('/');
    match (url.host_str().unwrap_or_default(), path) {
        ("show", "") => Ok(IpcMessage::Show {
            day: param("day")
                .map(|day| time::day(&day, now, tz))
                .transpose()?,
            focus: true,
        }),
        ("activity", "") => Ok(IpcMessage::ActivityAdd {
            items: vec![input(required_instant("start")?)?],
        }),
        ("activity", "/start") => Ok(IpcMessage::ActivityStart {
            input: input(instant("start")?.unwrap_or(now))?,
        }),
        ("activity", "/stop") => Ok(IpcMessage::ActivityStop {
            source: required("source")?,
            end: instant("end")?,
        }),
        _ => Err("TimeTracker has no such link".into()),
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use chrono_tz::Europe::Vienna;

    use super::*;
    use crate::db::time::Day;

    fn ms(rfc3339: &str) -> Ms {
        DateTime::parse_from_rfc3339(rfc3339)
            .unwrap()
            .timestamp_millis()
    }

    fn now() -> Ms {
        ms("2026-10-21T12:00:00+02:00")
    }

    fn link(url: &str) -> IpcMessage {
        link_message(url, now(), &Vienna)
    }

    fn day(text: &str) -> Day {
        Day::parse(text).unwrap()
    }

    fn input(title: &str) -> ActivityInput {
        ActivityInput {
            source: "planner".into(),
            title: title.into(),
            start: ms("2026-10-21T10:00:00+02:00"),
            end: Some(ms("2026-10-21T10:45:00+02:00")),
            app: None,
            url: None,
            path: None,
        }
    }

    fn failure(reason: &str) -> IpcMessage {
        IpcMessage::Error {
            message: format!("A timetracker:// link failed: {reason}"),
        }
    }

    #[test]
    fn show_links_bring_the_window_to_the_front() {
        for (url, expected) in [
            ("timetracker://show", None),
            ("timetracker://show/", None),
            ("timetracker://show?day=2026-10-21", Some(day("2026-10-21"))),
            ("timetracker://show?day=yesterday", Some(day("2026-10-20"))),
            ("timetracker://show?day=today", Some(day("2026-10-21"))),
            ("timetracker://show?day=", None),
        ] {
            assert_eq!(
                link(url),
                IpcMessage::Show {
                    day: expected,
                    focus: true
                },
                "{url}"
            );
        }
    }

    #[test]
    fn activity_link_adds_one_closed_span_with_decoded_text() {
        let mut expected = input("Meeting: X");
        expected.app = Some("Calendar".into());
        assert_eq!(
            link(
                "timetracker://activity?source=planner&title=Meeting%3A%20X&start=2026-10-21T10:00&end=2026-10-21T10:45&app=Calendar"
            ),
            IpcMessage::ActivityAdd {
                items: vec![expected]
            }
        );
    }

    #[test]
    fn plus_in_the_query_decodes_to_a_space() {
        let mut expected = input("Team sync");
        expected.url = Some("https://example.com/a b".into());
        assert_eq!(
            link(
                "timetracker://activity?source=planner&title=Team+sync&start=2026-10-21T10:00&end=2026-10-21T10:45&url=https%3A%2F%2Fexample.com%2Fa+b"
            ),
            IpcMessage::ActivityAdd {
                items: vec![expected]
            }
        );
    }

    #[test]
    fn an_encoded_plus_gives_a_time_an_offset() {
        let mut expected = input("T");
        expected.start = ms("2026-10-21T10:00:00+05:30");
        expected.end = Some(ms("2026-10-21T11:00:00Z"));
        assert_eq!(
            link(
                "timetracker://activity?source=planner&title=T&start=2026-10-21T10:00%2B05:30&end=2026-10-21T11:00Z"
            ),
            IpcMessage::ActivityAdd {
                items: vec![expected]
            }
        );
    }

    #[test]
    fn activity_start_link_opens_a_span_at_now_by_default() {
        let mut expected = input("Break");
        expected.start = now();
        expected.end = None;
        assert_eq!(
            link("timetracker://activity/start?source=planner&title=Break"),
            IpcMessage::ActivityStart { input: expected }
        );
    }

    #[test]
    fn activity_start_link_takes_an_explicit_start() {
        let mut expected = input("Break");
        expected.end = None;
        assert_eq!(
            link("timetracker://activity/start?source=planner&title=Break&start=10:00"),
            IpcMessage::ActivityStart { input: expected }
        );
    }

    #[test]
    fn activity_stop_link_closes_at_now_or_at_the_given_end() {
        assert_eq!(
            link("timetracker://activity/stop?source=planner"),
            IpcMessage::ActivityStop {
                source: "planner".into(),
                end: None
            }
        );
        assert_eq!(
            link("timetracker://activity/stop?source=planner&end=2026-10-21T10:45"),
            IpcMessage::ActivityStop {
                source: "planner".into(),
                end: Some(ms("2026-10-21T10:45:00+02:00"))
            }
        );
    }

    #[test]
    fn bad_links_become_errors_naming_the_problem() {
        let cases = [
            (
                "timetracker://activity?title=T&start=10:00&end=10:45",
                failure("`source` is missing"),
            ),
            (
                "timetracker://activity?source=planner&start=10:00&end=10:45",
                failure("`title` is missing"),
            ),
            (
                "timetracker://activity?source=planner&title=T&end=10:45",
                failure("`start` is missing"),
            ),
            (
                "timetracker://activity/start?title=Break",
                failure("`source` is missing"),
            ),
            (
                "timetracker://activity/stop",
                failure("`source` is missing"),
            ),
            (
                "timetracker://activity?source=planner&title=T&start=nonsense&end=10:45",
                failure("Not a time: `nonsense` (use 2026-10-21T09:30, 09:30 or now)"),
            ),
            (
                "timetracker://activity/stop?source=planner&end=soon",
                failure("Not a time: `soon` (use 2026-10-21T09:30, 09:30 or now)"),
            ),
            (
                "timetracker://show?day=someday",
                failure("Not a day: `someday` (use YYYY-MM-DD, today or yesterday)"),
            ),
            (
                "timetracker://nope",
                failure("TimeTracker has no such link"),
            ),
            (
                "timetracker://activity/pause?source=planner",
                failure("TimeTracker has no such link"),
            ),
        ];
        for (url, expected) in cases {
            assert_eq!(link(url), expected, "{url}");
        }
    }

    #[test]
    fn text_that_is_not_a_url_becomes_an_error() {
        for url in ["", "not a url", "show"] {
            let IpcMessage::Error { message } = link(url) else {
                panic!("expected an error for `{url}`");
            };
            assert!(
                message.starts_with("A timetracker:// link failed: "),
                "{url}: {message}"
            );
        }
    }
}
