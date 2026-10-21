//! The running timer as the tray, the page and the CLI see it, the recent timers, and the window and the
//! global shortcut that start one. Starting and stopping are recordings changes (`recordings::timer`).

pub mod commands;
pub mod picker;
pub mod shortcut;

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;

use crate::db::Db;
use crate::db::projects;
use crate::db::recordings::{self, Recording};
use crate::db::time::{Ms, now_ms};
use crate::error::AppResult;
use crate::events::TIMER_STATUS_EVENT;
use crate::recordings::timer::TimerInput;
use crate::state::AppState;
use crate::tray;

/// How many recent timers the tray and the picker offer.
pub const RECENT: usize = 8;
/// Finished recordings read to find the recent timers.
const RECENT_SCAN: u32 = 500;
/// The tray title and the page's elapsed time move on at this pace while a timer runs.
const TICK: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct RunningTimer {
    pub recording: Recording,
    pub project_name: String,
    pub color: String,
    pub shade: u8,
    pub elapsed_ms: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct TimerStatus {
    pub running: Option<RunningTimer>,
}

/// A project, note and tags recently recorded together, to time again in one click.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct RecentTimer {
    pub project_id: String,
    pub project_name: String,
    pub color: String,
    pub shade: u8,
    pub note: String,
    pub tag_ids: Vec<String>,
    pub last_used: Ms,
}

impl From<RecentTimer> for TimerInput {
    fn from(recent: RecentTimer) -> Self {
        TimerInput {
            project_id: recent.project_id,
            note: recent.note,
            tag_ids: recent.tag_ids,
        }
    }
}

pub async fn status(db: &Db, now: Ms) -> AppResult<TimerStatus> {
    let Some(recording) = recordings::running(db).await? else {
        return Ok(TimerStatus::default());
    };
    let project = projects::get(db, &recording.project_id).await?;
    Ok(TimerStatus {
        running: Some(RunningTimer {
            elapsed_ms: (now - recording.start).max(0),
            project_name: project.name,
            color: project.color,
            shade: project.shade,
            recording,
        }),
    })
}

/// The latest distinct project, note and tags combinations, newest first; archived projects left out.
pub async fn recent(db: &Db, limit: usize) -> AppResult<Vec<RecentTimer>> {
    let active = projects::list(db, false).await?;
    let mut recent: Vec<RecentTimer> = Vec::new();
    for r in recordings::latest(db, RECENT_SCAN).await? {
        if recent.len() == limit {
            break;
        }
        let Some(project) = active.iter().find(|p| p.id == r.project_id) else {
            continue;
        };
        let seen = recent
            .iter()
            .any(|t| t.project_id == r.project_id && t.note == r.note && t.tag_ids == r.tag_ids);
        if !seen {
            recent.push(RecentTimer {
                project_id: project.id.clone(),
                project_name: project.name.clone(),
                color: project.color.clone(),
                shade: project.shade,
                note: r.note,
                tag_ids: r.tag_ids,
                last_used: r.start,
            });
        }
    }
    Ok(recent)
}

/// Tells the page and the tray where the timer stands; returns whether a timer runs.
pub async fn publish(app: &AppHandle) -> bool {
    let Ok(db) = app.state::<AppState>().data.db() else {
        return false;
    };
    let (status, recent) = match (status(&db, now_ms()).await, recent(&db, RECENT).await) {
        (Ok(status), Ok(recent)) => (status, recent),
        (Err(e), _) | (_, Err(e)) => {
            log::warn!("Couldn't read the timer: {e}");
            return false;
        }
    };
    if let Err(e) = app.emit(TIMER_STATUS_EVENT, &status) {
        log::warn!("Couldn't notify the page of the timer: {e}");
    }
    tray::set_timer(app, status.running.as_ref(), recent);
    status.running.is_some()
}

/// Publishes now, and again every minute while a timer runs; a change in between publishes on its own.
pub fn start_ticking(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut running = publish(&app).await;
        let mut ticks = tokio::time::interval(TICK);
        ticks.tick().await;
        loop {
            ticks.tick().await;
            if running || timer_runs(&app).await {
                running = publish(&app).await;
            }
        }
    });
}

