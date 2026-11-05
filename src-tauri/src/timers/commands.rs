use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use super::origin::{TimerOrigin, repeats};
use super::{RECENT, RecentTimer, TimerStatus, picker, recent, runs, status};
use crate::bus::{Command, CommandBus};
use crate::db::recordings::{self, Recording};
use crate::db::time::now_ms;
use crate::db::{projects, tags};
use crate::error::{AppError, AppResult};
use crate::lock::MutexExt;
use crate::recordings::commands::changing;
use crate::recordings::timer::TimerInput;
use crate::state::AppState;

/// Starts timing `input`; repeating the start of the running timer leaves it as it is, edits included.
pub async fn start(app: &AppHandle, input: TimerInput) -> AppResult<Recording> {
    if let Some(running) = repeated(app, &input).await? {
        return Ok(running);
    }
    let started = changing(app, async |s| s.start_timer(input.clone(), now_ms()).await).await?;
    remember_origin(app, &started, input);
    Ok(started)
}

fn remember_origin(app: &AppHandle, running: &Recording, input: TimerInput) {
    *app.state::<AppState>().timer_origin.lock_unpoisoned() = Some(TimerOrigin {
        recording_id: running.id.clone(),
        input,
    });
}

/// The running timer when starting `input` repeats the start that made it.
async fn repeated(app: &AppHandle, input: &TimerInput) -> AppResult<Option<Recording>> {
    let state = app.state::<AppState>();
    let running = recordings::running(&state.data.db()?).await?;
    let origin = state.timer_origin.lock_unpoisoned().clone();
    Ok(running.filter(|r| repeats(origin.as_ref(), Some(&r.id), input)))
}

pub async fn stop(app: &AppHandle) -> AppResult<Option<Recording>> {
    changing(app, async |s| s.stop_timer(now_ms()).await).await
}

/// Start Timer… opens the picker; Stop Timer works while a timer runs.
pub fn register(bus: &mut CommandBus<AppHandle>) {
    bus.add(
        Command::new("timer:start", "Start Timer…")
            .help("Open the timer picker on the screen you work on")
            .run_now(|app: &AppHandle| picker::open(app, picker::Place::OnFocusedScreen)),
    );
    bus.add(
        Command::new("timer:startUnderTray", "Start Timer…")
            .help("Open the timer picker under the menu bar icon")
            .hidden()
            .run_now(|app: &AppHandle| picker::open(app, picker::Place::UnderTray)),
    );
    bus.add(
        Command::new("timer:stop", "Stop Timer")
            .help("Stop the running timer")
            .enabled_when(runs)
            .run(|app| async move { stop(&app).await.map(drop) }),
    );
    bus.add(
        Command::new("timer:toggle", "Start or Stop Timer")
            .help("Stop the running timer, else open the timer picker")
            .run(|app| async move {
                if runs(&app) {
                    stop(&app).await.map(drop)
                } else {
                    picker::open(&app, picker::Place::OnFocusedScreen);
                    Ok(())
                }
            }),
    );
}

/// Gives the running timer new fields; they become its origin, so starting the old ones switches timers.
pub async fn update(app: &AppHandle, input: TimerInput) -> AppResult<Option<Recording>> {
    let updated = changing(app, async |s| s.update_timer(input.clone(), now_ms()).await).await?;
    if let Some(running) = &updated {
        remember_origin(app, running, input);
    }
    Ok(updated)
}

/// `timetracker start`: the project by id, name or unique prefix, the tag by name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimerStartRequest {
    pub project: String,
    pub note: String,
    pub tag: Option<String>,
}

/// `timetracker timer update`: what changes; absent fields stay. The project by id, name or unique prefix,
/// the tag by name.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerUpdateRequest {
    pub project: Option<String>,
    pub note: Option<String>,
    pub tag: Option<String>,
    /// Removes the tag; `tag` wins when both are given.
    #[serde(default)]
    pub clear_tag: bool,
}

/// The started recording with the project name the CLI prints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerStarted {
    #[serde(flatten)]
    pub recording: Recording,
    pub project_name: String,
}

