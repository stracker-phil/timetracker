//! `timetracker activity`: another app's spans, written into its own lane, and deleting activity.

use std::io::Read;

use chrono::TimeZone;
use clap::{Args, Subcommand};

use super::time;
use crate::activity::{ActivityInput, external};
use crate::db::time::Ms;
use crate::ipc::IpcMessage;

#[derive(Subcommand, Debug)]
pub enum ActivityCommand {
    /// Add a closed span; with --json and no other flags, read a JSON array of spans from stdin
    Add {
        #[command(flatten)]
        span: SpanArgs,
        /// Like --start
        #[arg(long)]
        end: Option<String>,
    },
    /// Open a span, closing the source's open one
    Start {
        #[command(flatten)]
        span: SpanArgs,
    },
    /// Close the source's open span
    Stop {
        #[arg(long)]
        source: String,
        /// Like --start (default now)
        #[arg(long)]
        end: Option<String>,
    },
    /// Delete the activity and away spans of a day or range, whole when they cross it; recordings stay
    Delete {
        /// A day (YYYY-MM-DD, today, yesterday), or with TO the start (2026-10-21T09:30, 09:30)
        from: String,
        /// The end of the range, like the start
        to: Option<String>,
    },
    /// Restore the activity of the last delete, within 30 s of it
    Undo,
}

#[derive(Args, Debug)]
pub struct SpanArgs {
    /// The reporting app's name: lowercase letters, digits, - and _
    #[arg(long)]
    source: Option<String>,
    #[arg(long)]
    title: Option<String>,
    /// 2026-10-21T09:30 (local, or with Z or ±hh:mm), 09:30 (today) or now
    #[arg(long)]
    start: Option<String>,
    /// The app name to show (default the source)
    #[arg(long)]
    app: Option<String>,
    #[arg(long)]
    url: Option<String>,
    #[arg(long)]
    path: Option<String>,
}

impl SpanArgs {
    fn is_empty(&self) -> bool {
        [
            &self.source,
            &self.title,
            &self.start,
            &self.app,
            &self.url,
            &self.path,
        ]
        .iter()
        .all(|arg| arg.is_none())
    }

    /// `default_start` stands in for a missing `--start`.
    fn input<Tz: TimeZone>(
        self,
        default_start: Option<Ms>,
        end: Option<Ms>,
        now: Ms,
        tz: &Tz,
    ) -> Result<ActivityInput, String> {
        let start = match self.start {
            Some(start) => time::instant(&start, now, tz)?,
            None => default_start.ok_or("missing --start")?,
        };
        let input = ActivityInput {
            source: self.source.ok_or("missing --source")?,
            title: self.title.ok_or("missing --title")?,
            start,
            end,
            app: self.app,
            url: self.url,
            path: self.path,
        };
        external::validate(&input, now).map_err(|e| e.to_string())?;
        Ok(input)
    }
}

impl ActivityCommand {
    /// `json` with `add` and no span flags reads the spans from `stdin`.
    pub fn message<Tz: TimeZone>(
        self,
        json: bool,
        now: Ms,
        tz: &Tz,
        stdin: &mut dyn Read,
    ) -> Result<IpcMessage, String> {
        Ok(match self {
            Self::Add { span, end } => IpcMessage::ActivityAdd {
                items: add_items(span, end, json, now, tz, stdin)?,
            },
            Self::Start { span } => IpcMessage::ActivityStart {
                input: span.input(Some(now), None, now, tz)?,
            },
            Self::Stop { source, end } => {
                external::validate_source(&source).map_err(|e| e.to_string())?;
                IpcMessage::ActivityStop {
                    source,
                    end: end.map(|end| time::instant(&end, now, tz)).transpose()?,
                }
            }
            Self::Delete { from, to } => {
                let (from, to) = match to {
                    Some(to) => (time::instant(&from, now, tz)?, time::instant(&to, now, tz)?),
                    None => time::day(&from, now, tz)?.bounds_in(tz),
                };
                IpcMessage::ActivityDelete { from, to }
            }
            Self::Undo => IpcMessage::ActivityDeleteUndo,
        })
    }
}