/// Started elsewhere (the CLI, the tray) since the last tick.
async fn timer_runs(app: &AppHandle) -> bool {
    match app.state::<AppState>().data.db() {
        Ok(db) => recordings::running(&db).await.ok().flatten().is_some(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::db::projects::Project;
    use crate::db::recordings::NewRecording;
    use crate::db::tags;
    use crate::test_support::{project, recording};

    const MIN: Ms = 60_000;
    const HOUR: Ms = 60 * MIN;
    const T0: Ms = 1_781_517_600_000;

    async fn db() -> Db {
        Db::open_memory().await.expect("open db")
    }

    async fn record(
        db: &Db,
        p: &Project,
        start: Ms,
        end: Option<Ms>,
        note: &str,
        tag_ids: &[&str],
    ) {
        recordings::insert(
            db,
            NewRecording {
                id: crate::db::new_id(),
                project_id: p.id.clone(),
                start,
                end,
                note: note.into(),
                tag_ids: tag_ids.iter().map(|t| t.to_string()).collect(),
            },
        )
        .await
        .expect("insert recording");
    }

    #[tokio::test]
    async fn status_without_a_timer_is_not_running() {
        let db = db().await;
        let p = project(&db, "Acme").await;
        recording(&db, &p.id, T0, Some(T0 + HOUR)).await;

        assert_eq!(
            status(&db, T0 + 2 * HOUR).await.unwrap(),
            TimerStatus { running: None }
        );
    }

    #[tokio::test]
    async fn status_reports_the_running_timer_with_its_project_and_elapsed_time() {
        let db = db().await;
        let p = project(&db, "Acme").await;
        let running = recording(&db, &p.id, T0, None).await;

        let timer = status(&db, T0 + 25 * MIN).await.unwrap().running.unwrap();

        assert_eq!(
            timer,
            RunningTimer {
                recording: running,
                project_name: "Acme".into(),
                color: p.color.clone(),
                shade: p.shade,
                elapsed_ms: 25 * MIN,
            }
        );
    }

    #[tokio::test]
    async fn a_timer_started_yesterday_reports_its_full_elapsed_time() {
        let db = db().await;
        let p = project(&db, "Acme").await;
        recording(&db, &p.id, T0, None).await;

        let timer = status(&db, T0 + 26 * HOUR).await.unwrap().running.unwrap();

        assert_eq!(timer.elapsed_ms, 26 * HOUR);
    }

    #[tokio::test]
    async fn elapsed_time_is_never_negative() {
        let db = db().await;
        let p = project(&db, "Acme").await;
        recording(&db, &p.id, T0, None).await;

        let timer = status(&db, T0 - MIN).await.unwrap().running.unwrap();

        assert_eq!(timer.elapsed_ms, 0);
    }

    #[tokio::test]
    async fn the_status_serializes_with_camel_case_fields() {
        let db = db().await;
        assert_eq!(
            serde_json::to_value(status(&db, T0).await.unwrap()).unwrap(),
            json!({ "running": null })
        );

        let p = project(&db, "Acme").await;
        recording(&db, &p.id, T0, None).await;
        let value = serde_json::to_value(status(&db, T0 + MIN).await.unwrap()).unwrap();

        let running = &value["running"];
        assert_eq!(running["projectName"], "Acme");
        assert_eq!(running["elapsedMs"], MIN);
        assert_eq!(running["recording"]["projectId"], json!(p.id));
        assert_eq!(running["recording"]["end"], json!(null));
    }

    #[tokio::test]
    async fn recent_lists_the_newest_eight_distinct_timers_first() {
        let db = db().await;
        let p = project(&db, "Acme").await;
        for i in 0..10 {
            record(
                &db,
                &p,
                T0 + i * HOUR,
                Some(T0 + i * HOUR + MIN),
                &format!("n{i}"),
                &[],
            )
            .await;
        }

        let recent = recent(&db, RECENT).await.unwrap();

        let notes: Vec<&str> = recent.iter().map(|t| t.note.as_str()).collect();
        assert_eq!(notes, ["n9", "n8", "n7", "n6", "n5", "n4", "n3", "n2"]);
    }

    #[tokio::test]
    async fn recent_collapses_duplicates_skips_archived_projects_and_the_running_timer() {
        let db = db().await;
        let acme = project(&db, "Acme").await;
        let beta = project(&db, "Beta").await;
        let alpha = tags::insert(&db, "alpha").await.unwrap();
        record(&db, &acme, T0, Some(T0 + MIN), "x", &[&alpha.id]).await;
        record(
            &db,
            &acme,
            T0 + HOUR,
            Some(T0 + HOUR + MIN),
            "x",
            &[&alpha.id],
        )
        .await;
        record(
            &db,
            &acme,
            T0 + 2 * HOUR,
            Some(T0 + 2 * HOUR + MIN),
            "x",
            &[],
        )
        .await;
        record(
            &db,
            &beta,
            T0 + 3 * HOUR,
            Some(T0 + 3 * HOUR + MIN),
            "y",
            &[],
        )
        .await;
        record(&db, &acme, T0 + 4 * HOUR, None, "running", &[]).await;
        projects::set_archived(&db, &beta.id, true).await.unwrap();

        let recent = recent(&db, RECENT).await.unwrap();

        let timer = |note: &str, tag_ids: Vec<String>, last_used: Ms| RecentTimer {
            project_id: acme.id.clone(),
            project_name: "Acme".into(),
            color: acme.color.clone(),
            shade: acme.shade,
            note: note.into(),
            tag_ids,
            last_used,
        };
        assert_eq!(
            recent,
            vec![
                timer("x", vec![], T0 + 2 * HOUR),
                timer("x", vec![alpha.id.clone()], T0 + HOUR),
            ]
        );
    }

    #[tokio::test]
    async fn recent_returns_at_most_the_limit() {
        let db = db().await;
        let p = project(&db, "Acme").await;
        for i in 0..4 {
            record(
                &db,
                &p,
                T0 + i * HOUR,
                Some(T0 + i * HOUR + MIN),
                &format!("n{i}"),
                &[],
            )
            .await;
        }

        assert_eq!(recent(&db, 3).await.unwrap().len(), 3);
        assert!(recent(&db, 0).await.unwrap().is_empty());
    }
}
