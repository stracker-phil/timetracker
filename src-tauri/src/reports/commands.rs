use tauri::State;

use super::saved::{self, SavedReport};
use super::{ReportFilter, ReportResult, run, text};
use crate::db::time::{Day, now_ms};
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub async fn report_run(
    state: State<'_, AppState>,
    filter: ReportFilter,
) -> AppResult<ReportResult> {
    run(&state.data.db()?, &filter, Day::today(), now_ms()).await
}

#[tauri::command]
pub async fn report_text(state: State<'_, AppState>, filter: ReportFilter) -> AppResult<String> {
    Ok(text(
        &run(&state.data.db()?, &filter, Day::today(), now_ms()).await?,
    ))
}

#[tauri::command]
pub async fn saved_reports_list(state: State<'_, AppState>) -> AppResult<Vec<SavedReport>> {
    saved::list(&state.data.db()?).await
}

#[tauri::command]
pub async fn saved_report_save(
    state: State<'_, AppState>,
    name: String,
    filter: ReportFilter,
) -> AppResult<SavedReport> {
    saved::save(&state.data.db()?, &name, &filter).await
}

#[tauri::command]
pub async fn saved_report_rename(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> AppResult<SavedReport> {
    saved::rename(&state.data.db()?, &id, &name).await
}

#[tauri::command]
pub async fn saved_report_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    saved::delete(&state.data.db()?, &id).await
}
