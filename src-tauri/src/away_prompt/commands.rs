use tauri::{AppHandle, Manager, State};

use super::{AwayPrompt, AwayPromptState, answer, skip, window};
use crate::db::recordings::Recording;
use crate::db::time::now_ms;
use crate::error::AppResult;
use crate::recordings::RecordingInput;
use crate::recordings::commands::changing;

#[tauri::command]
pub fn away_prompt_current(prompts: State<'_, AwayPromptState>) -> Option<AwayPrompt> {
    prompts.current()
}

/// Records the answer and closes the window; an `overlap` keeps both, so the times can change.
#[tauri::command]
pub async fn away_prompt_answer(app: AppHandle, input: RecordingInput) -> AppResult<Recording> {
    let prompts = app.state::<AwayPromptState>();
    let recording = changing(&app, async |s| answer(&prompts, s, input, now_ms()).await).await?;
    window::close(&app);
    Ok(recording)
}

#[tauri::command]
pub fn away_prompt_skip(app: AppHandle) {
    skip(&app);
}
