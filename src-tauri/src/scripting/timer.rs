//! The running timer as flat fields for scripts (`timer status --json`, `status --json`), and the recent
//! timers (`timer recent --json`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::day;
use crate::db::time::{Ms, to_local_iso};
use crate::db::{Db, recordings};
use crate::error::AppResult;
use crate::timers::{self, open_start};

/// Every field but `running` is `None` while no timer runs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct TimerState {
    pub running: bool,
    pub recording_id: Option<String>,
    pub project_id: Option<String>,
    pub project: Option<String>,
    pub client: Option<String>,
    pub tag_id: Option<String>,
    pub tag: Option<String>,
    pub note: Option<String>,
    pub started_at: Option<Ms>,
    pub started_at_iso: Option<String>,
    pub elapsed_ms: Option<Ms>,
}

/// A project, note and tag recently recorded together, as the tray's Recent Timers offer it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentTimerEntry {
    pub project_id: String,
    pub project: String,
    pub client: Option<String>,
    pub tag_id: Option<String>,
    pub tag: Option<String>,
    pub note: String,
    /// Where its latest span starts.
    pub last_used_at: Ms,
}

/// Newest first, `limit` held to `1..=MAX_RECENT`; archived projects left out (`timers::recent`).
pub async fn recent(db: &Db, limit: u32) -> AppResult<Vec<RecentTimerEntry>> {
    let names = day::names(db).await?;
    let limit = (limit as usize).clamp(1, timers::MAX_RECENT);
    Ok(timers::recent(db, limit)
        .await?
        .into_iter()
        .map(|recent| RecentTimerEntry {
            client: names
                .projects
                .get(&recent.project_id)
                .and_then(|p| p.client_name.clone()),
            tag: recent
                .tag_id
                .as_ref()
                .and_then(|id| names.tags.get(id))
                .map(|t| t.name.clone()),
            project_id: recent.project_id,
            project: recent.project_name,
            tag_id: recent.tag_id,
            note: recent.note,
            last_used_at: recent.last_used,
        })
        .collect())
}

