//! The `timetracker` command line. Without a subcommand it starts the app (or activates the running
//! one); every subcommand becomes one `IpcMessage` that the running app answers (`remote::handle`).
//! A new subcommand adds a `Command` variant, its arm in `message`, a parser test, its arm in
//! `remote::handle` and, when it prints something, its arm in `reply::render`.

pub mod reply;
pub mod report;
pub mod time;

use std::ffi::OsString;
use std::io::Read;

use chrono::TimeZone;
use clap::{Args, CommandFactory, Parser, Subcommand};

use crate::activity::ActivityInput;
use crate::activity::external;
use crate::db::time::Ms;
use crate::ipc::{IpcMessage, MAX_MESSAGE_BYTES};
use crate::recordings::Overlap;
use crate::recordings::cli::RecordRequest;
use crate::store::settings::LogLevel;
use crate::timers::commands::TimerStartRequest;
use report::ReportArgs;

const EXAMPLES: &str = "Examples:
  timetracker                     start TimeTracker or bring it to the front
  timetracker --hidden            start without opening the window (launch at login)
  timetracker show yesterday      show yesterday's Day view
  timetracker status --json       the tracker state as JSON
  timetracker pause 1h            pause tracking for an hour
  timetracker activity add --source planner --title \"Weekly sync\" --start 10:00 --end 10:45
  timetracker activity start --source planner --title Break
  timetracker activity stop --source planner
  timetracker activity delete today   delete today's activity (undo within 30 s: activity undo)
  timetracker record --project acme --from 09:00 --to 09:30 --note standup --tag \"internal meeting\"
  timetracker report last-week --by tag --round 15
  timetracker start acme --note \"weekly call\" --tag \"external meeting\"
  timetracker stop
  claude mcp add timetracker -- ~/.local/bin/timetracker mcp";

/// Items per request of a batch; the line limit may split it further.
const MAX_BATCH_ITEMS: usize = 200;
/// Room for the request around the items (`IpcRequest` id, message tag).
const REQUEST_OVERHEAD: usize = 1024;

#[derive(Parser, Debug)]
#[command(
    name = "timetracker",
    version,
    about = "Local-first automatic time tracker",
    after_help = EXAMPLES
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Start without showing the window; a running instance stays as it is
    #[arg(long)]
    hidden: bool,
    /// Log level for this run
    #[arg(long, value_enum)]
    log_level: Option<LogLevel>,
    /// Print replies as JSON
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Show the window at a day's Day view, starting TimeTracker if needed
    Show {
        /// YYYY-MM-DD, today or yesterday (default today)
        day: Option<String>,
    },
    /// Show the window and bring it to the front, starting TimeTracker if needed
    Focus,
    /// Hide the window; TimeTracker keeps running in the menu bar
    Hide,
    /// Quit TimeTracker
    Quit,
    /// Print the tracker state, the current activity and whether the window shows
    Status,
    /// Pause tracking
    Pause {
        /// 15m, 1h (up to 24h), tomorrow (until midnight) or off (until resumed)
        until: String,
    },
    /// Resume paused tracking
    Resume,
    /// Write another app's activity (a planner, a meeting tool) into its own lane, or delete activity
    #[command(subcommand)]
    Activity(ActivityCommand),
    /// Record time for a project
    Record {
        /// The project's id, name or a unique start of its name
        #[arg(long)]
        project: String,
        /// 2026-10-21T09:30 (local, or with Z or ±hh:mm), 09:30 (today) or now
        #[arg(long)]
        from: String,
        /// Like --from
        #[arg(long)]
        to: String,
        /// One line
        #[arg(long, default_value = "")]
        note: String,
        /// A tag by name; repeat for several
        #[arg(long = "tag")]
        tags: Vec<String>,
        /// Trim or remove the recordings in the way instead of refusing
        #[arg(long)]
        replace: bool,
    },
    /// Print the hours and notes of a day or range, ready to paste into a timesheet
    Report(ReportArgs),
    /// Start a timer for a project now; a running timer stops
    Start {
        /// The project's id, name or a unique start of its name
        project: String,
        /// One line
        #[arg(long, default_value = "")]
        note: String,
        /// A tag by name; repeat for several
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    /// Stop the running timer
    Stop,
    /// Serve a read-only MCP server on stdin/stdout for Claude and other MCP clients; it reads the data
    /// itself, so it works while the app is closed (`claude mcp add timetracker -- ~/.local/bin/timetracker mcp`)
    Mcp,
    /// Developer tools (debug builds only)
    #[cfg(debug_assertions)]
    #[command(subcommand)]
    Dev(DevCommand),
}

