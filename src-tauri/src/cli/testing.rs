//! Shared helpers of the command line's tests: argv parsed at a fixed time in Vienna.

use std::io::Cursor;

use chrono::DateTime;
use chrono_tz::Europe::Vienna;

use super::{CliError, CliOutcome, parse};
use crate::activity::ActivityInput;
use crate::db::time::{Day, Ms};
use crate::ipc::IpcMessage;

pub fn ms(rfc3339: &str) -> Ms {
    DateTime::parse_from_rfc3339(rfc3339)
        .unwrap()
        .timestamp_millis()
}

pub fn now() -> Ms {
    ms("2026-10-21T12:00:00+02:00")
}

pub fn run_with(args: &[&str], stdin: &str) -> Result<CliOutcome, CliError> {
    let argv = std::iter::once("timetracker").chain(args.iter().copied());
    parse(argv, now(), &Vienna, &mut Cursor::new(stdin.to_string()))
}

pub fn run(args: &[&str]) -> Result<CliOutcome, CliError> {
    run_with(args, "")
}

pub fn remote(args: &[&str]) -> (IpcMessage, bool, bool) {
    match run(args).unwrap() {
        CliOutcome::Remote {
            message,
            json,
            starts_app,
        } => (message, json, starts_app),
        other => panic!("not a remote outcome: {other:?}"),
    }
}

pub fn day(text: &str) -> Day {
    Day::parse(text).unwrap()
}

pub fn input(title: &str) -> ActivityInput {
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
