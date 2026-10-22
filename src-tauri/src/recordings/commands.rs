use tauri::{AppHandle, Manager, State};

use super::undo::UndoStatus;
use super::{Change, Overlap, RecordingInput, Recordings, Span, default_span, snap_span};
use crate::db::recordings::{self, Recording, RecordingPatch};
use crate::db::time::{Day, Ms, now_ms};
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

/// Moves the span `spanId`, else the whole recording, to `day` at the same local time; spans keep their
/// lengths.
#[tauri::command]
pub async fn recording_move(
    app: AppHandle,
    id: String,
    span_id: Option<String>,
    day: Day,
    overlap: Overlap,
) -> AppResult<Recording> {
    changing(&app, async |s| {
        s.move_to_day(&id, span_id.as_deref(), day, overlap).await
    })
    .await
}

/// A copy of the span `spanId`, else of the whole recording, on `day` at the same local time.
#[tauri::command]
pub async fn recording_copy(
    app: AppHandle,
    id: String,
    span_id: Option<String>,
    day: Day,
    overlap: Overlap,
) -> AppResult<Recording> {
    changing(&app, async |s| {
        s.copy_to_day(&id, span_id.as_deref(), day, overlap).await
    })
    .await
}

#[tauri::command]
pub async fn recording_add_span(
    app: AppHandle,
    id: String,
    span: Span,
    overlap: Overlap,
) -> AppResult<Recording> {
    changing(&app, async |s| s.add_span(&id, span, overlap).await).await
}

#[tauri::command]
pub async fn recording_update_span(
    app: AppHandle,
    id: String,
    span_id: String,
    span: Span,
    overlap: Overlap,
) -> AppResult<Recording> {
    changing(&app, async |s| {
        s.update_span(&id, &span_id, span, overlap).await
    })
    .await
}

/// Moves the span's start and end by whole snap steps each (the page's arrow keys).
#[tauri::command]
pub async fn recording_nudge(
    app: AppHandle,
    id: String,
    span_id: String,
    start_steps: i32,
    end_steps: i32,
    overlap: Overlap,
) -> AppResult<Recording> {
    changing(&app, async |s| {
        s.nudge_span(&id, &span_id, start_steps, end_steps, overlap)
            .await
    })
    .await
}

/// The recording without the span; `None` when it was the last one and the recording went with it.
#[tauri::command]
pub async fn recording_remove_span(
    app: AppHandle,
    id: String,
    span_id: String,
) -> AppResult<Option<Recording>> {
    changing(&app, async |s| s.remove_span(&id, &span_id).await).await
}

/// Cuts the span in two at `at`, snapped.
#[tauri::command]
pub async fn recording_split_span(
    app: AppHandle,
    id: String,
    span_id: String,
    at: Ms,
) -> AppResult<Recording> {
    changing(&app, async |s| s.split_span(&id, &span_id, at).await).await
}

/// The spans of `from` join `into`; `from` is deleted.
#[tauri::command]
pub async fn recording_merge(app: AppHandle, into: String, from: String) -> AppResult<Recording> {
    changing(&app, async |s| s.merge(&into, &from).await).await
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

/// The span File → New Recording opens the editor on when nothing is selected.
#[tauri::command]
pub fn recording_default_span(state: State<'_, AppState>) -> Span {
    default_span(now_ms(), snap_minutes(&state))
}

fn snap_minutes(state: &AppState) -> u32 {
    u32::from(state.settings.get().timeline.snap_minutes)
}