#[cfg(debug_assertions)]
#[derive(Subcommand, Debug)]
enum DevCommand {
    /// Write generated workdays up to yesterday into the running app's database; refused when it
    /// holds activity before today
    Seed {
        /// How many years back (1 to 10)
        #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u32).range(1..=10))]
        years: u32,
        /// Seed even when the database holds activity before today
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ActivityCommand {
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
struct SpanArgs {
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

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum CliOutcome {
    Run {
        hidden: bool,
        log_level: Option<LogLevel>,
    },
    /// `--help` / `--version`: print and exit without starting the app.
    Exit { message: String },
    /// `timetracker mcp`: serve MCP on stdio instead of starting or asking the app.
    Mcp,
    /// A subcommand for the running app. `starts_app`: with no app running, start it with the message.
    Remote {
        message: IpcMessage,
        json: bool,
        starts_app: bool,
    },
}

#[derive(Debug, PartialEq)]
pub struct CliError {
    pub message: String,
    /// A plain launch with bad arguments also reports them in the app, which may have been opened
    /// from Finder; a subcommand's error is for the terminal only.
    pub in_app: bool,
}

/// Parses the full argv (including the program name). Times are read in `tz` at `now`; `stdin` is
/// read only by `activity add --json`.
pub fn parse<I, S, Tz>(
    args: I,
    now: Ms,
    tz: &Tz,
    stdin: &mut dyn Read,
) -> Result<CliOutcome, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString> + Clone,
    Tz: TimeZone,
{
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    let in_app = !names_subcommand(&args);
    let error = |message: String| CliError { message, in_app };
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) => {
            return match e.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                    Ok(CliOutcome::Exit {
                        message: e.to_string(),
                    })
                }
                _ => Err(error(e.to_string())),
            };
        }
    };
    let Some(command) = cli.command else {
        return Ok(CliOutcome::Run {
            hidden: cli.hidden,
            log_level: cli.log_level,
        });
    };
    if matches!(command, Command::Mcp) {
        return Ok(CliOutcome::Mcp);
    }
    let starts_app = matches!(command, Command::Show { .. } | Command::Focus);
    let message = message(command, cli.json, now, tz, stdin).map_err(error)?;
    Ok(CliOutcome::Remote {
        message,
        json: cli.json,
        starts_app,
    })
}

fn names_subcommand(args: &[OsString]) -> bool {
    let cli = Cli::command();
    let names: Vec<&str> = cli.get_subcommands().map(|c| c.get_name()).collect();
    args.iter()
        .skip(1)
        .any(|arg| arg.to_str().is_some_and(|arg| names.contains(&arg)))
}

