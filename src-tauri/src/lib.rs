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
pub mod watch;
pub mod window;

use std::path::PathBuf;

use crate::cli::{CliError, CliOutcome};
use crate::ipc::IpcMessage;
use crate::state::AppState;
use crate::store::json_file::create_private_dir;

const IDENTIFIER: &str = "com.philippstracker.timetracker";
/// `tauri.debug.conf.json`'s identifier, so macOS lists and grants the debug build apart.
const DEBUG_IDENTIFIER: &str = "com.philippstracker.timetracker.debug";
/// Names the config dir. Each build has its own settings, data, socket and lock, so a debug build
/// starts next to a running release app instead of handing off to it.
const BUILD_IDENTIFIER: &str = if cfg!(debug_assertions) {
    DEBUG_IDENTIFIER
} else {
    IDENTIFIER
};
/// Debug builds go by their own name in the menu bar and the tray.
pub const APP_NAME: &str = if cfg!(debug_assertions) {
    "TimeTracker (Debug)"
} else {
    "TimeTracker"
};
const SOCKET_NAME: &str = "timetracker.sock";
/// Debug builds claim their own scheme (`tauri.debug.conf.json`), so links reach the installed app.
pub const LINK_SCHEME: &str = if cfg!(debug_assertions) {
    "timetracker-debug"
} else {
    "timetracker"
};

/// Same directory Tauri resolves as `app_config_dir`, but available before the app starts.
fn config_dir() -> PathBuf {
    let Some(home) = std::env::home_dir() else {
        fail("Cannot locate the home directory for TimeTracker's settings");
    };
    home.join("Library/Application Support")
        .join(BUILD_IDENTIFIER)
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
        Ok(CliOutcome::Watch { json }) => match instance::watch(&socket, json) {
            Some(code) => std::process::exit(code),
            None => fail("TimeTracker is not running"),
        },
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

    fn configs() -> (serde_json::Value, serde_json::Value) {
        (
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap(),
            serde_json::from_str(include_str!("../tauri.debug.conf.json")).unwrap(),
        )
    }

    fn this_build() -> serde_json::Value {
        let (release, debug) = configs();
        if cfg!(debug_assertions) {
            debug
        } else {
            release
        }
    }

    #[test]
    fn identifier_matches_the_tauri_config() {
        let (release, debug) = configs();
        assert_eq!(release["identifier"], serde_json::json!(IDENTIFIER));
        assert_eq!(debug["identifier"], serde_json::json!(DEBUG_IDENTIFIER));
    }

    #[test]
    fn app_name_matches_the_product_name_in_the_tauri_config() {
        assert_eq!(this_build()["productName"], serde_json::json!(APP_NAME));
    }

    #[test]
    fn link_scheme_matches_the_deep_link_scheme_in_the_tauri_config() {
        assert_eq!(
            this_build()["plugins"]["deep-link"]["desktop"]["schemes"],
            serde_json::json!([LINK_SCHEME])
        );
    }

    #[test]
    fn the_debug_build_differs_from_the_release_build_in_name_identifier_and_scheme() {
        let (release, debug) = configs();
        assert_eq!(release["productName"], serde_json::json!("TimeTracker"));
        assert_eq!(
            debug["productName"],
            serde_json::json!("TimeTracker (Debug)")
        );
        assert_eq!(debug["identifier"], serde_json::json!(DEBUG_IDENTIFIER));
        assert_eq!(DEBUG_IDENTIFIER, format!("{IDENTIFIER}.debug"));
        assert_eq!(
            release["plugins"]["deep-link"]["desktop"]["schemes"],
            serde_json::json!(["timetracker"])
        );
        assert_eq!(
            debug["plugins"]["deep-link"]["desktop"]["schemes"],
            serde_json::json!(["timetracker-debug"])
        );
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
