//! The timer that runs: a recording with an open span (started final), or a stored suggestion with an open
//! start (started by another app, ADR 0041). At most one timer runs.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::db::Db;
use crate::db::recordings::{self, Recording};
use crate::db::stored_suggestions::{self, StoredSuggestion};
use crate::db::time::Ms;
use crate::error::AppResult;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum Timer {
    Recording(Recording),
    Suggestion(StoredSuggestion),
}

impl Timer {
    /// The recording's or the stored suggestion's id.
    pub fn id(&self) -> &str {
        match self {
            Timer::Recording(r) => &r.id,
            Timer::Suggestion(s) => &s.id,
        }
    }

    /// Where the open span starts.
    pub fn open_start(&self) -> Ms {
        match self {
            Timer::Recording(r) => open_start(r),
            Timer::Suggestion(s) => s.open_start.or_else(|| s.start()).unwrap_or(0),
        }
    }

    pub fn project_id(&self) -> Option<&str> {
        match self {
            Timer::Recording(r) => Some(&r.project_id),
            Timer::Suggestion(s) => s.project_id.as_deref(),
        }
    }

    pub fn note(&self) -> &str {
        match self {
            Timer::Recording(r) => &r.note,
            Timer::Suggestion(s) => &s.note,
        }
    }

    pub fn tag_id(&self) -> Option<&str> {
        match self {
            Timer::Recording(r) => r.tag_id.as_deref(),
            Timer::Suggestion(s) => s.tag_id.as_deref(),
        }
    }

    /// Whether it runs as a suggestion that waits for the owner's review.
    pub fn is_suggested(&self) -> bool {
        matches!(self, Timer::Suggestion(_))
    }

    /// The app that started a suggested timer.
    pub fn source(&self) -> Option<&str> {
        match self {
            Timer::Recording(_) => None,
            Timer::Suggestion(s) => Some(&s.source),
        }
    }
}

/// Where the running recording's open span starts.
pub fn open_start(running: &Recording) -> Ms {
    running.open_span().map_or(running.start(), |s| s.start)
}

/// The running timer: the running recording, else the running stored suggestion.
pub async fn current(db: &Db) -> AppResult<Option<Timer>> {
    if let Some(recording) = recordings::running(db).await? {
        return Ok(Some(Timer::Recording(recording)));
    }
    Ok(stored_suggestions::running(db)
        .await?
        .map(Timer::Suggestion))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::db::recordings::Span;
    use crate::db::time::Day;
    use crate::test_support::{project, recording};

    const T0: Ms = 1_781_517_600_000;
    const MIN: Ms = 60_000;

    fn running_suggestion(project_id: Option<&str>, open_start: Ms) -> StoredSuggestion {
        StoredSuggestion {
            id: "s1".into(),
            project_id: project_id.map(str::to_string),
            note: "review".into(),
            tag_id: None,
            source: "claude".into(),
            day: Day::of(open_start),
            spans: vec![Span {
                start: open_start - 30 * MIN,
                end: open_start - 10 * MIN,
            }],
            created_at: 1_000,
            updated_at: 2_000,
            open_start: Some(open_start),
        }
    }

    async fn put(db: &Db, suggestion: &StoredSuggestion) {
        let mut conn = db.pool().acquire().await.unwrap();
        stored_suggestions::put_on(&mut conn, suggestion)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn no_timer_runs_when_nothing_is_open() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "Acme").await;
        recording(&db, &p.id, T0, Some(T0 + MIN)).await;

        assert_eq!(current(&db).await.unwrap(), None);
    }

    #[tokio::test]
    async fn the_running_recording_is_the_timer() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "Acme").await;
        let running = recording(&db, &p.id, T0, None).await;

        let timer = current(&db).await.unwrap().unwrap();

        assert_eq!(timer, Timer::Recording(running.clone()));
        assert_eq!(
            (timer.id(), timer.open_start(), timer.is_suggested()),
            (running.id.as_str(), T0, false)
        );
        assert_eq!(timer.source(), None);
        assert_eq!(timer.project_id(), Some(p.id.as_str()));
    }

    #[tokio::test]
    async fn the_running_suggestion_is_the_timer_with_its_source_and_open_start() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "Acme").await;
        let suggestion = running_suggestion(Some(&p.id), T0);
        put(&db, &suggestion).await;

        let timer = current(&db).await.unwrap().unwrap();

        assert_eq!(timer, Timer::Suggestion(suggestion));
        assert_eq!(
            (timer.id(), timer.open_start(), timer.is_suggested()),
            ("s1", T0, true)
        );
        assert_eq!(timer.source(), Some("claude"));
        assert_eq!(
            (timer.project_id(), timer.note(), timer.tag_id()),
            (Some(p.id.as_str()), "review", None)
        );
    }

    #[tokio::test]
    async fn a_running_recording_comes_before_a_running_suggestion() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "Acme").await;
        put(&db, &running_suggestion(Some(&p.id), T0)).await;
        let running = recording(&db, &p.id, T0 + MIN, None).await;

        assert_eq!(current(&db).await.unwrap(), Some(Timer::Recording(running)));
    }

    #[tokio::test]
    async fn a_stopped_suggestion_is_no_timer() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "Acme").await;
        put(
            &db,
            &StoredSuggestion {
                open_start: None,
                ..running_suggestion(Some(&p.id), T0)
            },
        )
        .await;

        assert_eq!(current(&db).await.unwrap(), None);
    }

    #[test]
    fn a_suggestion_without_a_project_has_no_project_id() {
        assert_eq!(
            Timer::Suggestion(running_suggestion(None, T0)).project_id(),
            None
        );
    }

    #[test]
    fn a_timer_serializes_its_kind_next_to_the_fields_of_the_recording_or_suggestion() {
        let suggestion = Timer::Suggestion(running_suggestion(Some("p1"), T0));

        assert_eq!(
            serde_json::to_value(&suggestion).unwrap(),
            json!({
                "kind": "suggestion",
                "id": "s1",
                "projectId": "p1",
                "note": "review",
                "tagId": null,
                "source": "claude",
                "day": Day::of(T0).to_string(),
                "spans": [{"start": T0 - 30 * MIN, "end": T0 - 10 * MIN}],
                "createdAt": 1_000,
                "updatedAt": 2_000,
                "openStart": T0,
            })
        );
        assert_eq!(
            serde_json::from_value::<Timer>(serde_json::to_value(&suggestion).unwrap()).unwrap(),
            suggestion
        );
    }
}