fn message<Tz: TimeZone>(
    command: Command,
    json: bool,
    now: Ms,
    tz: &Tz,
    stdin: &mut dyn Read,
) -> Result<IpcMessage, String> {
    Ok(match command {
        Command::Show { day } => IpcMessage::Show {
            day: day.map(|day| time::day(&day, now, tz)).transpose()?,
            focus: false,
        },
        Command::Focus => IpcMessage::Activate { hidden: false },
        Command::Hide => IpcMessage::Hide,
        Command::Quit => IpcMessage::Quit,
        Command::Status => IpcMessage::Status,
        Command::Pause { until } => IpcMessage::Pause {
            until: time::pause(&until)?,
        },
        Command::Resume => IpcMessage::Resume,
        Command::Activity(ActivityCommand::Add { span, end }) => IpcMessage::ActivityAdd {
            items: add_items(span, end, json, now, tz, stdin)?,
        },
        Command::Activity(ActivityCommand::Start { span }) => IpcMessage::ActivityStart {
            input: span.input(Some(now), None, now, tz)?,
        },
        Command::Record {
            project,
            from,
            to,
            note,
            tags,
            replace,
        } => IpcMessage::Record {
            request: RecordRequest {
                project,
                start: time::instant(&from, now, tz)?,
                end: time::instant(&to, now, tz)?,
                note,
                tags,
                overlap: if replace {
                    Overlap::Replace
                } else {
                    Overlap::Cancel
                },
            },
        },
        Command::Start {
            project,
            note,
            tags,
        } => IpcMessage::TimerStart {
            request: TimerStartRequest {
                project,
                note,
                tags,
            },
        },
        Command::Stop => IpcMessage::TimerStop,
        #[cfg(debug_assertions)]
        Command::Dev(DevCommand::Seed { years, yes }) => IpcMessage::DevSeed { years, yes },
        Command::Mcp => unreachable!("`parse` serves `mcp` before it builds a message"),
        Command::Report(args) => IpcMessage::Report {
            request: args.request(now, tz)?,
        },
        Command::Activity(ActivityCommand::Delete { from, to }) => {
            let (from, to) = match to {
                Some(to) => (time::instant(&from, now, tz)?, time::instant(&to, now, tz)?),
                None => time::day(&from, now, tz)?.bounds_in(tz),
            };
            IpcMessage::ActivityDelete { from, to }
        }
        Command::Activity(ActivityCommand::Undo) => IpcMessage::ActivityDeleteUndo,
        Command::Activity(ActivityCommand::Stop { source, end }) => {
            external::validate_source(&source).map_err(|e| e.to_string())?;
            IpcMessage::ActivityStop {
                source,
                end: end.map(|end| time::instant(&end, now, tz)).transpose()?,
            }
        }
    })
}

/// The span of the flags, or with `--json` and no span flags the array on stdin.
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