pub async fn start_by_name(app: &AppHandle, request: TimerStartRequest) -> AppResult<TimerStarted> {
    let db = app.state::<AppState>().data.db()?;
    let project = projects::resolve(&db, &request.project).await?;
    let tag_id = tags::resolve_optional_name(&db, request.tag.as_deref()).await?;
    let input = TimerInput {
        project_id: project.id,
        note: request.note,
        tag_id,
    };
    Ok(TimerStarted {
        recording: start(app, input).await?,
        project_name: project.name,
    })
}

/// `timetracker timer update`: the running timer with the request applied; refused while none runs.
pub async fn update_by_name(app: &AppHandle, request: TimerUpdateRequest) -> AppResult<Recording> {
    let db = app.state::<AppState>().data.db()?;
    let Some(running) = recordings::running(&db).await? else {
        return Err(AppError::InvalidInput("No timer is running".into()));
    };
    let project_id = match &request.project {
        Some(project) => projects::resolve(&db, project).await?.id,
        None => running.project_id,
    };
    let tag_id = match (&request.tag, request.clear_tag) {
        (Some(tag), _) => Some(tags::resolve_name(&db, tag).await?.id),
        (None, true) => None,
        (None, false) => running.tag_id,
    };
    let input = TimerInput {
        project_id,
        note: request.note.unwrap_or(running.note),
        tag_id,
    };
    update(app, input)
        .await?
        .ok_or_else(|| AppError::InvalidInput("No timer is running".into()))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_started_timer_flattens_the_recording_next_to_the_project_name() {
        let started = TimerStarted {
            recording: Recording {
                id: "r1".into(),
                project_id: "p1".into(),
                note: "n".into(),
                tag_id: Some("t1".into()),
                created_at: 1_000,
                updated_at: 1_000,
                spans: vec![crate::db::recordings::RecordingSpan {
                    id: "s1".into(),
                    start: 1_000,
                    end: None,
                }],
            },
            project_name: "Acme".into(),
        };

        assert_eq!(
            serde_json::to_value(&started).unwrap(),
            json!({
                "id": "r1",
                "projectId": "p1",
                "note": "n",
                "tagId": "t1",
                "createdAt": 1000,
                "updatedAt": 1000,
                "spans": [{"id": "s1", "start": 1000, "end": null}],
                "projectName": "Acme",
            })
        );
        assert_eq!(
            serde_json::from_value::<TimerStarted>(serde_json::to_value(&started).unwrap())
                .unwrap(),
            started
        );
    }

    #[test]
    fn a_start_request_keeps_its_field_names_on_the_wire() {
        let request = TimerStartRequest {
            project: "acme".into(),
            note: "x".into(),
            tag: Some("a".into()),
        };

        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            json!({ "project": "acme", "note": "x", "tag": "a" })
        );
    }
}

#[tauri::command]
pub async fn timer_start(
    app: AppHandle,
    project_id: String,
    note: String,
    tag_id: Option<String>,
) -> AppResult<Recording> {
    let input = TimerInput {
        project_id,
        note,
        tag_id,
    };
    start(&app, input).await
}

/// Changes the running timer's project, note and tag; `null` when none runs.
#[tauri::command]
pub async fn timer_update(
    app: AppHandle,
    project_id: String,
    note: String,
    tag_id: Option<String>,
) -> AppResult<Option<Recording>> {
    let input = TimerInput {
        project_id,
        note,
        tag_id,
    };
    update(&app, input).await
}

#[tauri::command]
pub async fn timer_stop(app: AppHandle) -> AppResult<Option<Recording>> {
    stop(&app).await
}

#[tauri::command]
pub async fn timer_status(state: State<'_, AppState>) -> AppResult<TimerStatus> {
    status(&state.data.db()?, now_ms()).await
}

#[tauri::command]
pub async fn timers_recent(state: State<'_, AppState>) -> AppResult<Vec<RecentTimer>> {
    recent(&state.data.db()?, RECENT).await
}

#[tauri::command]
pub fn timer_picker_open(app: AppHandle) {
    picker::open(&app, picker::Place::OnFocusedScreen);
}

#[tauri::command]
pub fn timer_picker_close(app: AppHandle) {
    picker::hide(&app);
}
