//! `timetracker record`, `edit`, `delete` and `approve`: suggestions unless final (ADR 0028). An id names a
//! recording, a stored suggestion (also as one of its pieces, `stored:<id>:<start>:<end>`) or a rule's
//! suggestion (`<rule id>:<start>:<end>`); changing a recording needs `final`.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::db::recordings::{self as stored_recordings, Recording, RecordingPatch};
use crate::db::stored_suggestions::{self, StoredSuggestion};
use crate::db::time::{Day, Ms};
use crate::db::{Db, projects, tags};
use crate::error::{AppError, AppResult};
use crate::recordings::cli::{RecordRequest, Recorded, record as record_final};
use crate::recordings::commands::changing;
use crate::recordings::{Overlap, RecordingInput, Span};
use crate::rules;
use crate::state::AppState;

/// The source of suggestions the CLI stores without `--source`.
pub const CLI_SOURCE: &str = "cli";

/// A stored suggestion with the project name the CLI prints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggested {
    #[serde(flatten)]
    pub suggestion: StoredSuggestion,
    pub project_name: String,
}

/// What a write left: a recording or a stored suggestion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Written {
    Recording(Recorded),
    Suggestion(Suggested),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeletedKind {
    Recording,
    Suggestion,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Deleted {
    pub kind: DeletedKind,
    pub id: String,
}

/// `timetracker edit`: what changes; absent fields stay. The project by id, name or unique prefix, the tag
/// by name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditRequest {
    pub id: String,
    pub project: Option<String>,
    pub note: Option<String>,
    pub tag: Option<String>,
    /// Removes the tag; `tag` wins when both are given.
    #[serde(default)]
    pub clear_tag: bool,
    /// Replace every span.
    pub spans: Option<Vec<Span>>,
    /// Change a recording, or record a suggestion with the changes.
    #[serde(default, rename = "final")]
    pub final_write: bool,
    pub overlap: Overlap,
}

/// What an id names.
enum Target {
    Recording(Recording),
    Stored(StoredSuggestion),
    /// A rule's suggestion on its day.
    Rule {
        day: Day,
        id: String,
    },
}

/// `<rule id>:<start>:<end>`'s day.
fn rule_piece_day(id: &str) -> Option<Day> {
    let mut parts = id.rsplitn(3, ':');
    let _end: Ms = parts.next()?.parse().ok()?;
    let start: Ms = parts.next()?.parse().ok()?;
    parts.next()?;
    Some(Day::of(start))
}

async fn target(db: &Db, id: &str) -> AppResult<Target> {
    if let Some(rest) = id.strip_prefix("stored:") {
        let stored_id = rest.split_once(':').map_or(rest, |(id, _)| id);
        return Ok(Target::Stored(
            stored_suggestions::get(db, stored_id).await?,
        ));
    }
    if let Some(day) = rule_piece_day(id) {
        return Ok(Target::Rule {
            day,
            id: id.to_string(),
        });
    }
    match stored_suggestions::get(db, id).await {
        Ok(suggestion) => return Ok(Target::Stored(suggestion)),
        Err(AppError::NotFound(_)) => {}
        Err(e) => return Err(e),
    }
    stored_recordings::get(db, id)
        .await
        .map(Target::Recording)
        .map_err(|e| match e {
            AppError::NotFound(_) => {
                AppError::NotFound(format!("No recording or suggestion `{id}`"))
            }
            e => e,
        })
}

fn needs_final(id: &str) -> AppError {
    AppError::InvalidInput(format!("`{id}` is a recording; add --final to change it"))
}

/// The id of the first piece of `suggestion` the Day view shows.
async fn first_piece(app: &AppHandle, db: &Db, suggestion: &StoredSuggestion) -> AppResult<String> {
    let settings = app.state::<AppState>().settings.get();
    rules::suggestions_of(db, suggestion.day, &settings)
        .await?
        .into_iter()
        .find(|s| s.stored_id.as_deref() == Some(&suggestion.id))
        .map(|s| s.id)
        .ok_or_else(|| {
            AppError::InvalidInput(format!(
                "The time of suggestion `{}` is recorded already; delete it",
                suggestion.id
            ))
        })
}

