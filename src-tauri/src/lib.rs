pub mod activity;
mod app;
pub mod app_icons;
pub mod autostart;
pub mod away_prompt;
pub mod calendar;
pub mod capture;
pub mod cli;
pub mod commands;
pub mod day;
pub mod db;
pub mod deep_link;
#[cfg(debug_assertions)]
mod dev;
pub mod error;
pub mod events;
mod handlers;
pub mod help;
mod instance;
pub mod ipc;
pub mod launch;
pub mod lock;
pub mod mcp;
pub mod menu;
pub mod pending;
pub mod projects;
pub mod recordings;
pub mod remote;
pub mod reports;
pub mod rules;
pub mod scripting;
pub mod state;
pub mod store;
#[cfg(any(test, feature = "test-util", debug_assertions))]
pub mod test_support;
pub mod timers;
pub mod tracker;
pub mod tray;
pub mod window;

use std::path::PathBuf;

use crate::cli::{CliError, CliOutcome};
use crate::ipc::IpcMessage;
use crate::state::AppState;
use crate::store::json_file::create_private_dir;

const IDENTIFIER: &str = "com.philippstracker.timetracker";
const SOCKET_NAME: &str = "timetracker.sock";

/// Same directory Tauri resolves as `app_config_dir`, but available before the app starts.
fn config_dir() -> PathBuf {
    let Some(home) = std::env::home_dir() else {
        fail("Cannot locate the home directory for TimeTracker's settings");
    };
    home.join("Library/Application Support").join(IDENTIFIER)
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_dir = config_dir();
    if let Err(e) = create_private_dir(&config_dir) {
        eprintln!("Cannot create {}: {e}", config_dir.display());
    }
    let socket = config_dir.join(SOCKET_NAME);

    let outcome = cli::parse(
        std::env::args_os(),
        db::time::now_ms(),
        &chrono::Local,
        &mut std::io::stdin(),
    );
    let (message, log_level, hidden) = match outcome {
        Ok(CliOutcome::Exit { message }) => {
            print!("{message}");
            std::process::exit(0);
        }
        Ok(CliOutcome::Mcp) => std::process::exit(mcp::run(&config_dir, socket)),
        Ok(CliOutcome::Run { hidden, log_level }) => {
            (IpcMessage::Activate { hidden }, log_level, hidden)
        }
        Ok(CliOutcome::Remote {
            message,
            json,
            starts_app,
        }) => {
            if let Some(code) = instance::ask(&socket, &message, json) {
                std::process::exit(code);
            }
            if !starts_app {
                fail("TimeTracker is not running");
            }
            (message, None, false)
        }
        Err(CliError {
            message,
            in_app: false,
        }) => fail(&message),
        Err(CliError { message, .. }) => {
            eprintln!("{message}");
            (IpcMessage::Error { message }, None, false)
        }
    };
    let exit_code = if matches!(message, IpcMessage::Error { .. }) {
        2
    } else {
        0
    };

    let _instance_lock = instance::forward_or_lock(&config_dir, &socket, &message, exit_code);

    app::start(
        AppState::open(&config_dir),
        socket,
        message,
        log_level.unwrap_or_default(),
        hidden,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    const NETWORK_CRATES: &[&str] = &["reqwest", "hyper", "ureq", "tauri-plugin-updater"];

    #[test]
    fn identifier_matches_the_tauri_config() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["identifier"], serde_json::json!(IDENTIFIER));
    }

    #[test]
    fn no_network_crate_is_compiled_for_macos() {
        let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
        let output = Command::new(env!("CARGO"))
            .args(["tree", "--offline", "--manifest-path", manifest])
            .args([
                "-e",
                "normal",
                "--target",
                "aarch64-apple-darwin",
                "--prefix",
                "none",
            ])
            .output()
            .expect("run cargo tree");
        assert!(
            output.status.success(),
            "cargo tree failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let tree = String::from_utf8(output.stdout).unwrap();
        for line in tree.lines() {
            let name = line.split_whitespace().next().unwrap_or_default();
            assert!(
                !NETWORK_CRATES.contains(&name),
                "network crate `{name}` is compiled for macOS (ADR 0013): {line}"
            );
        }
    }

    #[test]
    fn cargo_manifest_names_no_network_crate() {
        let manifest = include_str!("../Cargo.toml");
        for line in manifest.lines() {
            let name = line.split(['=', ' ']).next().unwrap_or_default().trim();
            assert!(
                !NETWORK_CRATES.contains(&name),
                "Cargo.toml depends on `{name}` (ADR 0013)"
            );
        }
    }
}
