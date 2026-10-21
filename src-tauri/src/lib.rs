pub mod cli;
pub mod commands;
pub mod db;
mod deep_link;
#[cfg(debug_assertions)]
mod dev;
pub mod error;
pub mod events;
pub mod ipc;
pub mod launch;
pub mod lock;
pub mod menu;
pub mod pending;
pub mod remote;
pub mod state;
pub mod store;
#[cfg(any(test, feature = "test-util"))]
pub mod test_support;
pub mod tray;
pub mod window;

use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

use crate::cli::CliOutcome;
use crate::ipc::{Forward, IpcMessage};
use crate::state::AppState;
use crate::store::json_file::create_private_dir;
use crate::store::settings::{LogLevel, Settings};

const IDENTIFIER: &str = "com.philippstracker.timetracker";
const SOCKET_NAME: &str = "timetracker.sock";
const LOCK_NAME: &str = "timetracker.lock";
const MAX_LOG_BYTES: u128 = 5 * 1024 * 1024;
const LOCK_POLL: Duration = Duration::from_millis(200);
const LOCK_POLLS: u32 = 25;

/// Same directory Tauri resolves as `app_config_dir`, but available before the app starts.
fn config_dir() -> PathBuf {
    let Some(home) = std::env::home_dir() else {
        fail("Cannot locate the home directory for TimeTracker's settings");
    };
    home.join("Library/Application Support").join(IDENTIFIER)
}

/// Held for the app's lifetime, so two launches that both find no listener can't both start.
/// `None` when another process holds it.
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

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
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

fn log_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_log::Builder::new()
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir { file_name: None }),
        ])
        .rotation_strategy(RotationStrategy::KeepSome(3))
        .max_file_size(MAX_LOG_BYTES)
        .timezone_strategy(TimezoneStrategy::UseLocal)
        // The dispatch caps every later `log::set_max_level`, so it lets everything through;
        // `init_logging` sets the effective level.
        .level(log::LevelFilter::Trace)
        .level_for("tao", log::LevelFilter::Warn)
        .level_for("wry", log::LevelFilter::Warn)
        .level_for("tauri_runtime_wry", log::LevelFilter::Info)
        .build()
}

/// Runs after `log_plugin`'s setup, which raises the max level to `Trace`, and applies the effective level.
fn init_logging(level: LogLevel) -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("log-level")
        .setup(move |_, _| {
            log::set_max_level(level.into());
            Ok(())
        })
        .build()
}

#[cfg(debug_assertions)]
fn apply_dev_frame(app: &tauri::App) {
    let (Some((x, y, w, h)), Some(window)) = (dev::window_frame(), app.get_webview_window("main"))
    else {
        return;
    };
    let _ = window.set_position(tauri::LogicalPosition::new(x, y));
    let _ = window.set_size(tauri::LogicalSize::new(w, h));
}

/// Without the socket the app still runs, only without single-instance handoff.
fn start_ipc(handle: AppHandle, socket: &Path) {
    let listener = match ipc::bind(socket) {
        Ok(listener) => listener,
        Err(e) => {
            log::warn!("Cannot listen on {}: {e}", socket.display());
            return;
        }
    };
    tauri::async_runtime::spawn(ipc::serve(listener, move |message| {
        launch::on_ipc_message(handle.clone(), message)
    }));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_dir = config_dir();
    if let Err(e) = create_private_dir(&config_dir) {
        eprintln!("Cannot create {}: {e}", config_dir.display());
    }
    let socket = config_dir.join(SOCKET_NAME);
    let stored = Settings::peek(&config_dir);

    let (message, log_override, hidden) = match cli::parse(std::env::args_os()) {
        Ok(CliOutcome::Exit { message }) => {
            print!("{message}");
            std::process::exit(0);
        }
        Ok(CliOutcome::Run { hidden, log_level }) => {
            (IpcMessage::Activate { hidden }, log_level, hidden)
        }
        Err(message) => {
            eprintln!("{message}");
            (IpcMessage::Error { message }, None, false)
        }
    };
    let exit_code = if matches!(message, IpcMessage::Error { .. }) {
        2
    } else {
        0
    };

    forward(&socket, &message, exit_code);
    let _instance_lock = match lock_instance(&config_dir) {
        Ok(Some(lock)) => Some(lock),
        Ok(None) => forward_to_starting_instance(&socket, &message, exit_code),
        Err(e) => {
            eprintln!("Cannot lock the TimeTracker instance: {e}");
            None
        }
    };

    let mut state = AppState::open(&config_dir);
    state.log_override = log_override;
    start_app(
        state,
        socket,
        message,
        log_override.unwrap_or(stored.log_level),
        hidden,
    );
}

fn start_app(
    state: AppState,
    socket: PathBuf,
    initial: IpcMessage,
    log_level: LogLevel,
    hidden: bool,
) {
    tauri::Builder::default()
        .manage(state)
        .manage(remote::Bridge::default())
        .plugin(log_plugin())
        .plugin(init_logging(log_level))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .on_menu_event(|app, event| menu::forward(app, event.id().as_ref()))
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event
                && window.label() == "main"
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(move |app| {
            log::info!("TimeTracker {} started", env!("CARGO_PKG_VERSION"));
            let state = app.state::<AppState>();
            app.set_menu(menu::build(app, &state.keybindings)?)?;
            state.launches.push(initial);
            commands::apply_settings(app.handle(), None, &state.settings.get());
            tray::install(app.handle())?;
            #[cfg(debug_assertions)]
            apply_dev_frame(app);
            start_ipc(app.handle().clone(), &socket);
            deep_link::listen(app.handle());
            if hidden {
                window::hide_main(app.handle());
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::get_keybindings,
            commands::set_keybindings,
            commands::get_ui_state,
            commands::set_ui_state,
            commands::open_log_folder,
            commands::open_url,
            commands::log_frontend_error,
            commands::take_launch_events,
            commands::hide_window,
            commands::quit_app,
            commands::choose_save_path,
            commands::reveal_path,
            db::commands::data_info,
            db::commands::backup_export,
            remote::remote_reply,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            RunEvent::ExitRequested {
                code: None, api, ..
            } => api.prevent_exit(),
            RunEvent::Exit => {
                tauri::async_runtime::block_on(app.state::<AppState>().data.close());
            }
            _ => {}
        });
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
