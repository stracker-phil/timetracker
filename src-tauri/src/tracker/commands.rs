use std::sync::Arc;

use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use super::{PauseUntil, Tracker, TrackerStatus};
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