async fn suggested(db: &Db, suggestion: StoredSuggestion) -> AppResult<Written> {
    let project_name = projects::get(db, &suggestion.project_id).await?.name;
    Ok(Written::Suggestion(Suggested {
        suggestion,
        project_name,
    }))
}

async fn recorded(db: &Db, recording: Recording) -> AppResult<Recorded> {
    let project_name = projects::get(db, &recording.project_id).await?.name;
    Ok(Recorded {
        recording,
        project_name,
    })
}

/// A suggestion unless `request.final_write`; then a recording, refused on overlap unless replacing.
pub async fn record(app: &AppHandle, request: RecordRequest) -> AppResult<Written> {
    if request.final_write {
        return Ok(Written::Recording(record_final(app, request).await?));
    }
    let db = app.state::<AppState>().data.db()?;
    let project = projects::resolve(&db, &request.project).await?;
    let tag_id = tags::resolve_optional_name(&db, request.tag.as_deref()).await?;
    let input = RecordingInput {
        project_id: project.id,
        spans: request.spans,
        note: request.note,
        tag_id,
    };
    let source = request.source.unwrap_or_else(|| CLI_SOURCE.into());
    let suggestion = changing(app, async |s| s.create_suggestion(input, &source).await).await?;
    suggested(&db, suggestion).await
}

async fn patch_of(db: &Db, request: EditRequest) -> AppResult<RecordingPatch> {
    let project_id = match &request.project {
        Some(project) => Some(projects::resolve(db, project).await?.id),
        None => None,
    };
    let tag_id = match (&request.tag, request.clear_tag) {
        (Some(tag), _) => Some(Some(tags::resolve_name(db, tag).await?.id)),
        (None, true) => Some(None),
        (None, false) => None,
    };
    Ok(RecordingPatch {
        project_id,
        spans: request.spans,
        note: request.note,
        tag_id,
    })
}

/// Records the suggestion `id` of `day` with `patch` applied, settling it.
async fn approve_patched(
    app: &AppHandle,
    day: Day,
    id: &str,
    patch: RecordingPatch,
    overlap: Overlap,
) -> AppResult<Recording> {
    let settings = app.state::<AppState>().settings.get();
    changing(app, async |s| {
        rules::approve_patched(s, &settings, day, id, patch, overlap).await
    })
    .await
}

pub async fn edit(app: &AppHandle, request: EditRequest) -> AppResult<Written> {
    let db = app.state::<AppState>().data.db()?;
    let (id, final_write, overlap) = (request.id.clone(), request.final_write, request.overlap);
    let patch = patch_of(&db, request).await?;
    let target = target(&db, &id).await?;
    if !final_write {
        return edit_suggestion(app, &db, target, &id, patch).await;
    }
    let recording = match target {
        Target::Recording(recording) => {
            changing(app, async |s| s.update(&recording.id, patch, overlap).await).await?
        }
        Target::Stored(suggestion) => {
            let piece = first_piece(app, &db, &suggestion).await?;
            approve_patched(app, suggestion.day, &piece, patch, overlap).await?
        }
        Target::Rule { day, id: piece } => {
            approve_patched(app, day, &piece, patch, overlap).await?
        }
    };
    Ok(Written::Recording(recorded(&db, recording).await?))
}

/// Without `final` only a stored suggestion changes.
async fn edit_suggestion(
    app: &AppHandle,
    db: &Db,
    target: Target,
    id: &str,
    patch: RecordingPatch,
) -> AppResult<Written> {
    match target {
        Target::Stored(suggestion) => {
            let edited = changing(app, async |s| {
                s.update_suggestion(&suggestion.id, patch).await
            })
            .await?;
            suggested(db, edited).await
        }
        Target::Recording(_) => Err(needs_final(id)),
        Target::Rule { .. } => Err(AppError::InvalidInput(
            "A rule's suggestion can't be edited; add --final to record it with the changes".into(),
        )),
    }
}

