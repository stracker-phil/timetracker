//! The `timetracker` command line. Without a subcommand it starts the app (or activates the running
//! one); every subcommand becomes one `IpcMessage` that the running app answers (`remote::handle`).
//! A new subcommand adds a `Command` variant, its arm in `message`, a parser test, its arm in
//! `remote::handle` and, when it prints something, its arm in `reply::render`. `mcp` and `watch` are no
//! single request: they are outcomes of their own.

mod activity;
mod batch;
mod record;
pub mod reply;
pub mod report;
mod scripting;
#[cfg(test)]
mod testing;
pub mod time;
pub mod watch;
mod written;

use std::ffi::OsString;
use std::io::Read;

use chrono::TimeZone;
use clap::{CommandFactory, Parser, Subcommand};

use crate::db::time::Ms;
use crate::ipc::IpcMessage;
use crate::timers::commands::TimerStartRequest;
use activity::ActivityCommand;
pub use batch::split;
use record::RecordArgs;
use report::ReportArgs;
use scripting::{DeleteArgs, EditArgs, ListRange, TimerCommand, TrackerCommand};

const EXAMPLES: &str = "Examples:
  timetracker                          start TimeTracker or bring it to the front
  timetracker --hidden                 start without opening the window (launch at login)
  timetracker show yesterday           show yesterday's Day view
  timetracker status --json            the tracker state as JSON
  timetracker tracker pause 1h         pause activity tracking for an hour
  timetracker activity delete today    delete today's activity (undo within 30 s: activity undo)
  timetracker timer stop               stop the running timer
  timetracker timer start acme --note \"weekly call\" --tag \"external meeting\"
  timetracker timer status --json      the running timer as flat fields
  timetracker timer recent --json      the recent timers, as the tray offers them
  timetracker watch --json             one line per change of the timer, the tracker or the data
  timetracker activity add --source planner --title \"Weekly sync\" --start 10:00 --end 10:45
  timetracker activity start --source planner --title Break
  timetracker activity stop --source planner
  timetracker record --project acme --from 09:00 --to 09:30 --note standup --tag \"internal meeting\"
  timetracker record --project acme --span 14:25-15:00 --span 16:05-20:15 --final
  timetracker suggestions --json       today's suggestions with their ids
  timetracker approve <id>             record a suggestion as it is
  timetracker timer update --note \"ID-123: Payment bug\"
  timetracker projects --json          the projects with their clients and ids
  timetracker report last-week --by tag --round 15
  claude mcp add timetracker -- ~/.local/bin/timetracker mcp";

