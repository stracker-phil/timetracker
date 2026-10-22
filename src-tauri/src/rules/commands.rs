use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use ts_rs::TS;

use super::types::{Rule, RuleInput, RulePreview, Suggestion};
use super::{accept, accept_rule, preview, store, suggestions_of};
use crate::db::recordings::Recording;
use crate::db::time::{Day, Ms, now_ms};
use crate::error::AppResult;
use crate::events::{self, DataKind};
use crate::recordings::commands::changing;
use crate::state::AppState;

fn rules_changed(app: &AppHandle) {
    events::data_changed(app, DataKind::Rules, None);
}

#[tauri::command]
pub async fn rules_list(state: State<'_, AppState>) -> AppResult<Vec<Rule>> {
    store::list(&state.data.db()?).await
}

#[tauri::command]
pub async fn rule_save(
    app: AppHandle,
    state: State<'_, AppState>,
    id: Option<String>,
    input: RuleInput,
) -> AppResult<Rule> {
    let rule = store::save(&state.data.db()?, id.as_deref(), &input).await?;
    rules_changed(&app);
    Ok(rule)
}

#[tauri::command]
pub async fn rule_delete(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<()> {
    store::delete(&state.data.db()?, &id).await?;
    rules_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn rules_reorder(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> AppResult<Vec<Rule>> {
    let rules = store::reorder(&state.data.db()?, &ids).await?;
    rules_changed(&app);
    Ok(rules)
}

#[tauri::command]
pub async fn rule_set_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> AppResult<Rule> {
    let rule = store::set_enabled(&state.data.db()?, &id, enabled).await?;
    rules_changed(&app);
    Ok(rule)
}

/// The blocks of the last `days` days, today included, that the input's conditions match.
#[tauri::command]
pub async fn rule_preview(
    state: State<'_, AppState>,
    input: RuleInput,
    days: u32,
) -> AppResult<RulePreview> {
    let db = state.data.db()?;
    preview(&db, &state.settings.get(), &input, Day::today(), days).await
}

#[tauri::command]
pub async fn suggestions(state: State<'_, AppState>, day: Day) -> AppResult<Vec<Suggestion>> {
    suggestions_of(&state.data.db()?, day, &state.settings.get()).await
}

/// An accepted suggestion: the new recording and how long it is, for the page's toast.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct SuggestionAccepted {
    pub recording: Recording,
    pub duration_ms: Ms,
}

/// Records the suggestion `id` of `day`; refused with `overlap` when a recording took its time meanwhile.
#[tauri::command]
pub async fn suggestion_accept(
    app: AppHandle,
    day: Day,
    id: String,
) -> AppResult<SuggestionAccepted> {
    let settings = app.state::<AppState>().settings.get();
    let recording = changing(&app, async |s| accept(s, &settings, day, &id).await).await?;
    Ok(SuggestionAccepted {
        duration_ms: recording.duration(now_ms()),
        recording,
    })
}

/// Records every suggestion of the rule on `day` as one undoable change.
#[tauri::command]
pub async fn suggestions_accept_rule(
    app: AppHandle,
    day: Day,
    rule_id: String,
) -> AppResult<Vec<Recording>> {
    let settings = app.state::<AppState>().settings.get();
    changing(&app, async |s| {
        accept_rule(s, &settings, day, &rule_id).await
    })
    .await
}
