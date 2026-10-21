//! Launch at login: a LaunchAgent written by `tauri-plugin-autostart` that starts the app with
//! `--hidden`. The LaunchAgent is the truth; `settings.launchAtLogin` mirrors it and is applied
//! again on every start (`commands::apply_settings`).

use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

use crate::commands;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Whether the LaunchAgent is installed.
pub fn status(app: &AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// Installs or removes the LaunchAgent unless it already matches.
pub fn apply(app: &AppHandle, enabled: bool) -> AppResult<()> {
    if status(app) == enabled {
        return Ok(());
    }
    let autolaunch = app.autolaunch();
    let result = if enabled {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    };
    result.map_err(|e| {
        AppError::Io(format!(
            "Couldn't turn launch at login {}: {e}",
            if enabled { "on" } else { "off" }
        ))
    })
}

/// Applies the choice, then stores it in the settings; returns whether it is on now.
pub async fn set(app: &AppHandle, state: &AppState, enabled: bool) -> AppResult<bool> {
    apply(app, enabled)?;
    let mut settings = state.settings.get();
    settings.launch_at_login = enabled;
    commands::save_settings(app, state, settings).await?;
    Ok(status(app))
}

#[tauri::command]
pub async fn autostart_set(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> AppResult<bool> {
    set(&app, &state, enabled).await
}

#[tauri::command]
pub fn autostart_status(app: AppHandle) -> bool {
    status(&app)
}
