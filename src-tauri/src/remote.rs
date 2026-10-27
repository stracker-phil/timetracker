//! Requests from other processes (the CLI, deep links): `handle` maps each `IpcMessage` to the module
//! that does it. Requests only the page can answer go out as `REMOTE_EVENT` with an id and come back
//! through `remote_reply`.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use ts_rs::TS;

use crate::activity;
use crate::db::Db;
use crate::db::time::{Day, now_ms};
use crate::error::{AppError, ErrorKind};
use crate::events::REMOTE_EVENT;
use crate::ipc::{HandlerResult, IpcError, IpcMessage};
use crate::launch;
use crate::pending::{NoReply, PendingReplies};
use crate::recordings::commands::changing;
use crate::reports;
use crate::scripting::timer::{self, TimerState};
use crate::scripting::{catalog, listing, writes};
use crate::state::AppState;
use crate::timers;
use crate::tracker::{Tracker, TrackerStatus};
use crate::{watch, window};

/// Below the IPC client's timeout, so the sender still reads the reply.
const REPLY_TIMEOUT: Duration = Duration::from_secs(4);
/// Lets the reply to `quit` reach the sender before the app exits.
const QUIT_DELAY: Duration = Duration::from_millis(200);

#[derive(Serialize, Debug, Clone, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RemoteRequest {
    /// Navigate to the day's Day view.
    ShowDay { day: Day },
}

#[derive(Serialize, Debug, Clone, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct RemoteEvent {
    pub id: String,
    pub request: RemoteRequest,
}

/// The reply to `status`. `tracker` and `billable` are `None` while the data store is unavailable.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct StatusReport {
    pub version: String,
    pub tracker: Option<TrackerStatus>,
    pub timer: TimerState,
    pub billable: Option<reports::Billable>,
    pub window_visible: bool,
}

#[derive(Default)]
pub struct Bridge {
    pending: PendingReplies<HandlerResult>,
}

impl Bridge {
    /// Asks the page and waits for its answer, at most `REPLY_TIMEOUT`.
    pub async fn ask(&self, handle: &AppHandle, request: RemoteRequest) -> HandlerResult {
        let sent = self.pending.ask(REPLY_TIMEOUT, |id| {
            let event = RemoteEvent {
                id: id.to_string(),
                request,
            };
            handle.emit(REMOTE_EVENT, event)
        });
        let failed = |message: String| Err(IpcError::new(ErrorKind::Io, message));
        match sent.await {
            Ok(result) => result,
            Err(NoReply::NotSent(e)) => {
                failed(format!("Couldn't reach the TimeTracker window: {e}"))
            }
            Err(NoReply::Dropped) => failed("The TimeTracker window dropped the request".into()),
            Err(NoReply::TimedOut) => {
                failed("The TimeTracker window did not answer in time".into())
            }
        }
    }

    /// False when no request with `id` waits (answered, timed out or unknown).
    pub fn answer(&self, id: &str, result: HandlerResult) -> bool {
        self.pending.answer(id, result)
    }
}

/// The page's answer to a `REMOTE_EVENT`: `error` set means it failed.
#[tauri::command]
pub fn remote_reply(
    bridge: State<'_, Bridge>,
    id: String,
    result: Option<serde_json::Value>,
    error: Option<String>,
) {
    let outcome = match error {
        Some(error) => Err(IpcError::new(ErrorKind::Io, error)),
        None => Ok(result.unwrap_or_default()),
    };
    if !bridge.answer(&id, outcome) {
        log::debug!("Late or unknown remote reply {id}");
    }
}

fn tracker(app: &AppHandle) -> Result<Arc<Tracker>, IpcError> {
    app.try_state::<Arc<Tracker>>()
        .map(|tracker| Arc::clone(&tracker))
        .ok_or_else(|| {
            IpcError::new(
                ErrorKind::Db,
                "The tracker is off: the data store is not available",
            )
        })
}

fn db(app: &AppHandle) -> Result<Db, IpcError> {
    Ok(app.state::<AppState>().data.db()?)
}

fn json(value: impl Serialize) -> HandlerResult {
    serde_json::to_value(value).map_err(|e| AppError::from(e).into())
}

