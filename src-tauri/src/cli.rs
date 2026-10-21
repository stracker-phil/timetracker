use clap::Parser;

use crate::store::settings::LogLevel;

#[derive(Parser, Debug)]
#[command(
    name = "timetracker",
    version,
    about = "Local-first automatic time tracker",
    after_help = "Examples:\n  timetracker            start TimeTracker or bring it to the front\n  timetracker --hidden   start without opening the window (launch at login)"
)]
struct Cli {
    /// Start without showing the window; a running instance stays as it is
    #[arg(long)]
    hidden: bool,
    /// Log level for this run
    #[arg(long, value_enum)]
    log_level: Option<LogLevel>,
}

#[derive(Debug, PartialEq)]
pub enum CliOutcome {
    Run {
        hidden: bool,
        log_level: Option<LogLevel>,
    },
    /// `--help` / `--version`: print and exit without starting the app.
    Exit { message: String },
}

/// Parses the full argv (including the program name).
pub fn parse<I, S>(args: I) -> Result<CliOutcome, String>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    match Cli::try_parse_from(args) {
        Ok(cli) => Ok(CliOutcome::Run {
            hidden: cli.hidden,
            log_level: cli.log_level,
        }),
        Err(e) => match e.kind() {
            clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                Ok(CliOutcome::Exit {
                    message: e.to_string(),
                })
            }
            _ => Err(e.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(hidden: bool, log_level: Option<LogLevel>) -> Result<CliOutcome, String> {
        Ok(CliOutcome::Run { hidden, log_level })
    }

    #[test]
    fn no_arguments_run_with_the_window() {
        assert_eq!(parse(["timetracker"]), run(false, None));
    }

    #[test]
    fn hidden_flag_starts_without_a_window() {
        assert_eq!(parse(["timetracker", "--hidden"]), run(true, None));
    }

    #[test]
    fn log_level_flag_selects_the_level() {
        for (arg, level) in [
            ("error", LogLevel::Error),
            ("warn", LogLevel::Warn),
            ("info", LogLevel::Info),
            ("debug", LogLevel::Debug),
            ("trace", LogLevel::Trace),
        ] {
            assert_eq!(
                parse(["timetracker", "--log-level", arg]),
                run(false, Some(level)),
                "{arg}"
            );
        }
    }

    #[test]
    fn flags_combine() {
        assert_eq!(
            parse(["timetracker", "--hidden", "--log-level", "debug"]),
            run(true, Some(LogLevel::Debug))
        );
    }

    #[test]
    fn help_exits_with_the_usage_text() {
        let Ok(CliOutcome::Exit { message }) = parse(["timetracker", "--help"]) else {
            panic!("--help should exit");
        };
        assert!(message.contains("--hidden"), "{message}");
        assert!(message.contains("--log-level"), "{message}");
    }

    #[test]
    fn version_exits_with_the_package_version() {
        let Ok(CliOutcome::Exit { message }) = parse(["timetracker", "--version"]) else {
            panic!("--version should exit");
        };
        assert_eq!(
            message.trim(),
            format!("timetracker {}", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn unknown_flags_and_log_levels_are_errors() {
        for args in [
            vec!["timetracker", "--nope"],
            vec!["timetracker", "--log-level", "loud"],
            vec!["timetracker", "--log-level"],
            vec!["timetracker", "stray"],
        ] {
            assert!(parse(args.clone()).is_err(), "{args:?}");
        }
    }
}
