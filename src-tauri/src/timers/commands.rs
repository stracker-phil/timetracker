use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use super::{RECENT, RecentTimer, TimerStatus, picker, publish, recent, status};
use crate::db::recordings::Recording;
use crate::db::time::now_ms;
use crate::db::{projects, tags};
use crate::error::AppResult;
use crate::recordings::commands::changing;
use crate::recordings::timer::TimerInput;
use crate::state::AppState;

pub async fn start(app: &AppHandle, input: TimerInput) -> AppResult<Recording> {
    let started = changing(app, async |s| s.start_timer(input, now_ms()).await).await?;
    publish(app).await;
    Ok(started)
}

pub async fn stop(app: &AppHandle) -> AppResult<Option<Recording>> {
    let stopped = changing(app, async |s| s.stop_timer(now_ms()).await).await?;
    publish(app).await;
    Ok(stopped)
}

/// `timetracker start`: the project by id, name or unique prefix, the tags by name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimerStartRequest {
    pub project: String,
    pub note: String,
    pub tags: Vec<String>,
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
    let tag_ids = tags::resolve_names(&db, &request.tags)
        .await?
        .into_iter()
        .map(|t| t.id)
        .collect();
    let input = TimerInput {
        project_id: project.id,
        note: request.note,
        tag_ids,
    };
    Ok(TimerStarted {
        recording: start(app, input).await?,
        project_name: project.name,
    })
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
                start: 1_000,
                end: None,
                note: "n".into(),
                tag_ids: vec!["t1".into()],
                created_at: 1_000,
                updated_at: 1_000,
            },
            project_name: "Acme".into(),
        };

        assert_eq!(
            serde_json::to_value(&started).unwrap(),
            json!({
                "id": "r1",
                "projectId": "p1",
                "start": 1000,
                "end": null,
                "note": "n",
                "tagIds": ["t1"],
                "createdAt": 1000,
                "updatedAt": 1000,
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
            tags: vec!["a".into(), "b".into()],
        };

        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            json!({ "project": "acme", "note": "x", "tags": ["a", "b"] })
        );
    }
}

#[tauri::command]
pub async fn timer_start(
    app: AppHandle,
    project_id: String,
    note: String,
    tag_ids: Vec<String>,
) -> AppResult<Recording> {
    let input = TimerInput {
        project_id,
        note,
        tag_ids,
    };
    start(&app, input).await
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
    picker::open(&app);
}

#[tauri::command]
pub fn timer_picker_close(app: AppHandle) {
    picker::hide(&app);
}