/// The `--log-level` of the CLI; without it the app logs at `Info`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl From<LogLevel> for log::LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Trace => log::LevelFilter::Trace,
        }
    }
}

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
    /// Same as `tracker pause`, for older scripts
    #[command(hide = true)]
    Pause {
        /// 15m, 1h (up to 24h), tomorrow (until midnight) or off (until resumed)
        until: String,
    },
    /// Same as `tracker resume`, for older scripts
    #[command(hide = true)]
    Resume,
    /// Write another app's activity (a planner, a meeting tool) into its own lane, or delete activity
    #[command(subcommand)]
    Activity(ActivityCommand),
    /// Suggest time for a project, to review in the Day view; --final records it
    Record(RecordArgs),
    /// Edit a suggestion; --final edits a recording, or records the suggestion with the changes
    Edit(EditArgs),
    /// Delete a suggestion (a rule's is dismissed); --final deletes a recording
    Delete(DeleteArgs),
    /// Record suggestions as they are
    Approve {
        /// Suggestion ids, from `timetracker suggestions`
        #[arg(required = true)]
        ids: Vec<String>,
    },
    /// List the suggestions to review, with their ids
    Suggestions(ListRange),
    /// List the recordings, with their ids
    Recordings(ListRange),
    /// List the projects, with their clients and ids
    Projects {
        /// Archived ones too
        #[arg(long)]
        archived: bool,
    },
    /// List the clients, with their ids
    Clients {
        /// Archived ones too
        #[arg(long)]
        archived: bool,
    },
    /// List the tags, with their ids
    Tags,
    /// Undo the last change to recordings or suggestions
    Undo,
    /// Redo the last undone change
    Redo,
    /// Print the hours and notes of a day or range, ready to paste into a timesheet
    Report(ReportArgs),
    /// Same as `timer start`, for older scripts
    #[command(hide = true)]
    Start {
        /// The project's id, name or a unique start of its name
        project: String,
        /// One line
        #[arg(long, default_value = "")]
        note: String,
        /// A tag by name (at most one)
        #[arg(long)]
        tag: Option<String>,
    },
    /// Same as `timer stop`, for older scripts
    #[command(hide = true)]
    Stop,
    /// Start, stop, show or change the running timer
    #[command(subcommand)]
    Timer(TimerCommand),
    /// Pause, resume or show activity tracking
    #[command(subcommand)]
    Tracker(TrackerCommand),
    /// Serve an MCP server (reads work while the app is closed; writes make suggestions)
    Mcp,
    /// Print the timer and the tracker, then one line per change (timer, tracker, data) until TimeTracker quits
    Watch,
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
    /// `timetracker watch`: stream the running app's changes until it quits.
    Watch { json: bool },
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
    /// A plain launch with bad options also reports them in the app, which may have been opened
    /// from Finder; a subcommand's error, or a word that is not a subcommand, is for the terminal only.
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
                clap::error::ErrorKind::InvalidSubcommand => Err(CliError {
                    message: e.to_string(),
                    in_app: false,
                }),
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
    match command {
        Command::Mcp => return Ok(CliOutcome::Mcp),
        Command::Watch => return Ok(CliOutcome::Watch { json: cli.json }),
        _ => {}
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
        Command::Activity(command) => command.message(json, now, tz, stdin)?,
        Command::Record(args) => IpcMessage::Record {
            request: args.request(now, tz)?,
        },
        Command::Report(args) => args.message(now, tz)?,
        Command::Start { project, note, tag } => IpcMessage::TimerStart {
            request: TimerStartRequest { project, note, tag },
        },
        Command::Stop => IpcMessage::TimerStop,
        Command::Timer(command) => command.message(),
        Command::Tracker(command) => command.message()?,
        Command::Edit(args) => args.message(now, tz)?,
        Command::Delete(args) => args.message(),
        Command::Approve { ids } => IpcMessage::Approve { ids },
        Command::Suggestions(range) => range.suggestions(now, tz)?,
        Command::Recordings(range) => range.recordings(now, tz)?,
        Command::Projects { archived } => IpcMessage::Projects { archived },
        Command::Clients { archived } => IpcMessage::Clients { archived },
        Command::Tags => IpcMessage::Tags,
        Command::Undo => IpcMessage::Undo,
        Command::Redo => IpcMessage::Redo,
        #[cfg(debug_assertions)]
        Command::Dev(DevCommand::Seed { years, yes }) => IpcMessage::DevSeed { years, yes },
        Command::Mcp | Command::Watch => {
            unreachable!("`parse` returns `mcp` and `watch` before it builds a message")
        }
    })
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::cli::testing::*;
    use crate::tracker::PauseUntil;

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
    fn the_watch_subcommand_streams_without_starting_the_app() {
        assert_eq!(run(&["watch"]), Ok(CliOutcome::Watch { json: false }));
        assert_eq!(
            run(&["watch", "--json"]),
            Ok(CliOutcome::Watch { json: true })
        );
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
        let listed = listed_commands();
        for name in [
            "show",
            "focus",
            "hide",
            "quit",
            "status",
            "activity",
            "record",
            "edit",
            "delete",
            "approve",
            "suggestions",
            "recordings",
            "projects",
            "clients",
            "tags",
            "undo",
            "redo",
            "report",
            "timer",
            "tracker",
            "mcp",
            "watch",
        ] {
            assert!(listed.iter().any(|l| l == name), "{name}: {listed:?}");
        }
    }

    #[test]
    fn the_old_top_level_timer_and_tracker_commands_still_parse_but_are_not_in_the_help() {
        let listed = listed_commands();
        for name in ["start", "stop", "pause", "resume"] {
            assert!(!listed.iter().any(|l| l == name), "{name}: {listed:?}");
        }
        let start = IpcMessage::TimerStart {
            request: TimerStartRequest {
                project: "acme".into(),
                note: String::new(),
                tag: None,
            },
        };
        for (args, message) in [
            (vec!["start", "acme"], start),
            (vec!["stop"], IpcMessage::TimerStop),
            (vec!["resume"], IpcMessage::Resume),
            (
                vec!["pause", "15m"],
                IpcMessage::Pause {
                    until: PauseUntil::Minutes(15),
                },
            ),
        ] {
            assert_eq!(remote(&args).0, message, "{args:?}");
        }
    }

    /// The names in the `Commands:` section of the help.
    fn listed_commands() -> Vec<String> {
        let help = Cli::command().render_help().to_string();
        help.lines()
            .skip_while(|line| *line != "Commands:")
            .skip(1)
            .take_while(|line| line.is_empty() || line.starts_with(' '))
            .filter(|line| line.starts_with("  ") && !line.starts_with("   "))
            .filter_map(|line| line.split_whitespace().next())
            .map(String::from)
            .collect()
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
    fn a_mistyped_subcommand_is_reported_in_the_terminal_only() {
        for args in [&["stauts"][..], &["reprot"]] {
            let err = run(args).unwrap_err();
            assert!(!err.in_app, "{args:?}");
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

    #[test]
    fn start_builds_a_timer_request_with_note_and_tag() {
        let (message, json, starts_app) = remote(&["start", "acme", "--note", "x", "--tag", "a"]);

        assert_eq!(
            message,
            IpcMessage::TimerStart {
                request: TimerStartRequest {
                    project: "acme".into(),
                    note: "x".into(),
                    tag: Some("a".into()),
                }
            }
        );
        assert!(!json);
        assert!(!starts_app);
    }

    #[test]
    fn start_without_note_and_tag_sends_them_empty() {
        assert_eq!(
            remote(&["start", "acme", "--json"]),
            (
                IpcMessage::TimerStart {
                    request: TimerStartRequest {
                        project: "acme".into(),
                        note: String::new(),
                        tag: None,
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
    fn help_describes_the_mcp_subcommand() {
        let help = Cli::command().render_help().to_string();
        assert!(
            help.contains(
                "Serve an MCP server (reads work while the app is closed; writes make suggestions)"
            ),
            "{help}"
        );
    }

    #[test]
    fn example_descriptions_start_in_the_same_column() {
        let help = Cli::command().render_long_help().to_string();
        let columns: Vec<(usize, &str)> = help
            .lines()
            .skip_while(|line| *line != "Examples:")
            .skip(1)
            .filter_map(|line| {
                let command = line.trim_start();
                let indent = line.len() - command.len();
                let gap = command.find("  ")?;
                let spaces = command[gap..].len() - command[gap..].trim_start().len();
                Some((indent + gap + spaces, line))
            })
            .collect();
        assert!(columns.len() >= 6, "{help}");
        let (first, _) = columns[0];
        for (column, line) in &columns {
            assert_eq!(*column, first, "{line}");
        }
    }

    #[test]
    fn timer_and_tracker_help_list_their_subcommands() {
        for (parent, names) in [
            (
                "timer",
                ["start", "stop", "status", "update", "pick"].as_slice(),
            ),
            ("tracker", ["pause", "resume", "status"].as_slice()),
        ] {
            let Ok(CliOutcome::Exit { message }) = run(&[parent, "--help"]) else {
                panic!("{parent} --help should exit");
            };
            for name in names {
                assert!(message.contains(&format!("  {name} ")), "{parent} {name}");
            }
        }
    }
}
