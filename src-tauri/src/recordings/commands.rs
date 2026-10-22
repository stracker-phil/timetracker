use chrono::Local;
use tauri::{AppHandle, Manager, State};

use super::draft::{self, DraftSpans, DraftToggle};
use super::proposal::{self, RecordingProposal};
use super::undo::UndoStatus;
use super::{
    Change, Overlap, RecordingInput, Recordings, SnapEdge, Span, default_span_on, snap_span,
};
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

/// Moves the recording's spans on `from`, else all of them, to `day` at the same local times; spans keep
/// their lengths.
#[tauri::command]
pub async fn recording_move(
    app: AppHandle,
    id: String,
    from: Option<Day>,
    day: Day,
    overlap: Overlap,
) -> AppResult<Recording> {
    changing(&app, async |s| s.move_to_day(&id, from, day, overlap).await).await
}

/// A copy of the recording's spans on `from`, else of all of them, on `day` at the same local times.
#[tauri::command]
pub async fn recording_copy(
    app: AppHandle,
    id: String,
    from: Option<Day>,
    day: Day,
    overlap: Overlap,
) -> AppResult<Recording> {
    changing(&app, async |s| s.copy_to_day(&id, from, day, overlap).await).await
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

/// New times for the span as a drag left them; with `moving`, only that end snaps and the other keeps its time.
#[tauri::command]
pub async fn recording_update_span(
    app: AppHandle,
    id: String,
    span_id: String,
    span: Span,
    moving: Option<SnapEdge>,
    overlap: Overlap,
) -> AppResult<Recording> {
    changing(&app, async |s| {
        s.drag_span(&id, &span_id, span, moving, overlap).await
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

/// The span on the snap grid, for a selection of activity blocks (a drag's selection is already on it).
#[tauri::command]
pub fn recording_snap(state: State<'_, AppState>, start: Ms, end: Ms) -> Span {
    snap_span(start, end, snap_minutes(&state))
}

/// The editor's spans with ends at or before their start rolled to the next day, and their sum.
#[tauri::command]
pub fn recording_draft_spans(spans: Vec<Span>) -> DraftSpans {
    draft::draft(&spans, &Local)
}

/// The editor's spans with the last one's end moved so they add up to `totalMs`.
#[tauri::command]
pub fn recording_draft_total(spans: Vec<Span>, total_ms: i64) -> DraftSpans {
    draft::with_total(&spans, total_ms, &Local)
}

/// The editor's spans with `span` (a snapped selection) in them, merged with those it touches or overlaps.
#[tauri::command]
pub fn recording_draft_include(spans: Vec<Span>, span: Span) -> DraftSpans {
    draft::with_included(&spans, span, &Local)
}

/// The editor's spans after a click on an activity block: its snapped span leaves them when they cover it,
/// else joins them; `others` are the other clicked blocks, whose time a removal keeps.
#[tauri::command]
pub fn recording_draft_toggle(
    state: State<'_, AppState>,
    spans: Vec<Span>,
    block: Span,
    others: Vec<Span>,
) -> DraftToggle {
    draft::toggled_on_grid(&spans, block, &others, snap_minutes(&state), &Local)
}

/// The editor's spans with the one at `index` dragged to `span` (only `moving` snaps), merged with what it touches.
#[tauri::command]
pub fn recording_draft_resize(
    state: State<'_, AppState>,
    spans: Vec<Span>,
    index: usize,
    span: Span,
    moving: Option<SnapEdge>,
) -> DraftSpans {
    draft::resized_on_grid(&spans, index, span, moving, snap_minutes(&state), &Local)
}

/// The editor's spans and one more after the last (Add time).
#[tauri::command]
pub fn recording_draft_add(state: State<'_, AppState>, spans: Vec<Span>) -> DraftSpans {
    draft::with_added(&spans, now_ms(), snap_minutes(&state), &Local)
}

/// The project the editor proposes for `spans`.
#[tauri::command]
pub async fn recording_proposal(
    state: State<'_, AppState>,
    spans: Vec<Span>,
) -> AppResult<RecordingProposal> {
    proposal::propose(&state.data.db()?, &spans, &state.settings.get(), now_ms()).await
}

/// The default span, on `day` when given.
#[tauri::command]
pub fn recording_default_span(state: State<'_, AppState>, day: Option<Day>) -> Span {
    default_span_on(day, now_ms(), snap_minutes(&state))
}

fn snap_minutes(state: &AppState) -> u32 {
    u32::from(state.settings.get().timeline.snap_minutes)
}