/// Deletes a stored suggestion or dismisses a rule's; deleting a recording needs `final`.
pub async fn delete(app: &AppHandle, id: &str, final_write: bool) -> AppResult<Deleted> {
    let db = app.state::<AppState>().data.db()?;
    let kind = match target(&db, id).await? {
        Target::Recording(recording) if final_write => {
            changing(app, async |s| s.delete(&recording.id).await).await?;
            DeletedKind::Recording
        }
        Target::Recording(_) => return Err(needs_final(id)),
        Target::Stored(suggestion) => {
            changing(app, async |s| s.delete_suggestion(&suggestion.id).await).await?;
            DeletedKind::Suggestion
        }
        Target::Rule { day, id: piece } => {
            let settings = app.state::<AppState>().settings.get();
            let pieces = [piece];
            changing(app, async |s| {
                rules::dismiss(s, &settings, day, &pieces).await
            })
            .await?;
            DeletedKind::Suggestion
        }
    };
    Ok(Deleted {
        kind,
        id: id.to_string(),
    })
}

/// Records each suggestion as it is, one undo step each; stops at the first that can't be recorded.
pub async fn approve(app: &AppHandle, ids: &[String]) -> AppResult<Vec<Recorded>> {
    let db = app.state::<AppState>().data.db()?;
    let settings = app.state::<AppState>().settings.get();
    let mut approved = Vec::new();
    for id in ids {
        let (day, piece) = match target(&db, id).await? {
            Target::Recording(_) => {
                return Err(AppError::InvalidInput(format!(
                    "`{id}` is a recording already"
                )));
            }
            Target::Stored(suggestion) => {
                let piece = first_piece(app, &db, &suggestion).await?;
                (suggestion.day, piece)
            }
            Target::Rule { day, id: piece } => (day, piece),
        };
        let recording = changing(app, async |s| {
            rules::approve(s, &settings, day, &piece).await
        })
        .await?;
        approved.push(recorded(&db, recording).await?);
    }
    Ok(approved)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio::sync::Mutex;

    use super::*;
    use crate::db::recordings::RecordingSpan;
    use crate::error::ErrorKind;
    use crate::recordings::Recordings;
    use crate::recordings::undo::UndoStack;
    use crate::test_support::{project, recording};

    fn nine() -> Ms {
        Day::parse("2026-01-05").unwrap().bounds().0 + 9 * 3_600_000
    }

    fn a_recording() -> Recording {
        Recording {
            id: "r1".into(),
            project_id: "p1".into(),
            note: "fix login".into(),
            tag_id: Some("t1".into()),
            created_at: 1_000,
            updated_at: 2_000,
            spans: vec![RecordingSpan {
                id: "s1".into(),
                start: 3_000,
                end: Some(4_000),
            }],
        }
    }

    fn a_stored_suggestion() -> StoredSuggestion {
        StoredSuggestion {
            id: "sg1".into(),
            project_id: "p1".into(),
            note: "standup".into(),
            tag_id: None,
            source: "planner".into(),
            day: Day::parse("2026-01-05").unwrap(),
            spans: vec![Span {
                start: 1_000,
                end: 2_000,
            }],
            created_at: 1_000,
            updated_at: 2_000,
        }
    }

    #[test]
    fn a_written_recording_is_tagged_and_carries_the_project_name_beside_the_recording() {
        let written = Written::Recording(Recorded {
            recording: a_recording(),
            project_name: "Portal".into(),
        });

        let wire = json!({
            "kind": "recording",
            "id": "r1", "projectId": "p1", "note": "fix login", "tagId": "t1",
            "createdAt": 1000, "updatedAt": 2000,
            "spans": [{ "id": "s1", "start": 3000, "end": 4000 }],
            "projectName": "Portal"
        });
        assert_eq!(serde_json::to_value(&written).unwrap(), wire);
        assert_eq!(serde_json::from_value::<Written>(wire).unwrap(), written);
    }

    #[test]
    fn a_written_suggestion_is_tagged_and_carries_the_project_name_beside_the_suggestion() {
        let written = Written::Suggestion(Suggested {
            suggestion: a_stored_suggestion(),
            project_name: "Portal".into(),
        });

        let wire = json!({
            "kind": "suggestion",
            "id": "sg1", "projectId": "p1", "note": "standup", "tagId": null,
            "source": "planner", "day": "2026-01-05",
            "spans": [{ "start": 1000, "end": 2000 }],
            "createdAt": 1000, "updatedAt": 2000,
            "projectName": "Portal"
        });
        assert_eq!(serde_json::to_value(&written).unwrap(), wire);
        assert_eq!(serde_json::from_value::<Written>(wire).unwrap(), written);
    }

    #[test]
    fn a_deleted_reply_names_its_kind_and_id() {
        for (kind, name) in [
            (DeletedKind::Recording, "recording"),
            (DeletedKind::Suggestion, "suggestion"),
        ] {
            let deleted = Deleted {
                kind,
                id: "x".into(),
            };
            assert_eq!(
                serde_json::to_value(&deleted).unwrap(),
                json!({ "kind": name, "id": "x" })
            );
        }
    }

    #[test]
    fn an_edit_request_without_final_or_clear_tag_changes_nothing_more() {
        let request: EditRequest =
            serde_json::from_value(json!({ "id": "x", "overlap": "replace" })).unwrap();

        assert_eq!(
            request,
            EditRequest {
                id: "x".into(),
                project: None,
                note: None,
                tag: None,
                clear_tag: false,
                spans: None,
                final_write: false,
                overlap: Overlap::Replace,
            }
        );
    }

    #[tokio::test]
    async fn an_id_names_a_stored_suggestion_a_rule_piece_or_a_recording() {
        let db = Db::open_memory().await.unwrap();
        let portal = project(&db, "Portal").await;
        let (start, end) = (nine(), nine() + 3_600_000);
        let undo = Mutex::new(UndoStack::default());
        let service = Recordings {
            db: &db,
            undo: &undo,
            snap_minutes: 5,
        };
        let input = RecordingInput {
            project_id: portal.id.clone(),
            spans: vec![Span { start, end }],
            note: "standup".into(),
            tag_id: None,
        };
        let stored = service
            .create_suggestion(input, "planner")
            .await
            .unwrap()
            .value;
        let existing = recording(&db, &portal.id, end, Some(end + 3_600_000)).await;
        let rule_piece = format!("rule-1:{start}:{end}");

        assert!(matches!(
            target(&db, &format!("stored:{}:{start}:{end}", stored.id)).await.unwrap(),
            Target::Stored(found) if found == stored
        ));
        assert!(matches!(
            target(&db, &stored.id).await.unwrap(),
            Target::Stored(found) if found == stored
        ));
        assert!(matches!(
            target(&db, &existing.id).await.unwrap(),
            Target::Recording(found) if found == existing
        ));
        assert!(matches!(
            target(&db, &rule_piece).await.unwrap(),
            Target::Rule { day, id } if day == Day::of(start) && id == rule_piece
        ));
    }

    #[tokio::test]
    async fn an_unknown_id_is_not_found_naming_it() {
        let db = Db::open_memory().await.unwrap();

        let err = target(&db, "nope").await.err().unwrap();

        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(err.to_string(), "No recording or suggestion `nope`");
    }

    #[tokio::test]
    async fn the_piece_of_a_missing_stored_suggestion_is_not_found() {
        let db = Db::open_memory().await.unwrap();

        let err = target(&db, "stored:gone:1000:2000").await.err().unwrap();

        assert_eq!(err.kind(), ErrorKind::NotFound);
    }
}
