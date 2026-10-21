use tauri::{AppHandle, Manager, State};

use super::undo::UndoStatus;
use super::{Change, Overlap, RecordingInput, Recordings, Span, snap_span};
use crate::db::recordings::{self, Recording, RecordingPatch};
use crate::db::time::{Day, Ms};
use crate::error::AppResult;
use crate::events::{self, DataKind};
use crate::state::AppState;

/// Runs `write` with the service, then tells the page about every day it changed.
pub async fn changing<T, F>(app: &AppHandle, write: F) -> AppResult<T>
where
    F: AsyncFnOnce(&Recordings<'_>) -> AppResult<Change<T>>,
{
    let state = app.state::<AppState>();
    let db = state.data.db()?;
    let service = Recordings {
        db: &db,
        undo: &state.undo,
        snap_minutes: snap_minutes(&state),
    };
    let change = write(&service).await?;
    for day in change.days {
        events::data_changed(app, DataKind::Recordings, Some(day));
    }
    Ok(change.value)
}

#[tauri::command]
pub async fn recording_create(
    app: AppHandle,
    input: RecordingInput,
    overlap: Overlap,
) -> AppResult<Recording> {
    changing(&app, async |s| s.create(input, overlap).await).await
}

#[tauri::command]
pub async fn recording_update(
    app: AppHandle,
    id: String,
    patch: RecordingPatch,
    overlap: Overlap,
) -> AppResult<Recording> {
    changing(&app, async |s| s.update(&id, patch, overlap).await).await
}

/// Moves the recording to `day` at the same local time; it keeps its length.
#[tauri::command]
pub async fn recording_move(
    app: AppHandle,
    id: String,
    day: Day,
    overlap: Overlap,
) -> AppResult<Recording> {
    changing(&app, async |s| s.move_to_day(&id, day, overlap).await).await
}

/// A copy of the recording on `day` at the same local time.
#[tauri::command]
pub async fn recording_copy(
    app: AppHandle,
    id: String,
    day: Day,
    overlap: Overlap,
) -> AppResult<Recording> {
    changing(&app, async |s| s.copy_to_day(&id, day, overlap).await).await
}

#[tauri::command]
pub async fn recording_delete(app: AppHandle, id: String) -> AppResult<()> {
    changing(&app, async |s| s.delete(&id).await).await
}

/// The label of the undone change; `None` when there was none.
#[tauri::command]
pub async fn undo(app: AppHandle) -> AppResult<Option<String>> {
    changing(&app, async |s| s.undo().await).await
}

#[tauri::command]
pub async fn redo(app: AppHandle) -> AppResult<Option<String>> {
    changing(&app, async |s| s.redo().await).await
}

#[tauri::command]
pub async fn undo_status(state: State<'_, AppState>) -> AppResult<UndoStatus> {
    Ok(state.undo.lock().await.status())
}

#[tauri::command]
pub async fn recordings_list(
    state: State<'_, AppState>,
    from: Ms,
    to: Ms,
) -> AppResult<Vec<Recording>> {
    recordings::list_for_range(&state.data.db()?, from, to).await
}

/// The span as a write would store it, for the page's drag ghost.
#[tauri::command]
pub fn recording_snap(state: State<'_, AppState>, start: Ms, end: Ms) -> Span {
    snap_span(start, end, snap_minutes(&state))
}

fn snap_minutes(state: &AppState) -> u32 {
    u32::from(state.settings.get().timeline.snap_minutes)
}