pub async fn state(db: &Db, now: Ms) -> AppResult<TimerState> {
    let Some(recording) = recordings::running(db).await? else {
        return Ok(TimerState::default());
    };
    let names = day::names(db).await?;
    let project = names.projects.get(&recording.project_id);
    let started_at = open_start(&recording);
    Ok(TimerState {
        running: true,
        project: project.map(|p| p.name.clone()),
        client: project.and_then(|p| p.client_name.clone()),
        tag: recording
            .tag_id
            .as_ref()
            .and_then(|id| names.tags.get(id))
            .map(|t| t.name.clone()),
        recording_id: Some(recording.id),
        project_id: Some(recording.project_id),
        tag_id: recording.tag_id,
        note: Some(recording.note),
        started_at: Some(started_at),
        started_at_iso: Some(to_local_iso(started_at)),
        elapsed_ms: Some((now - started_at).max(0)),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::db::projects::{self, ProjectInput};
    use crate::db::recordings::NewRecording;
    use crate::db::tags;
    use crate::test_support::{client, project, recording};

    const MIN: Ms = 60_000;

    async fn timer_of(db: &Db, project_id: &str, note: &str, tag_id: Option<&str>) -> String {
        recordings::insert(
            db,
            NewRecording {
                id: uuid::Uuid::new_v4().to_string(),
                project_id: project_id.into(),
                spans: vec![(1_000 * MIN, Some(1_060 * MIN)), (2_000 * MIN, None)],
                note: note.into(),
                tag_id: tag_id.map(String::from),
            },
        )
        .await
        .unwrap()
        .id
    }

    async fn finished(db: &Db, project_id: &str, start: Ms, note: &str, tag_id: Option<&str>) {
        recordings::insert(
            db,
            NewRecording {
                id: uuid::Uuid::new_v4().to_string(),
                project_id: project_id.into(),
                spans: vec![(start, Some(start + 30 * MIN))],
                note: note.into(),
                tag_id: tag_id.map(String::from),
            },
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn recent_timers_name_their_project_client_and_tag_newest_first() {
        let db = Db::open_memory().await.unwrap();
        let acme = client(&db, "Acme").await;
        let portal = projects::insert(
            &db,
            ProjectInput {
                client_id: Some(acme.id),
                name: "Portal".into(),
                color: "blue".into(),
                shade: None,
                billable: true,
            },
        )
        .await
        .unwrap();
        let admin = project(&db, "admin").await;
        let dev = tags::insert(&db, "dev").await.unwrap();
        finished(&db, &admin.id, 1_000 * MIN, "", None).await;
        finished(&db, &portal.id, 2_000 * MIN, "standup", Some(&dev.id)).await;

        assert_eq!(
            recent(&db, 8).await.unwrap(),
            [
                RecentTimerEntry {
                    project_id: portal.id,
                    project: "Portal".into(),
                    client: Some("Acme".into()),
                    tag_id: Some(dev.id),
                    tag: Some("dev".into()),
                    note: "standup".into(),
                    last_used_at: 2_000 * MIN,
                },
                RecentTimerEntry {
                    project_id: admin.id,
                    project: "admin".into(),
                    client: None,
                    tag_id: None,
                    tag: None,
                    note: String::new(),
                    last_used_at: 1_000 * MIN,
                },
            ]
        );
    }

    #[tokio::test]
    async fn recent_timers_are_at_most_the_limit_newest_first() {
        let db = Db::open_memory().await.unwrap();
        let podcast = project(&db, "Podcast").await;
        for i in 0..5 {
            finished(
                &db,
                &podcast.id,
                (1_000 + i * 60) * MIN,
                &format!("n{i}"),
                None,
            )
            .await;
        }

        let notes: Vec<String> = recent(&db, 3)
            .await
            .unwrap()
            .into_iter()
            .map(|t| t.note)
            .collect();

        assert_eq!(notes, ["n4", "n3", "n2"]);
    }

    #[tokio::test]
    async fn recent_timers_with_a_zero_limit_list_the_newest_one() {
        let db = Db::open_memory().await.unwrap();
        let podcast = project(&db, "Podcast").await;
        finished(&db, &podcast.id, 1_000 * MIN, "older", None).await;
        finished(&db, &podcast.id, 2_000 * MIN, "newer", None).await;

        let notes: Vec<String> = recent(&db, 0)
            .await
            .unwrap()
            .into_iter()
            .map(|t| t.note)
            .collect();

        assert_eq!(notes, ["newer"]);
    }

    #[tokio::test]
    async fn recent_timers_are_at_most_max_recent_however_large_the_limit() {
        let db = Db::open_memory().await.unwrap();
        let podcast = project(&db, "Podcast").await;
        for i in 0..=timers::MAX_RECENT as Ms {
            finished(
                &db,
                &podcast.id,
                (1_000 + i * 60) * MIN,
                &format!("n{i}"),
                None,
            )
            .await;
        }

        assert_eq!(
            recent(&db, u32::MAX).await.unwrap().len(),
            timers::MAX_RECENT
        );
    }

    #[tokio::test]
    async fn recent_timers_leave_out_archived_projects() {
        let db = Db::open_memory().await.unwrap();
        let kept = project(&db, "Kept").await;
        let old = project(&db, "Old").await;
        finished(&db, &kept.id, 1_000 * MIN, "", None).await;
        finished(&db, &old.id, 2_000 * MIN, "", None).await;
        projects::set_archived(&db, &old.id, true).await.unwrap();

        let listed: Vec<String> = recent(&db, 8)
            .await
            .unwrap()
            .into_iter()
            .map(|t| t.project)
            .collect();

        assert_eq!(listed, ["Kept"]);
    }

    #[test]
    fn recent_timers_serialize_as_a_list_with_camel_case_fields() {
        assert_eq!(
            serde_json::to_value(vec![RecentTimerEntry {
                project_id: "p1".into(),
                project: "Portal".into(),
                client: Some("Acme".into()),
                tag_id: Some("t1".into()),
                tag: Some("dev".into()),
                note: "standup".into(),
                last_used_at: 1_000,
            }])
            .unwrap(),
            json!([{
                "projectId": "p1",
                "project": "Portal",
                "client": "Acme",
                "tagId": "t1",
                "tag": "dev",
                "note": "standup",
                "lastUsedAt": 1_000
            }])
        );
    }

    #[tokio::test]
    async fn without_a_running_timer_everything_but_running_is_none() {
        let db = Db::open_memory().await.unwrap();
        let podcast = project(&db, "Podcast").await;
        recording(&db, &podcast.id, 1_000, Some(2_000)).await;

        let state = state(&db, 5_000).await.unwrap();

        assert_eq!(state, TimerState::default());
        assert!(!state.running);
    }

    #[tokio::test]
    async fn a_running_timer_names_its_project_client_tag_and_note() {
        let db = Db::open_memory().await.unwrap();
        let acme = client(&db, "Acme").await;
        let portal = projects::insert(
            &db,
            ProjectInput {
                client_id: Some(acme.id),
                name: "Portal".into(),
                color: "blue".into(),
                shade: None,
                billable: true,
            },
        )
        .await
        .unwrap();
        let billing = tags::insert(&db, "billing").await.unwrap();
        let id = timer_of(&db, &portal.id, "standup", Some(&billing.id)).await;
        let started = 2_000 * MIN;

        let state = state(&db, started + 25 * MIN).await.unwrap();

        assert_eq!(
            state,
            TimerState {
                running: true,
                recording_id: Some(id),
                project_id: Some(portal.id),
                project: Some("Portal".into()),
                client: Some("Acme".into()),
                tag_id: Some(billing.id),
                tag: Some("billing".into()),
                note: Some("standup".into()),
                started_at: Some(started),
                started_at_iso: Some(to_local_iso(started)),
                elapsed_ms: Some(25 * MIN),
            }
        );
    }

    #[tokio::test]
    async fn the_timer_counts_from_its_open_span_not_from_the_first_one() {
        let db = Db::open_memory().await.unwrap();
        let podcast = project(&db, "Podcast").await;
        timer_of(&db, &podcast.id, "", None).await;

        let state = state(&db, 2_000 * MIN + 90 * MIN).await.unwrap();

        assert_eq!(state.started_at, Some(2_000 * MIN));
        assert_eq!(state.elapsed_ms, Some(90 * MIN));
    }

    #[tokio::test]
    async fn a_timer_without_note_client_or_tag_has_an_empty_note_and_no_labels() {
        let db = Db::open_memory().await.unwrap();
        let podcast = project(&db, "Podcast").await;
        timer_of(&db, &podcast.id, "", None).await;

        let state = state(&db, 2_000 * MIN).await.unwrap();

        assert_eq!(
            (state.note, state.client, state.tag, state.tag_id),
            (Some(String::new()), None, None, None)
        );
    }

    #[tokio::test]
    async fn the_elapsed_time_is_never_negative() {
        let db = Db::open_memory().await.unwrap();
        let podcast = project(&db, "Podcast").await;
        timer_of(&db, &podcast.id, "", None).await;

        let state = state(&db, 2_000 * MIN - 5 * MIN).await.unwrap();

        assert_eq!(state.elapsed_ms, Some(0));
    }

    #[test]
    fn the_state_has_camel_case_fields_and_nulls_when_idle() {
        assert_eq!(
            serde_json::to_value(TimerState::default()).unwrap(),
            json!({
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
            })
        );
    }
}