fn add_items<Tz: TimeZone>(
    span: SpanArgs,
    end: Option<String>,
    json: bool,
    now: Ms,
    tz: &Tz,
    stdin: &mut dyn Read,
) -> Result<Vec<ActivityInput>, String> {
    if json && span.is_empty() && end.is_none() {
        return read_items(stdin, now);
    }
    let end = time::instant(&end.ok_or("missing --end")?, now, tz)?;
    Ok(vec![span.input(None, Some(end), now, tz)?])
}

/// A JSON array of `ActivityInput`, times in UTC milliseconds.
fn read_items(stdin: &mut dyn Read, now: Ms) -> Result<Vec<ActivityInput>, String> {
    let mut text = String::new();
    stdin
        .read_to_string(&mut text)
        .map_err(|e| format!("Couldn't read stdin: {e}"))?;
    let items: Vec<ActivityInput> = serde_json::from_str(&text)
        .map_err(|e| format!("stdin is not a JSON array of activities: {e}"))?;
    if items.is_empty() {
        return Err("stdin holds no activity".into());
    }
    for (index, item) in items.iter().enumerate() {
        external::validate(item, now).map_err(|e| format!("items[{index}]: {e}"))?;
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use chrono_tz::Europe::Vienna;
    use serde_json::json;

    use super::*;
    use crate::cli::CliOutcome;
    use crate::cli::testing::*;

    #[test]
    fn activity_add_builds_one_closed_span_in_the_local_zone() {
        let (message, json, starts_app) = remote(&[
            "activity",
            "add",
            "--source",
            "planner",
            "--title",
            "Weekly sync",
            "--start",
            "2026-10-21T10:00",
            "--end",
            "2026-10-21T10:45",
            "--app",
            "Calendar",
            "--url",
            "https://example.com/sync",
            "--path",
            "/tmp/sync.md",
        ]);
        let mut expected = input("Weekly sync");
        expected.app = Some("Calendar".into());
        expected.url = Some("https://example.com/sync".into());
        expected.path = Some("/tmp/sync.md".into());
        assert_eq!(
            message,
            IpcMessage::ActivityAdd {
                items: vec![expected]
            }
        );
        assert!(!json);
        assert!(!starts_app);
    }

    #[test]
    fn activity_add_with_json_reads_the_items_from_stdin() {
        let items = json!([
            {"source": "planner", "title": "A", "start": ms("2026-10-21T09:00:00+02:00"), "end": ms("2026-10-21T09:30:00+02:00")},
            {"source": "planner", "title": "B", "start": ms("2026-10-21T10:00:00+02:00"), "end": ms("2026-10-21T10:45:00+02:00"), "app": "Calendar"},
        ]);
        let outcome = run_with(&["activity", "add", "--json"], &items.to_string());
        let mut second = input("B");
        second.app = Some("Calendar".into());
        let mut first = input("A");
        first.start = ms("2026-10-21T09:00:00+02:00");
        first.end = Some(ms("2026-10-21T09:30:00+02:00"));
        assert_eq!(
            outcome,
            Ok(CliOutcome::Remote {
                message: IpcMessage::ActivityAdd {
                    items: vec![first, second]
                },
                json: true,
                starts_app: false
            })
        );
    }

    #[test]
    fn activity_add_with_json_rejects_bad_stdin() {
        let valid = json!({"source": "planner", "title": "A", "start": ms("2026-10-21T09:00:00+02:00"), "end": ms("2026-10-21T09:30:00+02:00")});
        let blank_title = json!({"source": "planner", "title": " ", "start": ms("2026-10-21T10:00:00+02:00"), "end": ms("2026-10-21T10:45:00+02:00")});
        let invalid_item = json!([valid, blank_title]).to_string();
        for (stdin, expected) in [
            ("[]", "stdin holds no activity"),
            ("", "stdin is not a JSON array of activities"),
            ("not json", "stdin is not a JSON array of activities"),
            (
                invalid_item.as_str(),
                "items[1]: `title`: must not be empty",
            ),
        ] {
            let err = run_with(&["activity", "add", "--json"], stdin).unwrap_err();
            assert!(!err.in_app, "{stdin}");
            assert!(
                err.message.starts_with(expected),
                "{stdin}: {}",
                err.message
            );
        }
    }

    #[test]
    fn activity_start_opens_a_span_at_now() {
        let (message, _, starts_app) = remote(&[
            "activity", "start", "--source", "planner", "--title", "Break",
        ]);
        let mut expected = input("Break");
        expected.start = now();
        expected.end = None;
        assert_eq!(message, IpcMessage::ActivityStart { input: expected });
        assert!(!starts_app);
    }

    #[test]
    fn activity_start_takes_an_explicit_start() {
        let (message, ..) = remote(&[
            "activity", "start", "--source", "planner", "--title", "Break", "--start", "10:00",
        ]);
        let mut expected = input("Break");
        expected.end = None;
        assert_eq!(message, IpcMessage::ActivityStart { input: expected });
    }

    #[test]
    fn activity_stop_closes_at_now_or_at_the_given_end() {
        assert_eq!(
            remote(&["activity", "stop", "--source", "planner"]).0,
            IpcMessage::ActivityStop {
                source: "planner".into(),
                end: None
            }
        );
        assert_eq!(
            remote(&["activity", "stop", "--source", "planner", "--end", "10:45"]).0,
            IpcMessage::ActivityStop {
                source: "planner".into(),
                end: Some(ms("2026-10-21T10:45:00+02:00"))
            }
        );
    }

    #[test]
    fn activity_delete_of_a_day_spans_that_local_day() {
        for (arg, day_text) in [
            ("today", "2026-10-21"),
            ("yesterday", "2026-10-20"),
            ("2026-10-25", "2026-10-25"),
        ] {
            let (from, to) = day(day_text).bounds_in(&Vienna);
            assert_eq!(
                remote(&["activity", "delete", arg]),
                (IpcMessage::ActivityDelete { from, to }, false, false),
                "{arg}"
            );
        }
    }

    #[test]
    fn activity_delete_of_a_day_with_a_dst_change_spans_its_real_length() {
        let (from, to) = day("2026-10-25").bounds_in(&Vienna);
        assert_eq!(to - from, 25 * 3_600_000);
        assert_eq!(
            remote(&["activity", "delete", "2026-10-25"]).0,
            IpcMessage::ActivityDelete { from, to }
        );
    }

    #[test]
    fn activity_delete_with_two_times_spans_exactly_that_range() {
        for (args, from, to) in [
            (
                ["2026-10-21T09:30", "2026-10-21T11:00"],
                "2026-10-21T09:30:00+02:00",
                "2026-10-21T11:00:00+02:00",
            ),
            (
                ["09:30", "11:00"],
                "2026-10-21T09:30:00+02:00",
                "2026-10-21T11:00:00+02:00",
            ),
            (
                ["2026-10-20T22:00", "now"],
                "2026-10-20T22:00:00+02:00",
                "2026-10-21T12:00:00+02:00",
            ),
        ] {
            assert_eq!(
                remote(&["activity", "delete", args[0], args[1]]).0,
                IpcMessage::ActivityDelete {
                    from: ms(from),
                    to: ms(to)
                },
                "{args:?}"
            );
        }
    }

    #[test]
    fn activity_undo_becomes_its_message() {
        assert_eq!(
            remote(&["activity", "undo"]),
            (IpcMessage::ActivityDeleteUndo, false, false)
        );
    }

    #[test]
    fn activity_delete_rejects_missing_or_unreadable_times() {
        for args in [
            &["activity", "delete"][..],
            &["activity", "delete", "nonsense"],
            &["activity", "delete", "09:30", "nonsense"],
            &["activity", "delete", "nonsense", "11:00"],
            &["activity", "delete", "09:30", "11:00", "12:00"],
            &["activity", "undo", "now"],
        ] {
            let err = run(args).unwrap_err();
            assert!(!err.in_app, "{args:?}");
            assert!(!err.message.is_empty(), "{args:?}");
        }
    }
}
