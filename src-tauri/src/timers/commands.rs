use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use super::origin::{TimerOrigin, repeats};
use super::{RECENT, RecentTimer, Timer, TimerStatus, current, picker, recent, runs, status};
use crate::bus::{Command, CommandBus};
use crate::db::recordings::Recording;
use crate::db::time::now_ms;
use crate::db::{projects, tags};
use crate::error::{AppError, AppResult};
use crate::recordings::cli::Recorded;
use crate::recordings::commands::changing;
use crate::recordings::timer::TimerInput;
use crate::scripting::writes::{CLI_SOURCE, Suggested};
use crate::state::AppState;

/// Starts timing `input` as a recording; repeating the start of the running timer leaves it as it is,
/// edits included. The picker, the tray and the menus start timers this way.
pub async fn start(app: &AppHandle, input: TimerInput) -> AppResult<Recording> {
    if let Some(Timer::Recording(running)) = repeated(app, &input).await? {
        return Ok(running);
    }
    let started = changing(app, async |s| s.start_timer(input.clone(), now_ms()).await).await?;
    remember_origin(app, &started.id, input).await;
    Ok(started)
}

/// Starts timing `input` as a suggestion from `source` (ADR 0041), as other apps do. Repeating the start of
/// the running timer, or matching a running recording's fields, leaves it as it is: the owner's timer wins.
pub async fn start_suggested(app: &AppHandle, input: TimerInput, source: &str) -> AppResult<Timer> {
    if let Some(running) = repeated(app, &input).await? {
        return Ok(running);
    }
    let started = changing(app, async |s| {
        s.start_suggested_timer(input.clone(), source, now_ms())
            .await
    })
    .await?;
    if started.is_suggested() {
        remember_origin(app, started.id(), input).await;
    }
    Ok(started)
}

/// A failed save only costs the origin after a restart, so it is logged, not returned.
async fn remember_origin(app: &AppHandle, timer_id: &str, input: TimerInput) {
    let origin = TimerOrigin {
        timer_id: timer_id.to_string(),
        input,
    };
    if let Err(e) = app.state::<AppState>().timer_origin.set(origin).await {
        log::warn!("Could not save the timer's origin: {e}");
    }
}

/// The running timer when starting `input` repeats the start that made it.
async fn repeated(app: &AppHandle, input: &TimerInput) -> AppResult<Option<Timer>> {
    let state = app.state::<AppState>();
    let running = current(&state.data.db()?).await?;
    let origin = state.timer_origin.get();
    Ok(running.filter(|t| repeats(origin.as_ref(), Some(t.id()), input)))
}

pub async fn stop(app: &AppHandle) -> AppResult<Option<Timer>> {
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
pub async fn update(app: &AppHandle, input: TimerInput) -> AppResult<Option<Timer>> {
    let updated = changing(app, async |s| s.update_timer(input.clone(), now_ms()).await).await?;
    if let Some(running) = &updated {
        remember_origin(app, running.id(), input).await;
    }
    Ok(updated)
}

/// `timetracker timer start`: the project by id, name or unique prefix, the tag by name. Other apps start
/// suggested timers unless `final` (ADR 0041).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimerStartRequest {
    pub project: String,
    pub note: String,
    pub tag: Option<String>,
    /// A running recording; else a suggestion to review.
    #[serde(default, rename = "final")]
    pub final_write: bool,
    /// The suggestion's source; `cli` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
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

/// What a start left running, with the project name the CLI prints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TimerStarted {
    Recording(Recorded),
    Suggestion(Suggested),
    /// A suggested start that matched the running recording, which stays as it is.
    Running(Recorded),
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
    if request.final_write {
        return Ok(TimerStarted::Recording(Recorded {
            recording: start(app, input).await?,
            project_name: project.name,
        }));
    }
    let source = request.source.as_deref().unwrap_or(CLI_SOURCE);
    Ok(match start_suggested(app, input, source).await? {
        Timer::Recording(recording) => TimerStarted::Running(Recorded {
            recording,
            project_name: project.name,
        }),
        Timer::Suggestion(suggestion) => TimerStarted::Suggestion(Suggested {
            suggestion,
            project_name: Some(project.name),
        }),
    })
}