pub async fn status_report(app: &AppHandle) -> StatusReport {
    let (timer, billable) = match app.state::<AppState>().data.db() {
        Ok(db) => {
            let now = now_ms();
            let timer = timer::state(&db, now).await.unwrap_or_default();
            let today = Day::today();
            let billable = reports::billable(&db, &reports::Range::Today, today, now)
                .await
                .ok();
            (timer, billable)
        }
        Err(_) => (TimerState::default(), None),
    };
    StatusReport {
        version: app.package_info().version.to_string(),
        tracker: tracker(app).ok().map(|t| t.status()),
        timer,
        billable,
        window_visible: window::main_visible(app),
    }
}

/// Today when `day` is `None`.
async fn show(app: &AppHandle, day: Option<Day>, focus: bool) -> HandlerResult {
    if focus {
        window::show_main(app);
    } else {
        window::show_main_in_background(app);
    }
    let day = day.unwrap_or_else(Day::today);
    app.state::<Bridge>()
        .ask(app, RemoteRequest::ShowDay { day })
        .await
}

fn quit_after_reply(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(QUIT_DELAY).await;
        app.exit(0);
    });
}

/// Handles one message from another process and returns what its sender prints.
pub async fn handle(app: AppHandle, message: IpcMessage) -> HandlerResult {
    log::info!("Received {} from another process", message.kind());
    match message {
        IpcMessage::Show { day, focus } => show(&app, day, focus).await,
        IpcMessage::Hide => {
            window::hide_main(&app);
            Ok(serde_json::Value::Null)
        }
        IpcMessage::Quit => {
            quit_after_reply(app);
            Ok(serde_json::Value::Null)
        }
        IpcMessage::Status => json(status_report(&app).await),
        IpcMessage::Pause { until } => json(tracker(&app)?.pause(until, now_ms()).await?),
        IpcMessage::Resume => json(tracker(&app)?.resume(now_ms()).await?),
        IpcMessage::ActivityAdd { items } => {
            let ids = activity::add(&app, &items).await?;
            json(serde_json::json!({ "ids": ids }))
        }
        IpcMessage::ActivityStart { input } => json(activity::start(&app, &input).await?),
        IpcMessage::ActivityStop { source, end } => json(activity::stop(&app, &source, end).await?),
        IpcMessage::ActivityDelete { from, to } => {
            json(activity::delete_range(&app, from, to).await?)
        }
        IpcMessage::ActivityDeleteUndo => json(activity::undo_delete(&app).await?),
        IpcMessage::Record { request } => json(writes::record(&app, request).await?),
        IpcMessage::Edit { request } => json(writes::edit(&app, request).await?),
        IpcMessage::Delete { id, final_write } => {
            json(writes::delete(&app, &id, final_write).await?)
        }
        IpcMessage::Approve { ids } => json(writes::approve(&app, &ids).await?),
        IpcMessage::Recordings { range } => json(listing::recordings_in(&app, &range).await?),
        IpcMessage::Suggestions { range } => json(listing::suggestions_in(&app, &range).await?),
        IpcMessage::Projects { archived } => json(catalog::projects(&db(&app)?, archived).await?),
        IpcMessage::Clients { archived } => json(catalog::clients(&db(&app)?, archived).await?),
        IpcMessage::Tags => json(catalog::tags(&db(&app)?).await?),
        IpcMessage::Undo => json(changing(&app, async |s| s.undo().await).await?),
        IpcMessage::Redo => json(changing(&app, async |s| s.redo().await).await?),
        IpcMessage::Report { request, output } => {
            json(reports::cli::report(&app, request, output).await?)
        }
        IpcMessage::TimerStart { request } => {
            json(timers::commands::start_by_name(&app, request).await?)
        }
        IpcMessage::TimerStop => json(timers::commands::stop(&app).await?),
        IpcMessage::TimerStatus => json(timer::state(&db(&app)?, now_ms()).await?),
        IpcMessage::TimerPicker => {
            timers::picker::open(&app, timers::picker::Place::OnFocusedScreen);
            Ok(serde_json::Value::Null)
        }
        IpcMessage::TimerRecent { limit } => json(timer::recent(&db(&app)?, limit).await?),
        IpcMessage::TrackerStatus => json(tracker(&app)?.status()),
        IpcMessage::Watch => json(watch::current(&app).await),
        IpcMessage::TimerUpdate { request } => {
            timers::commands::update_by_name(&app, request).await?;
            json(timer::state(&db(&app)?, now_ms()).await?)
        }
        launch @ (IpcMessage::Activate { .. } | IpcMessage::Error { .. }) => {
            launch::queue(&app, launch);
            Ok(serde_json::Value::Null)
        }
        #[cfg(debug_assertions)]
        IpcMessage::Eval { js } => {
            crate::dev::eval(&app, &js);
            Ok(serde_json::Value::Null)
        }
        #[cfg(debug_assertions)]
        IpcMessage::DevSeed {
            years,
            yes,
            showcase,
        } => json(crate::dev::seed(&app, years, yes, showcase).await?),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::PermissionStatus;
    use crate::tracker::TrackerState;

    fn report(tracker: Option<TrackerStatus>) -> StatusReport {
        StatusReport {
            version: "1.2.3".into(),
            tracker,
            timer: TimerState::default(),
            billable: None,
            window_visible: true,
        }
    }

    fn running() -> TrackerStatus {
        TrackerStatus {
            state: TrackerState::Running,
            current: None,
            permission: PermissionStatus::Granted,
            text: "Tracking".into(),
        }
    }

    #[test]
    fn status_report_serializes_with_camel_case_keys() {
        assert_eq!(
            serde_json::to_value(report(Some(running()))).unwrap(),
            serde_json::json!({
                "version": "1.2.3",
                "tracker": {
                    "state": { "kind": "running" },
                    "current": null,
                    "permission": "granted",
                    "text": "Tracking"
                },
                "timer": {
                    "running": false,
                    "recordingId": null,
                    "projectId": null,
                    "project": null,
                    "client": null,
                    "tagId": null,
                    "tag": null,
                    "note": null,
                    "startedAt": null,
                    "startedAtIso": null,
                    "elapsedMs": null
                },
                "billable": null,
                "windowVisible": true
            })
        );
    }

    #[test]
    fn status_report_carries_the_running_timer_as_flat_fields() {
        let timer = TimerState {
            running: true,
            recording_id: Some("r1".into()),
            project_id: Some("p1".into()),
            project: Some("Portal".into()),
            client: Some("Acme".into()),
            tag_id: Some("t1".into()),
            tag: Some("development".into()),
            note: Some("standup".into()),
            started_at: Some(1_000),
            started_at_iso: Some("1970-01-01T00:00:01+00:00".into()),
            elapsed_ms: Some(60_000),
        };
        let original = StatusReport {
            timer,
            ..report(None)
        };

        let value = serde_json::to_value(&original).unwrap();

        assert_eq!(
            value["timer"],
            serde_json::json!({
                "running": true,
                "recordingId": "r1",
                "projectId": "p1",
                "project": "Portal",
                "client": "Acme",
                "tagId": "t1",
                "tag": "development",
                "note": "standup",
                "startedAt": 1000,
                "startedAtIso": "1970-01-01T00:00:01+00:00",
                "elapsedMs": 60000
            })
        );
        assert_eq!(
            serde_json::from_value::<StatusReport>(value).unwrap(),
            original
        );
    }

    #[test]
    fn status_report_without_a_tracker_serializes_it_as_null() {
        let value = serde_json::to_value(report(None)).unwrap();
        assert_eq!(value["tracker"], serde_json::Value::Null);
    }

    #[test]
    fn status_report_round_trips_through_json() {
        let paused = TrackerStatus {
            state: TrackerState::Paused { until: Some(5_000) },
            text: "Paused".into(),
            ..running()
        };
        for original in [report(Some(running())), report(Some(paused)), report(None)] {
            let json = serde_json::to_string(&original).unwrap();
            assert_eq!(
                serde_json::from_str::<StatusReport>(&json).unwrap(),
                original
            );
        }
    }

    #[test]
    fn show_day_request_serializes_with_a_type_tag() {
        let day = Day::parse("2026-10-21").unwrap();
        assert_eq!(
            serde_json::to_value(RemoteRequest::ShowDay { day }).unwrap(),
            serde_json::json!({ "type": "showDay", "day": "2026-10-21" })
        );
    }
}