/// The requests that carry `message`: an activity batch is cut so each request stays within the
/// socket's line limit. A batch cut in parts is written part by part, not all or none.
pub fn split(message: IpcMessage) -> Result<Vec<IpcMessage>, String> {
    let IpcMessage::ActivityAdd { items } = message else {
        return Ok(vec![message]);
    };
    let budget = MAX_MESSAGE_BYTES as usize - REQUEST_OVERHEAD;
    let mut requests = Vec::new();
    let mut part = Vec::new();
    let mut bytes = 0;
    for (index, item) in items.into_iter().enumerate() {
        let size = serde_json::to_string(&item)
            .map_err(|e| e.to_string())?
            .len()
            + 1;
        if size > budget {
            return Err(format!("items[{index}] is too large to send"));
        }
        if !part.is_empty() && (bytes + size > budget || part.len() == MAX_BATCH_ITEMS) {
            requests.push(IpcMessage::ActivityAdd {
                items: std::mem::take(&mut part),
            });
            bytes = 0;
        }
        bytes += size;
        part.push(item);
    }
    requests.push(IpcMessage::ActivityAdd { items: part });
    Ok(requests)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use chrono::DateTime;
    use chrono_tz::Europe::Vienna;
    use serde_json::json;

    use super::*;
    use crate::db::time::Day;
    use crate::ipc::IpcRequest;
    use crate::store::settings::{Group, Rounding};
    use crate::tracker::PauseUntil;

    fn ms(rfc3339: &str) -> Ms {
        DateTime::parse_from_rfc3339(rfc3339)
            .unwrap()
            .timestamp_millis()
    }

    fn now() -> Ms {
        ms("2026-10-21T12:00:00+02:00")
    }

    fn run_with(args: &[&str], stdin: &str) -> Result<CliOutcome, CliError> {
        let argv = std::iter::once("timetracker").chain(args.iter().copied());
        parse(argv, now(), &Vienna, &mut Cursor::new(stdin.to_string()))
    }

    fn run(args: &[&str]) -> Result<CliOutcome, CliError> {
        run_with(args, "")
    }

    fn remote(args: &[&str]) -> (IpcMessage, bool, bool) {
        match run(args).unwrap() {
            CliOutcome::Remote {
                message,
                json,
                starts_app,
            } => (message, json, starts_app),
            other => panic!("not a remote outcome: {other:?}"),
        }
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

    #[test]
    fn no_arguments_start_the_app() {
        assert_eq!(
            run(&[]),
            Ok(CliOutcome::Run {
                hidden: false,
                log_level: None
            })
        );
    }

    #[test]
    fn the_mcp_subcommand_serves_without_starting_the_app() {
        assert_eq!(run(&["mcp"]), Ok(CliOutcome::Mcp));
    }

    #[test]
    fn hidden_flag_starts_the_app_without_a_window() {
        assert_eq!(
            run(&["--hidden"]),
            Ok(CliOutcome::Run {
                hidden: true,
                log_level: None
            })
        );
    }

    #[test]
    fn log_level_flag_is_parsed_for_each_level() {
        for (arg, level) in [
            ("error", LogLevel::Error),
            ("warn", LogLevel::Warn),
            ("info", LogLevel::Info),
            ("debug", LogLevel::Debug),
            ("trace", LogLevel::Trace),
        ] {
            assert_eq!(
                run(&["--log-level", arg]),
                Ok(CliOutcome::Run {
                    hidden: false,
                    log_level: Some(level)
                }),
                "{arg}"
            );
        }
    }

    #[test]
    fn help_and_version_exit_without_starting_the_app() {
        let Ok(CliOutcome::Exit { message }) = run(&["--version"]) else {
            panic!("--version should exit");
        };
        assert_eq!(
            message.trim_end(),
            format!("timetracker {}", env!("CARGO_PKG_VERSION"))
        );
        assert!(matches!(run(&["--help"]), Ok(CliOutcome::Exit { .. })));
    }

    #[test]
    fn help_lists_the_subcommands() {
        let Ok(CliOutcome::Exit { message }) = run(&["--help"]) else {
            panic!("--help should exit");
        };
        for name in [
            "show", "focus", "hide", "quit", "status", "pause", "resume", "activity", "record",
        ] {
            assert!(message.contains(&format!("  {name} ")), "{name}");
        }
    }

    #[test]
    fn bad_flags_without_a_subcommand_are_reported_in_the_app() {
        for args in [&["--nope"][..], &["--log-level", "loud"]] {
            let err = run(args).unwrap_err();
            assert!(err.in_app, "{args:?}");
            assert!(!err.message.is_empty(), "{args:?}");
        }
    }

    #[test]
    fn bad_flags_of_a_subcommand_are_reported_in_the_terminal_only() {
        for args in [&["status", "--nope"][..], &["activity", "stop"], &["pause"]] {
            let err = run(args).unwrap_err();
            assert!(!err.in_app, "{args:?}");
        }
    }

    #[test]
    fn subcommands_become_their_messages() {
        let cases: Vec<(Vec<&str>, IpcMessage, bool)> = vec![
            (
                vec!["show"],
                IpcMessage::Show {
                    day: None,
                    focus: false,
                },
                true,
            ),
            (
                vec!["show", "today"],
                IpcMessage::Show {
                    day: Some(day("2026-10-21")),
                    focus: false,
                },
                true,
            ),
            (
                vec!["show", "yesterday"],
                IpcMessage::Show {
                    day: Some(day("2026-10-20")),
                    focus: false,
                },
                true,
            ),
            (
                vec!["show", "2026-10-21"],
                IpcMessage::Show {
                    day: Some(day("2026-10-21")),
                    focus: false,
                },
                true,
            ),
            (vec!["focus"], IpcMessage::Activate { hidden: false }, true),
            (vec!["hide"], IpcMessage::Hide, false),
            (vec!["quit"], IpcMessage::Quit, false),
            (vec!["status"], IpcMessage::Status, false),
            (vec!["resume"], IpcMessage::Resume, false),
            (
                vec!["pause", "15m"],
                IpcMessage::Pause {
                    until: PauseUntil::Minutes(15),
                },
                false,
            ),
            (
                vec!["pause", "1h"],
                IpcMessage::Pause {
                    until: PauseUntil::Minutes(60),
                },
                false,
            ),
            (
                vec!["pause", "tomorrow"],
                IpcMessage::Pause {
                    until: PauseUntil::Tomorrow,
                },
                false,
            ),
            (
                vec!["pause", "off"],
                IpcMessage::Pause {
                    until: PauseUntil::Resumed,
                },
                false,
            ),
        ];
        for (args, message, starts_app) in cases {
            assert_eq!(
                run(&args),
                Ok(CliOutcome::Remote {
                    message,
                    json: false,
                    starts_app
                }),
                "{args:?}"
            );
        }
    }

    #[test]
    fn json_flag_is_accepted_before_and_after_the_subcommand() {
        for args in [&["status", "--json"][..], &["--json", "status"]] {
            assert_eq!(remote(args), (IpcMessage::Status, true, false), "{args:?}");
        }
    }

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

    #[test]
    fn subcommand_errors_name_the_problem_and_stay_in_the_terminal() {
        let add = |extra: &[&'static str]| -> Vec<&'static str> {
            ["activity", "add"]
                .into_iter()
                .chain(extra.iter().copied())
                .collect()
        };
        let cases: Vec<(Vec<&str>, &str)> = vec![
            (
                add(&["--source", "planner", "--start", "10:00", "--end", "10:45"]),
                "missing --title",
            ),
            (
                add(&["--title", "T", "--start", "10:00", "--end", "10:45"]),
                "missing --source",
            ),
            (
                add(&["--source", "planner", "--title", "T", "--end", "10:45"]),
                "missing --start",
            ),
            (
                add(&["--source", "planner", "--title", "T", "--start", "10:00"]),
                "missing --end",
            ),
            (
                add(&[
                    "--source", "planner", "--title", "T", "--start", "nonsense", "--end", "10:45",
                ]),
                "Not a time: `nonsense`",
            ),
            (
                add(&[
                    "--source", "planner", "--title", "T", "--start", "10:45", "--end", "10:00",
                ]),
                "`end`: must be after the start",
            ),
            (
                add(&[
                    "--source", "tracker", "--title", "T", "--start", "10:00", "--end", "10:45",
                ]),
                "`source`: `tracker` is the built-in tracker's",
            ),
            (
                add(&[
                    "--source", "Planner", "--title", "T", "--start", "10:00", "--end", "10:45",
                ]),
                "`source`: must be 1 to 32 lowercase letters",
            ),
            (
                vec!["activity", "stop", "--source", "Planner"],
                "`source`: must be 1 to 32 lowercase letters",
            ),
            (
                vec!["pause", "soon"],
                "Pause for 15m, 1h, tomorrow or off, not `soon`",
            ),
            (vec!["show", "someday"], "Not a day: `someday`"),
        ];
        for (args, expected) in cases {
            let err = run(&args).unwrap_err();
            assert!(!err.in_app, "{args:?}");
            assert!(
                err.message.starts_with(expected),
                "{args:?}: {}",
                err.message
            );
        }
    }

    fn record_request(note: &str, tags: &[&str], end: &str, overlap: Overlap) -> IpcMessage {
        IpcMessage::Record {
            request: RecordRequest {
                project: "ac".into(),
                start: ms("2026-10-21T09:00:00+02:00"),
                end: ms(end),
                note: note.into(),
                tags: tags.iter().map(|t| t.to_string()).collect(),
                overlap,
            },
        }
    }

    #[test]
    fn record_builds_a_request_with_the_times_in_the_local_zone() {
        let (message, json, starts_app) = remote(&[
            "record",
            "--project",
            "ac",
            "--from",
            "09:00",
            "--to",
            "2026-10-21T10:30",
            "--note",
            "fix login",
            "--tag",
            "development",
            "--tag",
            "internal meeting",
        ]);

        assert_eq!(
            message,
            record_request(
                "fix login",
                &["development", "internal meeting"],
                "2026-10-21T10:30:00+02:00",
                Overlap::Cancel
            )
        );
        assert!(!json);
        assert!(!starts_app);
    }

    #[test]
    fn record_without_note_and_tags_sends_them_empty() {
        let (message, ..) = remote(&[
            "record",
            "--project",
            "ac",
            "--from",
            "09:00",
            "--to",
            "10:30",
        ]);

        assert_eq!(
            message,
            record_request("", &[], "2026-10-21T10:30:00+02:00", Overlap::Cancel)
        );
    }

    #[test]
    fn record_with_replace_overlaps_by_replacing() {
        let (message, ..) = remote(&[
            "record",
            "--project",
            "ac",
            "--from",
            "09:00",
            "--to",
            "10:30",
            "--replace",
        ]);

        assert_eq!(
            message,
            record_request("", &[], "2026-10-21T10:30:00+02:00", Overlap::Replace)
        );
    }

    #[test]
    fn record_accepts_now_and_the_json_flag() {
        let (message, json, _) = remote(&[
            "record",
            "--project",
            "ac",
            "--from",
            "09:00",
            "--to",
            "now",
            "--json",
        ]);

        assert_eq!(
            message,
            record_request("", &[], "2026-10-21T12:00:00+02:00", Overlap::Cancel)
        );
        assert!(json);
    }

    #[test]
    fn start_builds_a_timer_request_with_note_and_tags() {
        let (message, json, starts_app) =
            remote(&["start", "acme", "--note", "x", "--tag", "a", "--tag", "b"]);

        assert_eq!(
            message,
            IpcMessage::TimerStart {
                request: TimerStartRequest {
                    project: "acme".into(),
                    note: "x".into(),
                    tags: vec!["a".into(), "b".into()],
                }
            }
        );
        assert!(!json);
        assert!(!starts_app);
    }

    #[test]
    fn start_without_note_and_tags_sends_them_empty() {
        assert_eq!(
            remote(&["start", "acme", "--json"]),
            (
                IpcMessage::TimerStart {
                    request: TimerStartRequest {
                        project: "acme".into(),
                        note: String::new(),
                        tags: Vec::new(),
                    }
                },
                true,
                false
            )
        );
    }

    #[test]
    fn stop_becomes_a_timer_stop_message() {
        assert_eq!(remote(&["stop"]), (IpcMessage::TimerStop, false, false));
    }

    #[cfg(debug_assertions)]
    #[test]
    fn dev_seed_defaults_to_two_years_and_asks_before_seeding() {
        assert_eq!(
            remote(&["dev", "seed"]),
            (
                IpcMessage::DevSeed {
                    years: 2,
                    yes: false
                },
                false,
                false
            )
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    fn dev_seed_takes_years_and_yes() {
        assert_eq!(
            remote(&["dev", "seed", "--years", "5", "--yes"]).0,
            IpcMessage::DevSeed {
                years: 5,
                yes: true
            }
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    fn dev_seed_rejects_years_outside_one_to_ten() {
        for years in ["0", "11"] {
            assert!(run(&["dev", "seed", "--years", years]).is_err(), "{years}");
        }
    }

    #[test]
    fn start_without_a_project_is_reported_in_the_terminal() {
        let err = run(&["start"]).unwrap_err();

        assert!(!err.in_app);
        assert!(err.message.contains("<PROJECT>"), "{}", err.message);
    }

    #[test]
    fn help_lists_the_timer_subcommands() {
        let Ok(CliOutcome::Exit { message }) = run(&["--help"]) else {
            panic!("--help should exit");
        };
        for name in ["start", "stop"] {
            assert!(message.contains(&format!("  {name} ")), "{name}");
        }
    }

    #[test]
    fn record_errors_name_the_problem_and_stay_in_the_terminal() {
        let cases: Vec<(Vec<&str>, &str)> = vec![
            (
                vec!["record", "--from", "09:00", "--to", "10:00"],
                "--project",
            ),
            (vec!["record", "--project", "ac", "--to", "10:00"], "--from"),
            (vec!["record", "--project", "ac", "--from", "09:00"], "--to"),
            (
                vec![
                    "record",
                    "--project",
                    "ac",
                    "--from",
                    "nonsense",
                    "--to",
                    "10:00",
                ],
                "Not a time: `nonsense`",
            ),
            (
                vec![
                    "record",
                    "--project",
                    "ac",
                    "--from",
                    "09:00",
                    "--to",
                    "later",
                ],
                "Not a time: `later`",
            ),
        ];
        for (args, expected) in cases {
            let err = run(&args).unwrap_err();
            assert!(!err.in_app, "{args:?}");
            assert!(err.message.contains(expected), "{args:?}: {}", err.message);
        }
    }

    fn report_request(
        range: crate::reports::Range,
        group_by: Group,
        rounding: Rounding,
    ) -> crate::reports::cli::ReportRequest {
        crate::reports::cli::ReportRequest {
            range,
            group_by,
            rounding,
            clients: vec![],
            projects: vec![],
            tags: vec![],
        }
    }

    #[test]
    fn report_without_arguments_reports_today_by_project_unrounded() {
        use crate::reports::Range;
        assert_eq!(
            remote(&["report"]),
            (
                IpcMessage::Report {
                    request: report_request(Range::Today, Group::Project, Rounding::None)
                },
                false,
                false
            )
        );
    }

    #[test]
    fn report_range_words_and_days_become_ranges() {
        use crate::reports::Range;
        let cases = [
            (vec!["report", "today"], Range::Today),
            (vec!["report", "yesterday"], Range::Yesterday),
            (vec!["report", "week"], Range::ThisWeek),
            (vec!["report", "last-week"], Range::LastWeek),
            (vec!["report", "month"], Range::ThisMonth),
            (vec!["report", "last-month"], Range::LastMonth),
            (
                vec!["report", "2026-10-01"],
                Range::Custom {
                    from: day("2026-10-01"),
                    to: day("2026-10-01"),
                },
            ),
            (
                vec!["report", "2026-10-01", "2026-10-07"],
                Range::Custom {
                    from: day("2026-10-01"),
                    to: day("2026-10-07"),
                },
            ),
            (
                vec!["report", "2026-10-07", "2026-10-07"],
                Range::Custom {
                    from: day("2026-10-07"),
                    to: day("2026-10-07"),
                },
            ),
        ];
        for (args, range) in cases {
            assert_eq!(
                remote(&args).0,
                IpcMessage::Report {
                    request: report_request(range, Group::Project, Rounding::None)
                },
                "{args:?}"
            );
        }
    }

    #[test]
    fn report_flags_set_grouping_rounding_and_json() {
        use crate::reports::Range;
        let (message, json, starts_app) = remote(&[
            "report",
            "last-week",
            "--by",
            "tag",
            "--round",
            "15",
            "--up",
            "--json",
        ]);
        assert_eq!(
            message,
            IpcMessage::Report {
                request: report_request(Range::LastWeek, Group::Tag, Rounding::Up15)
            }
        );
        assert!(json);
        assert!(!starts_app);
    }

    #[test]
    fn report_round_without_up_rounds_to_the_nearest_step() {
        for (minutes, rounding) in [
            ("5", Rounding::Nearest5),
            ("6", Rounding::Nearest6),
            ("10", Rounding::Nearest10),
            ("15", Rounding::Nearest15),
            ("30", Rounding::Nearest30),
        ] {
            let IpcMessage::Report { request } = remote(&["report", "--round", minutes]).0 else {
                panic!("not a report");
            };
            assert_eq!(request.rounding, rounding, "{minutes}");
        }
    }

    #[test]
    fn report_by_takes_every_grouping() {
        for (word, group) in [
            ("project", Group::Project),
            ("tag", Group::Tag),
            ("day", Group::Day),
            ("week", Group::Week),
            ("month", Group::Month),
        ] {
            let IpcMessage::Report { request } = remote(&["report", "--by", word]).0 else {
                panic!("not a report");
            };
            assert_eq!(request.group_by, group, "{word}");
        }
    }

    #[test]
    fn report_filters_can_be_repeated() {
        let IpcMessage::Report { request } = remote(&[
            "report",
            "--client",
            "acme",
            "--client",
            "globex",
            "--project",
            "portal",
            "--tag",
            "admin",
            "--tag",
            "support",
        ])
        .0
        else {
            panic!("not a report");
        };
        assert_eq!(request.clients, ["acme", "globex"]);
        assert_eq!(request.projects, ["portal"]);
        assert_eq!(request.tags, ["admin", "support"]);
    }

    #[test]
    fn report_errors_name_the_problem_and_stay_in_the_terminal() {
        let cases: Vec<(Vec<&str>, &str)> = vec![
            (vec!["report", "someday"], "Not a range: `someday`"),
            (
                vec!["report", "2026-10-01", "someday"],
                "Not a day: `someday`",
            ),
            (
                vec!["report", "2026-10-07", "2026-10-01"],
                "The range starts after it ends",
            ),
            (vec!["report", "--up"], "--round"),
            (vec!["report", "--round", "7"], "7"),
            (vec!["report", "--by", "year"], "year"),
            (vec!["report", "today", "yesterday", "week"], "week"),
        ];
        for (args, expected) in cases {
            let err = run(&args).unwrap_err();
            assert!(!err.in_app, "{args:?}");
            assert!(err.message.contains(expected), "{args:?}: {}", err.message);
        }
    }

    #[test]
    fn plain_messages_are_not_split() {
        assert_eq!(split(IpcMessage::Status), Ok(vec![IpcMessage::Status]));
        let batch = IpcMessage::ActivityAdd {
            items: vec![input("A")],
        };
        assert_eq!(split(batch.clone()), Ok(vec![batch]));
    }

    fn batch_items(requests: &[IpcMessage]) -> Vec<ActivityInput> {
        requests
            .iter()
            .flat_map(|request| match request {
                IpcMessage::ActivityAdd { items } => items.clone(),
                other => panic!("not a batch: {other:?}"),
            })
            .collect()
    }

    #[test]
    fn a_large_batch_is_cut_into_requests_of_at_most_200_items() {
        let items: Vec<ActivityInput> = (0..450).map(|i| input(&format!("item {i}"))).collect();
        let requests = split(IpcMessage::ActivityAdd {
            items: items.clone(),
        })
        .unwrap();
        let sizes: Vec<usize> = requests
            .iter()
            .map(|r| batch_items(&[r.clone()]).len())
            .collect();
        assert_eq!(sizes, [200, 200, 50]);
        assert_eq!(batch_items(&requests), items);
    }

    #[test]
    fn requests_of_large_items_stay_within_the_line_limit() {
        let items: Vec<ActivityInput> = (0..1000)
            .map(|i| {
                let mut item = input(&format!("item {i}"));
                item.url = Some("u".repeat(2000));
                item
            })
            .collect();
        let requests = split(IpcMessage::ActivityAdd {
            items: items.clone(),
        })
        .unwrap();
        assert!(requests.len() > 1);
        for request in &requests {
            let line = serde_json::to_string(&IpcRequest {
                id: "0123456789abcdef0123456789abcdef0123".into(),
                message: request.clone(),
            })
            .unwrap();
            assert!(line.len() < MAX_MESSAGE_BYTES as usize, "{}", line.len());
        }
        assert_eq!(batch_items(&requests), items);
    }

    #[test]
    fn an_item_too_large_to_send_fails_naming_its_index() {
        let mut huge = input("huge");
        huge.url = Some("u".repeat(MAX_MESSAGE_BYTES as usize));
        let result = split(IpcMessage::ActivityAdd {
            items: vec![input("small"), huge],
        });
        assert_eq!(result, Err("items[1] is too large to send".to_string()));
    }
}
