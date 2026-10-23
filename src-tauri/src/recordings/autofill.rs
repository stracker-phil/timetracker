//! What the editor's Autofill fills in from the note: the project and tag of an earlier recording with it.

use serde::Serialize;
use ts_rs::TS;

use crate::db::{Db, recordings};
use crate::error::AppResult;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct RecordingAutofill {
    pub project_id: String,
    pub tag_id: Option<String>,
}

/// The project and tag of the latest recording with the same note (trimmed, ignoring case); `None` for a
/// blank note or without one.
pub async fn autofill(db: &Db, note: &str) -> AppResult<Option<RecordingAutofill>> {
    let note = note.trim();
    if note.is_empty() {
        return Ok(None);
    }
    Ok(recordings::latest_with_note(db, note)
        .await?
        .map(|(project_id, tag_id)| RecordingAutofill { project_id, tag_id }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::projects;
    use crate::db::time::Ms;
    use crate::recordings::testing::*;
    use crate::recordings::{Overlap, RecordingInput};

    async fn save(w: &World, project_id: &str, note: &str, tag_id: Option<&str>, start: Ms) {
        let input = RecordingInput {
            project_id: project_id.into(),
            note: note.into(),
            tag_id: tag_id.map(str::to_string),
            ..w.input(start, start + HOUR)
        };
        w.service()
            .create(input, Overlap::Cancel)
            .await
            .expect("create recording");
    }

    fn filled(project_id: &str, tag_id: Option<&str>) -> Option<RecordingAutofill> {
        Some(RecordingAutofill {
            project_id: project_id.into(),
            tag_id: tag_id.map(str::to_string),
        })
    }

    #[tokio::test]
    async fn a_blank_note_fills_nothing() {
        let w = World::new().await;
        save(&w, &w.project.id, "", None, at(9, 0)).await;
        save(&w, &w.project.id, "code review", None, at(11, 0)).await;

        for note in ["", "   ", "\t\n"] {
            assert_eq!(autofill(&w.db, note).await.unwrap(), None, "{note:?}");
        }
    }

    #[tokio::test]
    async fn the_latest_recording_with_the_note_decides_project_and_tag() {
        let w = World::new().await;
        save(
            &w,
            &w.project.id,
            "code review",
            Some(&w.alpha.id),
            at(9, 0),
        )
        .await;
        save(&w, &w.other.id, "code review", Some(&w.beta.id), at(14, 0)).await;
        save(
            &w,
            &w.project.id,
            "code review",
            Some(&w.alpha.id),
            at(11, 0),
        )
        .await;

        assert_eq!(
            autofill(&w.db, "code review").await.unwrap(),
            filled(&w.other.id, Some(&w.beta.id))
        );
    }

    #[tokio::test]
    async fn the_note_matches_ignoring_case_and_surrounding_whitespace() {
        let w = World::new().await;
        save(&w, &w.other.id, "code review", Some(&w.alpha.id), at(9, 0)).await;

        for note in ["  Code review ", "CODE REVIEW", "code review"] {
            assert_eq!(
                autofill(&w.db, note).await.unwrap(),
                filled(&w.other.id, Some(&w.alpha.id)),
                "{note:?}"
            );
        }
    }

    #[tokio::test]
    async fn only_the_whole_note_matches() {
        let w = World::new().await;
        save(&w, &w.project.id, "code review", None, at(9, 0)).await;

        for note in ["code", "review", "code review today", "code  review"] {
            assert_eq!(autofill(&w.db, note).await.unwrap(), None, "{note:?}");
        }
    }

    #[tokio::test]
    async fn recordings_of_archived_projects_are_skipped() {
        let w = World::new().await;
        save(&w, &w.project.id, "standup", Some(&w.alpha.id), at(9, 0)).await;
        save(&w, &w.other.id, "standup", Some(&w.beta.id), at(14, 0)).await;
        projects::set_archived(&w.db, &w.other.id, true)
            .await
            .unwrap();

        assert_eq!(
            autofill(&w.db, "standup").await.unwrap(),
            filled(&w.project.id, Some(&w.alpha.id))
        );

        projects::set_archived(&w.db, &w.project.id, true)
            .await
            .unwrap();

        assert_eq!(autofill(&w.db, "standup").await.unwrap(), None);
    }

    #[tokio::test]
    async fn a_recording_without_a_tag_fills_no_tag() {
        let w = World::new().await;
        save(&w, &w.project.id, "standup", Some(&w.alpha.id), at(9, 0)).await;
        save(&w, &w.other.id, "standup", None, at(14, 0)).await;

        assert_eq!(
            autofill(&w.db, "standup").await.unwrap(),
            filled(&w.other.id, None)
        );
    }

    #[tokio::test]
    async fn the_latest_span_of_a_recording_orders_it() {
        let w = World::new().await;
        let input = RecordingInput {
            note: "planning".into(),
            ..input_of(&w, &[(at(8, 0), at(9, 0)), (at(16, 0), at(17, 0))])
        };
        w.service().create(input, Overlap::Cancel).await.unwrap();
        save(&w, &w.other.id, "planning", None, at(12, 0)).await;

        assert_eq!(
            autofill(&w.db, "planning").await.unwrap(),
            filled(&w.project.id, None)
        );
    }

    #[test]
    fn the_autofill_serializes_with_camel_case_names() {
        assert_eq!(
            serde_json::to_value(filled("p", Some("t")).unwrap()).unwrap(),
            serde_json::json!({"projectId": "p", "tagId": "t"})
        );
    }
}
