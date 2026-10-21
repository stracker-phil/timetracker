use tauri::AppHandle;

use super::ActivityInput;
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