/// `timetracker timer update`: the running timer with the request applied; refused while none runs.
pub async fn update_by_name(app: &AppHandle, request: TimerUpdateRequest) -> AppResult<Timer> {
    let db = app.state::<AppState>().data.db()?;
    let Some(running) = current(&db).await? else {
        return Err(AppError::InvalidInput("No timer is running".into()));
    };
    let project_id = match (&request.project, running.project_id()) {
        (Some(project), _) => projects::resolve(&db, project).await?.id,
        (None, Some(id)) => id.to_string(),
        (None, None) => {
            return Err(AppError::InvalidInput(
                "The timer has no project; add --project".into(),
            ));
        }
    };
    let tag_id = match (&request.tag, request.clear_tag) {
        (Some(tag), _) => Some(tags::resolve_name(&db, tag).await?.id),
        (None, true) => None,
        (None, false) => running.tag_id().map(str::to_string),
    };
    let input = TimerInput {
        project_id,
        note: request.note.unwrap_or_else(|| running.note().to_string()),
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
    use crate::db::stored_suggestions::StoredSuggestion;
    use crate::db::time::Day;

    #[test]
    fn the_started_timer_flattens_the_recording_next_to_the_project_name() {
        let started = TimerStarted::Recording(Recorded {
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
        });

        assert_eq!(
            serde_json::to_value(&started).unwrap(),
            json!({
                "kind": "recording",
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
            final_write: false,
            source: None,
        };

        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            json!({ "project": "acme", "note": "x", "tag": "a", "final": false })
        );
    }

    #[test]
    fn a_start_request_carries_final_and_the_source_when_given() {
        let request = TimerStartRequest {
            project: "acme".into(),
            note: String::new(),
            tag: None,
            final_write: true,
            source: Some("claude".into()),
        };

        let value = serde_json::to_value(&request).unwrap();

        assert_eq!(
            value,
            json!({ "project": "acme", "note": "", "tag": null, "final": true, "source": "claude" })
        );
        assert_eq!(
            serde_json::from_value::<TimerStartRequest>(value).unwrap(),
            request
        );
    }

    #[test]
    fn a_start_request_without_final_or_source_is_a_suggested_start_from_the_cli() {
        let request: TimerStartRequest =
            serde_json::from_value(json!({ "project": "acme", "note": "x", "tag": null })).unwrap();

        assert_eq!(
            request,
            TimerStartRequest {
                project: "acme".into(),
                note: "x".into(),
                tag: None,
                final_write: false,
                source: None,
            }
        );
    }

    fn suggestion() -> StoredSuggestion {
        StoredSuggestion {
            id: "s1".into(),
            project_id: Some("p1".into()),
            note: "n".into(),
            tag_id: None,
            source: "claude".into(),
            day: Day::parse("2026-01-05").unwrap(),
            spans: Vec::new(),
            created_at: 1_000,
            updated_at: 1_000,
            open_start: Some(5_000),
        }
    }

    #[test]
    fn a_started_suggestion_serializes_with_its_kind_next_to_the_project_name() {
        let started = TimerStarted::Suggestion(Suggested {
            suggestion: suggestion(),
            project_name: Some("Acme".into()),
        });

        let value = serde_json::to_value(&started).unwrap();

        assert_eq!(
            value,
            json!({
                "kind": "suggestion",
                "id": "s1",
                "projectId": "p1",
                "note": "n",
                "tagId": null,
                "source": "claude",
                "day": "2026-01-05",
                "spans": [],
                "createdAt": 1000,
                "updatedAt": 1000,
                "openStart": 5000,
                "projectName": "Acme",
            })
        );
        assert_eq!(
            serde_json::from_value::<TimerStarted>(value).unwrap(),
            started
        );
    }

    #[test]
    fn a_start_that_matched_the_running_recording_serializes_as_running() {
        let started = TimerStarted::Running(Recorded {
            recording: Recording {
                id: "r1".into(),
                project_id: "p1".into(),
                note: String::new(),
                tag_id: None,
                created_at: 1_000,
                updated_at: 1_000,
                spans: Vec::new(),
            },
            project_name: "Acme".into(),
        });

        let value = serde_json::to_value(&started).unwrap();

        assert_eq!(value["kind"], "running");
        assert_eq!(value["projectName"], "Acme");
        assert_eq!(
            serde_json::from_value::<TimerStarted>(value).unwrap(),
            started
        );
    }
}

/// Starts a running recording: the picker's start.
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
) -> AppResult<Option<Timer>> {
    let input = TimerInput {
        project_id,
        note,
        tag_id,
    };
    update(&app, input).await
}

#[tauri::command]
pub async fn timer_stop(app: AppHandle) -> AppResult<Option<Timer>> {
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
