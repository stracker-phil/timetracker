use std::sync::Arc;

use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use super::{PauseUntil, Tracker, TrackerState, TrackerStatus};
use crate::bus::{Command, CommandBus};
use crate::capture::PermissionStatus;
use crate::db::time::now_ms;
use crate::error::{AppError, AppResult};

const PRIVACY_SETTINGS: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

/// The tracker runs only with a data store to write to.
fn tracker(app: &AppHandle) -> AppResult<Arc<Tracker>> {
    app.try_state::<Arc<Tracker>>()
        .map(|tracker| Arc::clone(&tracker))
        .ok_or_else(|| AppError::Db {
            message: "The data store is not available".into(),
            code: None,
        })
}

/// The pause lengths `tracker:pause:<argument>` takes.
pub const PAUSES: [(&str, PauseUntil); 4] = [
    ("15", PauseUntil::Minutes(15)),
    ("60", PauseUntil::Minutes(60)),
    ("tomorrow", PauseUntil::Tomorrow),
    ("resumed", PauseUntil::Resumed),
];

fn paused(app: &AppHandle) -> Option<bool> {
    let tracker = app.try_state::<Arc<Tracker>>()?;
    Some(matches!(
        tracker.status().state,
        TrackerState::Paused { .. }
    ))
}

/// Pausing works while the tracker runs, resuming while it is paused; without a data store neither.
pub fn register(bus: &mut CommandBus<AppHandle>) {
    bus.add_family(
        Command::new("tracker:pause:", "Pause Tracking")
            .help("Pause activity tracking for 15 or 60 minutes, until tomorrow or until resumed")
            .family("<15|60|tomorrow|resumed>")
            .enabled_for(|app, _| paused(app) == Some(false))
            .run(|app, length| async move {
                let until = PAUSES
                    .iter()
                    .find(|(name, _)| *name == length)
                    .map(|(_, until)| *until)
                    .ok_or_else(|| {
                        AppError::InvalidInput(format!(
                            "Pause for 15, 60, tomorrow or resumed, not “{length}”."
                        ))
                    })?;
                tracker(&app)?.pause(until, now_ms()).await.map(drop)
            }),
    );
    bus.add(
        Command::new("tracker:resume", "Resume Tracking")
            .help("Resume activity tracking")
            .enabled_when(|app| paused(app) == Some(true))
            .run(|app| async move { tracker(&app)?.resume(now_ms()).await.map(drop) }),
    );
}

#[tauri::command]
pub fn tracker_status(app: AppHandle) -> AppResult<TrackerStatus> {
    Ok(tracker(&app)?.status())
}

#[tauri::command]
pub async fn tracker_pause(app: AppHandle, until: PauseUntil) -> AppResult<TrackerStatus> {
    tracker(&app)?.pause(until, now_ms()).await
}

#[tauri::command]
pub async fn tracker_resume(app: AppHandle) -> AppResult<TrackerStatus> {
    tracker(&app)?.resume(now_ms()).await
}

#[tauri::command]
pub async fn request_permission(app: AppHandle) -> AppResult<PermissionStatus> {
    tracker(&app)?.request_permission(now_ms()).await
}

#[tauri::command]
pub fn open_privacy_settings(app: AppHandle) -> AppResult<()> {
    app.opener()
        .open_url(PRIVACY_SETTINGS, None::<&str>)
        .map_err(|e| AppError::Io(e.to_string()))
}
