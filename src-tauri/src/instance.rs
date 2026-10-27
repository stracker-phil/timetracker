//! A new process and the running TimeTracker: handing a launch over, taking the instance lock, asking
//! the running app on behalf of a subcommand, and watching it.

use std::fs::File;
use std::io::{BufRead, Write};
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use crate::ipc::{self, Forward, IpcError, IpcMessage};
use crate::watch::WatchEvent;
use crate::{cli, db, fail};

const LOCK_NAME: &str = "timetracker.lock";
const LOCK_POLL: Duration = Duration::from_millis(200);
const LOCK_POLLS: u32 = 25;

/// Exits when a running instance takes the message; otherwise returns the instance lock, held for the
/// app's lifetime so two launches that both find no listener can't both start. `None` when the lock
/// cannot be taken at all.
pub fn forward_or_lock(
    config_dir: &Path,
    socket: &Path,
    message: &IpcMessage,
    exit_code: i32,
) -> Option<File> {
    forward(socket, message, exit_code);
    match lock_instance(config_dir) {
        Ok(Some(lock)) => Some(lock),
        Ok(None) => forward_to_starting_instance(socket, message, exit_code),
        Err(e) => {
            eprintln!("Cannot lock the TimeTracker instance: {e}");
            None
        }
    }
}

/// `None` when another process holds the lock.
fn lock_instance(dir: &Path) -> std::io::Result<Option<File>> {
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(LOCK_NAME))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(e)) => Err(e),
    }
}

/// Exits when a running instance answers or refuses the message; returns when none is listening.
fn forward(socket: &Path, message: &IpcMessage, exit_code: i32) {
    match ipc::try_forward(socket, message) {
        Ok(Forward::Delivered) => std::process::exit(exit_code),
        Ok(Forward::NoInstance) => {}
        Ok(Forward::Rejected) => fail("The running TimeTracker did not accept the request"),
        Err(e) => fail(&format!("Could not reach the running TimeTracker: {e}")),
    }
}

/// The lock holder may still be starting up and not listen yet.
fn forward_to_starting_instance(socket: &Path, message: &IpcMessage, exit_code: i32) -> ! {
    for _ in 0..LOCK_POLLS {
        std::thread::sleep(LOCK_POLL);
        forward(socket, message, exit_code);
    }
    fail("Another TimeTracker is starting but does not answer")
}

/// Sends a subcommand's message to the running app and prints its answer; returns the exit code, or
/// `None` when nothing listens. A batch sent in parts stops at the first part that fails.
pub fn ask(socket: &Path, message: &IpcMessage, json: bool) -> Option<i32> {
    let mut results = Vec::new();
    for request in cli::split(message.clone()).unwrap_or_else(|e| fail(&e)) {
        let reply = match ipc::request(socket, request) {
            Ok(Some(reply)) => reply,
            Ok(None) if results.is_empty() => return None,
            Ok(None) => fail("TimeTracker quit before the batch was written"),
            Err(e) => fail(&format!("Could not reach the running TimeTracker: {e}")),
        };
        match reply.into_result() {
            Ok(result) => results.push(result),
            Err(error) => {
                print_error(&error, json, &results);
                return Some(1);
            }
        }
    }
    let result = match results.len() {
        1 => results.remove(0),
        _ => merge_ids(&results),
    };
    print_result(message, &result, json);
    Some(0)
}

/// `written` holds the replies of the parts of a batch that went through before the error.
fn print_error(error: &IpcError, json: bool, written: &[serde_json::Value]) {
    if json {
        println!("{}", serde_json::to_value(error).unwrap_or_default());
    } else {
        eprintln!("{}", error.message);
    }
    if !written.is_empty() {
        eprintln!("The parts before it were written: {}", merge_ids(written));
    }
}

fn print_result(message: &IpcMessage, result: &serde_json::Value, json: bool) {
    if json {
        if !result.is_null() {
            println!("{result}");
        }
        return;
    }
    match cli::reply::render(message, result, db::time::now_ms(), &chrono::Local) {
        Ok(Some(text)) => println!("{text}"),
        Ok(None) => {}
        Err(e) => fail(&e),
    }
}

/// Prints the running app's timer and tracker, then one line per change. Returns the exit code: 2 when
/// the app quits, 0 when stdout closes; `None` when nothing listens.
pub fn watch(socket: &Path, json: bool) -> Option<i32> {
    let (reply, lines) = match ipc::open_request(socket, IpcMessage::Watch) {
        Ok(Some(opened)) => opened,
        Ok(None) => return None,
        Err(e) => fail(&format!("Could not reach the running TimeTracker: {e}")),
    };
    let current = match reply.into_result() {
        Ok(current) => current,
        Err(error) => {
            print_error(&error, json, &[]);
            return Some(1);
        }
    };
    for event in current.as_array().into_iter().flatten() {
        if !print_event(event, json) {
            return Some(0);
        }
    }
    Some(follow(lines, json))
}

/// Prints every event line until the app quits (2) or stdout closes (0).
fn follow(mut lines: impl BufRead, json: bool) -> i32 {
    let mut line = String::new();
    loop {
        line.clear();
        match lines.read_line(&mut line) {
            Ok(0) => {
                eprintln!("TimeTracker quit");
                return 2;
            }
            Ok(_) => {
                let Ok(event) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if !print_event(&event, json) {
                    return 0;
                }
            }
            Err(e) => {
                eprintln!("Lost the connection to TimeTracker: {e}");
                return 2;
            }
        }
    }
}

/// False once stdout is closed. An event this build does not know is printed as JSON.
fn print_event(event: &Value, json: bool) -> bool {
    let known = if json {
        None
    } else {
        serde_json::from_value::<WatchEvent>(event.clone()).ok()
    };
    let text = match known {
        Some(event) => cli::watch::line(&event, db::time::now_ms(), &chrono::Local),
        None => event.to_string(),
    };
    writeln!(std::io::stdout(), "{text}").is_ok()
}

/// The `{ ids }` replies of a split batch as one.
fn merge_ids(results: &[serde_json::Value]) -> serde_json::Value {
    let ids: Vec<serde_json::Value> = results
        .iter()
        .filter_map(|result| result["ids"].as_array())
        .flatten()
        .cloned()
        .collect();
    serde_json::json!({ "ids": ids })
}
