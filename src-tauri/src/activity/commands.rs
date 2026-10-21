use tauri::AppHandle;

use super::{ActivityCount, ActivityInput, DeleteResult};
use crate::db::activities::ActivityRow;
use crate::db::time::Ms;
use crate::error::AppResult;

#[tauri::command]
pub async fn activity_add(app: AppHandle, items: Vec<ActivityInput>) -> AppResult<Vec<i64>> {
    super::add(&app, &items).await
}

#[tauri::command]
pub async fn activity_start(app: AppHandle, input: ActivityInput) -> AppResult<ActivityRow> {
    super::start(&app, &input).await
}

#[tauri::command]
pub async fn activity_stop(
    app: AppHandle,
    source: String,
    end: Option<Ms>,
) -> AppResult<Option<ActivityRow>> {
    super::stop(&app, &source, end).await
}

#[tauri::command]
pub async fn activity_sources(app: AppHandle) -> AppResult<Vec<String>> {
    super::sources(&app).await
}

#[tauri::command]
pub async fn activity_count(app: AppHandle, from: Ms, to: Ms) -> AppResult<ActivityCount> {
    super::count(&app, from, to).await
}

#[tauri::command]
pub async fn activity_delete(app: AppHandle, from: Ms, to: Ms) -> AppResult<DeleteResult> {
    super::delete_range(&app, from, to).await
}

#[tauri::command]
pub async fn activity_delete_undo(app: AppHandle) -> AppResult<DeleteResult> {
    super::undo_delete(&app).await
}
