//! The Tauri app: managed state, plugins, window and menu events, setup and the run loop.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

use crate::cli::LogLevel;
use crate::handlers::handlers;
use crate::ipc::IpcMessage;
use crate::state::AppState;
use crate::tracker::Tracker;
use crate::{
    app_icons, away_prompt, capture, commands, db, deep_link, ipc, menu, remote, timers, tracker,
    tray, watch, window,
};

const MAX_LOG_BYTES: u128 = 5 * 1024 * 1024;

pub fn start(
    state: AppState,
    socket: PathBuf,
    initial: IpcMessage,
    log_level: LogLevel,
    hidden: bool,
) {
    let icons = app_icons::AppIcons::new(
        db::default_data_dir(&state.config_dir)
            .join("cache")
            .join("icons"),
        app_icons::installed(),
    );
    tauri::Builder::default()
        .manage(state)
        .manage(Arc::new(icons))
        .manage(remote::Bridge::default())
        .manage(watch::Watchers::default())
        .manage(menu::MenuSync::default())
        .manage(away_prompt::AwayPromptState::default())
        .plugin(log_plugin())
        .plugin(init_logging(log_level))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_positioner::init())
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
            if let Err(e) = timers::shortcut::register(
                app.handle(),
                state.settings.get().timer_shortcut.as_deref(),
            ) {
                log::warn!("{e}");
            }
            timers::start_ticking(app.handle());
            start_tracker(app.handle(), &state);
            #[cfg(debug_assertions)]
            apply_dev_frame(app);
            start_ipc(app.handle().clone(), &socket);
            deep_link::listen(app.handle());
            // The window starts invisible (tauri.conf.json), so a hidden start never flashes it.
            if !hidden {
                window::show_main(app.handle());
            }
            Ok(())
        })
        .invoke_handler(handlers())
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            RunEvent::ExitRequested {
                code: None, api, ..
            } => api.prevent_exit(),
            RunEvent::Exit => {
                tauri::async_runtime::block_on(app.state::<AppState>().data.close());
            }
            RunEvent::Reopen { .. } => window::show_main(app),
            _ => {}
        });
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
    let (Some((x, y, w, h)), Some(window)) =
        (crate::dev::window_frame(), app.get_webview_window("main"))
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
    let watchers = handle.state::<watch::Watchers>().inner().clone();
    tauri::async_runtime::spawn(ipc::serve(listener, watchers, move |message| {
        remote::handle(handle.clone(), message)
    }));
}

/// Without a data store there is nothing to record into, so the tracker stays off.
fn start_tracker(app: &AppHandle, state: &AppState) {
    let Ok(db) = state.data.db() else {
        log::warn!("The tracker is off: the data store is not available");
        return;
    };
    let settings = state.settings.get();
    let tracker = Tracker::new(
        db,
        capture::new_source(app),
        settings.tracker,
        db::time::now_ms(),
    );
    tracker.set_capture(settings.capture);
    app.manage(Arc::clone(&tracker));
    away_prompt::watch(app, &tracker);
    tracker::start(app, tracker);
}
