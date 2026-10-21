use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use ts_rs::TS;

use super::clients::{self, Client};
use super::time::{Ms, now_ms};
use super::{Db, invalid_field, name_in_use, new_id, required_name, same_name};
use crate::error::{AppError, AppResult};
use crate::store::settings::ACCENTS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Project {
    pub id: String,
    pub client_id: Option<String>,
    pub name: String,
    /// An accent name (`ACCENTS`).
    pub color: String,
    pub billable: bool,
    pub hourly_rate: Option<f64>,
    pub archived_at: Option<Ms>,
    pub created_at: Ms,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ProjectInput {
    pub client_id: Option<String>,
    pub name: String,
    pub color: String,
    pub billable: bool,
    pub hourly_rate: Option<f64>,
}

/// A new project's color when none is given.
pub const DEFAULT_COLOR: &str = "orange";

/// The input as stored: trimmed name, default color. `existing` is the project being edited; its
/// client may stay archived.
async fn validated(
    db: &Db,
    input: ProjectInput,
    existing: Option<&Project>,
) -> AppResult<ProjectInput> {
    let name = required_name(&input.name)?;
    let color = match input.color.trim() {
        "" => DEFAULT_COLOR.to_string(),
        color => color.to_string(),
    };
    if !ACCENTS.contains(&color.as_str()) {
        return Err(invalid_field("color", "must be one of the accent colors"));
    }
    if input.hourly_rate.is_some_and(|r| !r.is_finite() || r < 0.0) {
        return Err(invalid_field("hourlyRate", "must be zero or more"));
    }
    let client_changed = existing.is_none_or(|p| p.client_id != input.client_id);
    if let Some(client_id) = input.client_id.as_deref()
        && client_changed
        && clients::get(db, client_id).await?.archived_at.is_some()
    {
        return Err(invalid_field("clientId", "is archived"));
    }
    let projects = list(db, true).await?;
    let siblings = projects
        .iter()
        .filter(|p| p.client_id == input.client_id)
        .map(|p| (p.id.as_str(), p.name.as_str()));
    if name_in_use(siblings, &name, existing.map(|e| e.id.as_str())) {
        let owner = if input.client_id.is_some() {
            "of this client"
        } else {
            "without a client"
        };
        return Err(invalid_field(
            "name",
            format!("is used by another project {owner}"),
        ));
    }
    Ok(ProjectInput {
        name,
        color,
        ..input
    })
}

pub async fn list(db: &Db, include_archived: bool) -> AppResult<Vec<Project>> {
    Ok(sqlx::query_as(
        "SELECT * FROM projects WHERE ?1 OR archived_at IS NULL ORDER BY name COLLATE NOCASE",
    )
    .bind(include_archived)
    .fetch_all(db.pool())
    .await?)
}

pub async fn get(db: &Db, id: &str) -> AppResult<Project> {
    found(
        id,
        sqlx::query_as("SELECT * FROM projects WHERE id = ?1")
            .bind(id)
            .fetch_optional(db.pool())
            .await?,
    )
}

pub async fn insert(db: &Db, input: ProjectInput) -> AppResult<Project> {
    let input = validated(db, input, None).await?;
    Ok(sqlx::query_as(
        "INSERT INTO projects (id, client_id, name, color, billable, hourly_rate, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) RETURNING *",
    )
    .bind(new_id())
    .bind(input.client_id)
    .bind(input.name)
    .bind(input.color)
    .bind(input.billable)
    .bind(input.hourly_rate)
    .bind(now_ms())
    .fetch_one(db.pool())
    .await?)
}

/// Keeps the archive state.
pub async fn update(db: &Db, id: &str, input: ProjectInput) -> AppResult<Project> {
    let existing = get(db, id).await?;
    let input = validated(db, input, Some(&existing)).await?;
    found(
        id,
        sqlx::query_as(
            "UPDATE projects SET client_id = ?2, name = ?3, color = ?4, billable = ?5, hourly_rate = ?6
             WHERE id = ?1 RETURNING *",
        )
        .bind(id)
        .bind(input.client_id)
        .bind(input.name)
        .bind(input.color)
        .bind(input.billable)
        .bind(input.hourly_rate)
        .fetch_optional(db.pool())
        .await?,
    )
}

/// Archiving keeps the first archive time; unarchiving clears it.
pub async fn set_archived(db: &Db, id: &str, archived: bool) -> AppResult<Project> {
    found(
        id,
        sqlx::query_as(
            "UPDATE projects SET archived_at = CASE WHEN ?2 THEN coalesce(archived_at, ?3) END
             WHERE id = ?1 RETURNING *",
        )
        .bind(id)
        .bind(archived)
        .bind(now_ms())
        .fetch_optional(db.pool())
        .await?,
    )
}

/// The project an id or a name stands for: an exact id; else an active project with exactly that name,
/// ignoring case; else the one active project whose name starts with it. Several matches are
/// `invalid_input` naming them.
pub async fn resolve(db: &Db, name_or_id: &str) -> AppResult<Project> {
    let text = name_or_id.trim();
    if let Some(project) = sqlx::query_as("SELECT * FROM projects WHERE id = ?1")
        .bind(text)
        .fetch_optional(db.pool())
        .await?
    {
        return Ok(project);
    }
    let active = list(db, false).await?;
    let exact: Vec<Project> = active
        .iter()
        .filter(|p| same_name(&p.name, text))
        .cloned()
        .collect();
    let candidates = if exact.is_empty() {
        let prefix = text.to_lowercase();
        active
            .into_iter()
            .filter(|p| !prefix.is_empty() && p.name.to_lowercase().starts_with(&prefix))
            .collect()
    } else {
        exact
    };
    match candidates.as_slice() {
        [] => Err(AppError::NotFound(format!("No project matches `{text}`"))),
        [project] => Ok(project.clone()),
        several => Err(AppError::InvalidInput(format!(
            "`{text}` matches several projects: {}",
            several
                .iter()
                .map(|p| format!("{} ({})", p.name, p.id))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// Active projects by their latest recording, the most recent first; projects never recorded are left out.
pub async fn recent(db: &Db, limit: u32) -> AppResult<Vec<Project>> {
    Ok(sqlx::query_as(
        "SELECT p.* FROM projects p JOIN recordings r ON r.project_id = p.id
         WHERE p.archived_at IS NULL GROUP BY p.id ORDER BY max(r.start) DESC, p.name LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(db.pool())
    .await?)
}

/// A project as the Projects screen lists it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ProjectListItem {
    pub project: Project,
    pub client: Option<Client>,
    /// The start of its latest recording.
    pub last_used: Option<Ms>,
    /// Recorded time in the 30 days before now; a running recording counts until now.
    pub recorded_ms_30d: i64,
}

const USAGE_WINDOW: Ms = 30 * 24 * 3_600_000;

/// Per project: the latest recording start and the time recorded in the window before `now`.
async fn usage_by_project(db: &Db, now: Ms) -> AppResult<HashMap<String, (Option<Ms>, i64)>> {
    let rows: Vec<(String, Option<Ms>, i64)> = sqlx::query_as(
        r#"SELECT project_id, max(start),
             coalesce(sum(max(0, min(coalesce("end", ?1), ?1) - max(start, ?2))), 0)
           FROM recordings GROUP BY project_id"#,
    )
    .bind(now)
    .bind(now - USAGE_WINDOW)
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, last_used, recorded)| (id, (last_used, recorded)))
        .collect())
}

pub async fn list_items(
    db: &Db,
    include_archived: bool,
    now: Ms,
) -> AppResult<Vec<ProjectListItem>> {
    let clients: HashMap<String, Client> = clients::list(db, true)
        .await?
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();
    let usage = usage_by_project(db, now).await?;
    Ok(list(db, include_archived)
        .await?
        .into_iter()
        .map(|project| {
            let (last_used, recorded_ms_30d) = usage.get(&project.id).copied().unwrap_or_default();
            ProjectListItem {
                client: project
                    .client_id
                    .as_ref()
                    .and_then(|id| clients.get(id))
                    .cloned(),
                project,
                last_used,
                recorded_ms_30d,
            }
        })
        .collect())
}

pub async fn count(db: &Db) -> AppResult<i64> {
    Ok(sqlx::query_scalar("SELECT count(*) FROM projects")
        .fetch_one(db.pool())
        .await?)
}

pub(super) async fn all(conn: &mut SqliteConnection) -> AppResult<Vec<Project>> {
    Ok(
        sqlx::query_as("SELECT * FROM projects ORDER BY created_at, id")
            .fetch_all(conn)
            .await?,
    )
}

fn found(id: &str, project: Option<Project>) -> AppResult<Project> {
    project.ok_or_else(|| AppError::NotFound(format!("No project {id}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;

    fn input(name: &str) -> ProjectInput {
        ProjectInput {
            client_id: None,
            name: name.into(),
            color: "blue".into(),
            billable: true,
            hourly_rate: None,
        }
    }

    fn names(projects: Vec<Project>) -> Vec<String> {
        projects.into_iter().map(|p| p.name).collect()
    }

    #[tokio::test]
    async fn insert_stores_every_field() {
        let db = Db::open_memory().await.unwrap();
        let client = clients::insert(&db, "Acme").await.unwrap();
        let project = insert(
            &db,
            ProjectInput {
                client_id: Some(client.id.clone()),
                name: "  Website ".into(),
                color: "teal".into(),
                billable: false,
                hourly_rate: Some(95.5),
            },
        )
        .await
        .unwrap();

        assert_eq!(project.client_id, Some(client.id));
        assert_eq!(project.name, "Website");
        assert_eq!(project.color, "teal");
        assert!(!project.billable);
        assert_eq!(project.hourly_rate, Some(95.5));
        assert_eq!(project.archived_at, None);
        assert_eq!(get(&db, &project.id).await.unwrap(), project);
        assert_eq!(count(&db).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn invalid_input_is_rejected() {
        let db = Db::open_memory().await.unwrap();
        let cases = [
            (
                "blank name",
                ProjectInput {
                    name: "  ".into(),
                    ..input("x")
                },
            ),
            (
                "unknown color",
                ProjectInput {
                    color: "chartreuse".into(),
                    ..input("x")
                },
            ),
            (
                "negative rate",
                ProjectInput {
                    hourly_rate: Some(-1.0),
                    ..input("x")
                },
            ),
            (
                "NaN rate",
                ProjectInput {
                    hourly_rate: Some(f64::NAN),
                    ..input("x")
                },
            ),
            (
                "infinite rate",
                ProjectInput {
                    hourly_rate: Some(f64::INFINITY),
                    ..input("x")
                },
            ),
        ];
        for (case, bad) in cases {
            let err = insert(&db, bad).await.unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "{case}");
        }
        assert_eq!(count(&db).await.unwrap(), 0);
    }

    fn field_of(err: AppError) -> String {
        match err {
            AppError::InvalidField { field, .. } => field,
            other => panic!("expected an invalid field, got {other:?}"),
        }
    }

    async fn in_client(db: &Db, client_id: &str, name: &str) -> Project {
        insert(
            db,
            ProjectInput {
                client_id: Some(client_id.into()),
                ..input(name)
            },
        )
        .await
        .unwrap()
    }

    async fn record(db: &Db, project_id: &str, start: Ms, end: Option<Ms>) {
        crate::test_support::recording(db, project_id, start, end).await;
    }

    #[tokio::test]
    async fn invalid_input_names_the_offending_field() {
        let db = Db::open_memory().await.unwrap();
        let cases = [
            (
                "blank name",
                ProjectInput {
                    name: "  ".into(),
                    ..input("x")
                },
                "name",
            ),
            (
                "name over the limit",
                ProjectInput {
                    name: "n".repeat(crate::db::MAX_NAME_CHARS + 1),
                    ..input("x")
                },
                "name",
            ),
            (
                "unknown color",
                ProjectInput {
                    color: "chartreuse".into(),
                    ..input("x")
                },
                "color",
            ),
            (
                "negative rate",
                ProjectInput {
                    hourly_rate: Some(-0.5),
                    ..input("x")
                },
                "hourlyRate",
            ),
            (
                "NaN rate",
                ProjectInput {
                    hourly_rate: Some(f64::NAN),
                    ..input("x")
                },
                "hourlyRate",
            ),
            (
                "infinite rate",
                ProjectInput {
                    hourly_rate: Some(f64::INFINITY),
                    ..input("x")
                },
                "hourlyRate",
            ),
        ];
        for (case, bad, field) in cases {
            let err = insert(&db, bad).await.unwrap_err();
            assert_eq!(field_of(err), field, "{case}");
        }
    }

    #[tokio::test]
    async fn an_empty_color_becomes_the_default_and_a_zero_rate_is_allowed() {
        let db = Db::open_memory().await.unwrap();
        for color in ["", "  "] {
            let project = insert(
                &db,
                ProjectInput {
                    color: color.into(),
                    hourly_rate: Some(0.0),
                    ..input(&format!("P{}", color.len()))
                },
            )
            .await
            .unwrap();
            assert_eq!(project.color, DEFAULT_COLOR, "{color:?}");
            assert_eq!(project.hourly_rate, Some(0.0));
        }
        assert_eq!(DEFAULT_COLOR, "orange");
    }

    #[tokio::test]
    async fn an_archived_client_is_refused_on_create_and_when_switching_to_it() {
        let db = Db::open_memory().await.unwrap();
        let old = clients::insert(&db, "Old").await.unwrap();
        clients::set_archived(&db, &old.id, true).await.unwrap();
        let project = insert(&db, input("P")).await.unwrap();

        let err = insert(
            &db,
            ProjectInput {
                client_id: Some(old.id.clone()),
                ..input("Q")
            },
        )
        .await
        .unwrap_err();
        assert_eq!(field_of(err), "clientId");

        let err = update(
            &db,
            &project.id,
            ProjectInput {
                client_id: Some(old.id),
                ..input("P")
            },
        )
        .await
        .unwrap_err();
        assert_eq!(field_of(err), "clientId");
    }

    #[tokio::test]
    async fn a_project_can_be_edited_while_it_keeps_its_archived_client() {
        let db = Db::open_memory().await.unwrap();
        let client = clients::insert(&db, "Acme").await.unwrap();
        let project = in_client(&db, &client.id, "P").await;
        clients::set_archived(&db, &client.id, true).await.unwrap();
        set_archived(&db, &project.id, false).await.unwrap();

        let updated = update(
            &db,
            &project.id,
            ProjectInput {
                client_id: Some(client.id.clone()),
                name: "Renamed".into(),
                ..input("x")
            },
        )
        .await
        .unwrap();

        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.client_id, Some(client.id));
    }

    #[tokio::test]
    async fn names_are_unique_per_client_ignoring_case_and_archive_state() {
        let db = Db::open_memory().await.unwrap();
        let acme = clients::insert(&db, "Acme").await.unwrap();
        let globex = clients::insert(&db, "Globex").await.unwrap();
        insert(&db, input("Äpfel")).await.unwrap();
        let archived = in_client(&db, &acme.id, "Web").await;
        set_archived(&db, &archived.id, true).await.unwrap();

        for (client_id, name) in [
            (None, "Äpfel"),
            (None, "äpfel"),
            (None, " ÄPFEL "),
            (Some(acme.id.clone()), "web"),
            (Some(acme.id.clone()), "WEB"),
        ] {
            let err = insert(
                &db,
                ProjectInput {
                    client_id: client_id.clone(),
                    ..input(name)
                },
            )
            .await
            .unwrap_err();
            assert_eq!(field_of(err), "name", "{client_id:?} {name:?}");
        }

        in_client(&db, &globex.id, "Web").await;
        insert(&db, input("Web")).await.unwrap();
    }

    #[tokio::test]
    async fn an_update_keeps_its_own_name_but_cannot_take_a_siblings_or_move_into_a_clash() {
        let db = Db::open_memory().await.unwrap();
        let acme = clients::insert(&db, "Acme").await.unwrap();
        let first = insert(&db, input("First")).await.unwrap();
        insert(&db, input("Second")).await.unwrap();
        in_client(&db, &acme.id, "First").await;

        let same = update(&db, &first.id, input("first")).await.unwrap();
        assert_eq!(same.name, "first");

        let err = update(&db, &first.id, input("SECOND")).await.unwrap_err();
        assert_eq!(field_of(err), "name");

        let err = update(
            &db,
            &first.id,
            ProjectInput {
                client_id: Some(acme.id),
                ..input("First")
            },
        )
        .await
        .unwrap_err();
        assert_eq!(field_of(err), "name");
    }

    #[tokio::test]
    async fn editing_an_archived_project_leaves_it_archived() {
        let db = Db::open_memory().await.unwrap();
        let project = insert(&db, input("P")).await.unwrap();
        let archived_at = set_archived(&db, &project.id, true)
            .await
            .unwrap()
            .archived_at;

        let updated = update(&db, &project.id, input("Renamed")).await.unwrap();

        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.archived_at, archived_at);
        assert!(archived_at.is_some());
    }

    #[tokio::test]
    async fn unknown_client_is_not_found() {
        let db = Db::open_memory().await.unwrap();
        let err = insert(
            &db,
            ProjectInput {
                client_id: Some("nope".into()),
                ..input("x")
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn unknown_project_is_not_found() {
        let db = Db::open_memory().await.unwrap();
        assert_eq!(
            get(&db, "nope").await.unwrap_err().kind(),
            ErrorKind::NotFound
        );
        assert_eq!(
            update(&db, "nope", input("x")).await.unwrap_err().kind(),
            ErrorKind::NotFound
        );
        assert_eq!(
            set_archived(&db, "nope", true).await.unwrap_err().kind(),
            ErrorKind::NotFound
        );
    }

    #[tokio::test]
    async fn update_replaces_all_fields() {
        let db = Db::open_memory().await.unwrap();
        let client = clients::insert(&db, "Acme").await.unwrap();
        let project = insert(&db, input("Old")).await.unwrap();

        let updated = update(
            &db,
            &project.id,
            ProjectInput {
                client_id: Some(client.id.clone()),
                name: "New".into(),
                color: "red".into(),
                billable: false,
                hourly_rate: Some(10.0),
            },
        )
        .await
        .unwrap();

        assert_eq!(
            updated,
            Project {
                client_id: Some(client.id),
                name: "New".into(),
                color: "red".into(),
                billable: false,
                hourly_rate: Some(10.0),
                ..project
            }
        );
    }

    #[tokio::test]
    async fn archiving_keeps_the_first_timestamp_and_unarchiving_clears_it() {
        let db = Db::open_memory().await.unwrap();
        let project = insert(&db, input("P")).await.unwrap();

        let first_at = set_archived(&db, &project.id, true)
            .await
            .unwrap()
            .archived_at
            .expect("archived");
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let again = set_archived(&db, &project.id, true).await.unwrap();
        assert_eq!(again.archived_at, Some(first_at));
        let restored = set_archived(&db, &project.id, false).await.unwrap();
        assert_eq!(restored.archived_at, None);
    }

    #[tokio::test]
    async fn list_hides_archived_projects_unless_asked_and_sorts_ignoring_case() {
        let db = Db::open_memory().await.unwrap();
        insert(&db, input("banana")).await.unwrap();
        insert(&db, input("Cherry")).await.unwrap();
        insert(&db, input("apple")).await.unwrap();
        let old = insert(&db, input("Berry")).await.unwrap();
        set_archived(&db, &old.id, true).await.unwrap();

        assert_eq!(
            names(list(&db, false).await.unwrap()),
            ["apple", "banana", "Cherry"]
        );
        assert_eq!(
            names(list(&db, true).await.unwrap()),
            ["apple", "banana", "Berry", "Cherry"]
        );
    }

    #[tokio::test]
    async fn resolve_finds_a_project_by_id_even_when_archived() {
        let db = Db::open_memory().await.unwrap();
        let project = insert(&db, input("P")).await.unwrap();
        assert_eq!(resolve(&db, &project.id).await.unwrap(), project);

        let archived = set_archived(&db, &project.id, true).await.unwrap();
        assert_eq!(resolve(&db, &project.id).await.unwrap(), archived);
    }

    #[tokio::test]
    async fn resolve_matches_a_name_ignoring_case_and_a_unique_prefix() {
        let db = Db::open_memory().await.unwrap();
        for name in ["Ärger", "Website", "Other"] {
            insert(&db, input(name)).await.unwrap();
        }

        for (text, expected) in [
            ("ärger", "Ärger"),
            ("ÄRGER", "Ärger"),
            ("  website ", "Website"),
            ("är", "Ärger"),
            ("ÄR", "Ärger"),
            ("web", "Website"),
            ("o", "Other"),
        ] {
            assert_eq!(resolve(&db, text).await.unwrap().name, expected, "{text:?}");
        }
    }

    #[tokio::test]
    async fn resolve_prefers_an_exact_name_over_longer_prefix_matches() {
        let db = Db::open_memory().await.unwrap();
        let web = insert(&db, input("Web")).await.unwrap();
        insert(&db, input("Website")).await.unwrap();

        assert_eq!(resolve(&db, "web").await.unwrap(), web);
    }

    #[tokio::test]
    async fn resolve_without_a_match_is_not_found() {
        let db = Db::open_memory().await.unwrap();
        insert(&db, input("Website")).await.unwrap();
        let gone = insert(&db, input("Archive")).await.unwrap();
        set_archived(&db, &gone.id, true).await.unwrap();

        for text in ["zzz", "archive", "arch", "", "   "] {
            let err = resolve(&db, text).await.unwrap_err();
            assert_eq!(err.kind(), ErrorKind::NotFound, "{text:?}");
        }
    }

    #[tokio::test]
    async fn resolve_with_several_matches_lists_the_candidates() {
        let db = Db::open_memory().await.unwrap();
        let alpha = insert(&db, input("Alpha")).await.unwrap();
        let alps = insert(&db, input("Alps")).await.unwrap();
        insert(&db, input("Beta")).await.unwrap();

        let err = resolve(&db, "al").await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(
            err.to_string(),
            format!(
                "`al` matches several projects: Alpha ({}), Alps ({})",
                alpha.id, alps.id
            )
        );
    }

    #[tokio::test]
    async fn the_same_name_under_two_clients_is_ambiguous() {
        let db = Db::open_memory().await.unwrap();
        let acme = clients::insert(&db, "Acme").await.unwrap();
        let globex = clients::insert(&db, "Globex").await.unwrap();
        in_client(&db, &acme.id, "Web").await;
        in_client(&db, &globex.id, "Web").await;

        let err = resolve(&db, "web").await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }

    #[tokio::test]
    async fn recent_lists_active_recorded_projects_by_latest_recording() {
        let db = Db::open_memory().await.unwrap();
        let first = insert(&db, input("First")).await.unwrap();
        let second = insert(&db, input("Second")).await.unwrap();
        insert(&db, input("Never")).await.unwrap();
        let archived = insert(&db, input("Archived")).await.unwrap();
        record(&db, &first.id, 1_000, Some(2_000)).await;
        record(&db, &second.id, 3_000, Some(4_000)).await;
        record(&db, &first.id, 5_000, Some(6_000)).await;
        record(&db, &archived.id, 7_000, Some(8_000)).await;
        set_archived(&db, &archived.id, true).await.unwrap();

        assert_eq!(names(recent(&db, 10).await.unwrap()), ["First", "Second"]);
        assert_eq!(names(recent(&db, 1).await.unwrap()), ["First"]);
        assert!(recent(&db, 0).await.unwrap().is_empty());
    }

    const DAY: Ms = 24 * 3_600_000;
    const HOUR: Ms = 3_600_000;

    #[tokio::test]
    async fn list_items_attach_clients_last_use_and_the_last_30_days() {
        let db = Db::open_memory().await.unwrap();
        let now = 100 * DAY;
        let window_start = now - 30 * DAY;
        let acme = clients::insert(&db, "Acme").await.unwrap();
        let busy = in_client(&db, &acme.id, "Busy").await;
        let old = insert(&db, input("Old")).await.unwrap();
        let running = insert(&db, input("Running")).await.unwrap();
        let unused = insert(&db, input("Unused")).await.unwrap();

        record(&db, &old.id, 10 * DAY, Some(10 * DAY + HOUR)).await;
        record(
            &db,
            &busy.id,
            window_start - HOUR,
            Some(window_start + 2 * HOUR),
        )
        .await;
        record(&db, &busy.id, 80 * DAY, Some(80 * DAY + HOUR / 2)).await;
        record(&db, &running.id, now - HOUR, None).await;

        let items = list_items(&db, false, now).await.unwrap();

        let summary: Vec<_> = items
            .iter()
            .map(|i| {
                (
                    i.project.id.as_str(),
                    i.client.as_ref().map(|c| c.id.as_str()),
                    i.last_used,
                    i.recorded_ms_30d,
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                (
                    busy.id.as_str(),
                    Some(acme.id.as_str()),
                    Some(80 * DAY),
                    2 * HOUR + HOUR / 2
                ),
                (old.id.as_str(), None, Some(10 * DAY), 0),
                (running.id.as_str(), None, Some(now - HOUR), HOUR),
                (unused.id.as_str(), None, None, 0),
            ]
        );
    }

    #[tokio::test]
    async fn list_items_show_archived_projects_and_clients_only_when_asked() {
        let db = Db::open_memory().await.unwrap();
        let client = clients::insert(&db, "Acme").await.unwrap();
        let project = in_client(&db, &client.id, "P").await;
        insert(&db, input("Active")).await.unwrap();
        let archived_client = clients::set_archived(&db, &client.id, true).await.unwrap();

        let active = list_items(&db, false, DAY).await.unwrap();
        let active_names: Vec<_> = active.iter().map(|i| i.project.name.as_str()).collect();
        assert_eq!(active_names, ["Active"]);

        let all = list_items(&db, true, DAY).await.unwrap();
        assert_eq!(all.len(), 2);
        let archived = all.iter().find(|i| i.project.id == project.id).unwrap();
        assert_eq!(archived.client, Some(archived_client));
        assert!(archived.project.archived_at.is_some());
    }

    #[tokio::test]
    async fn a_list_item_serializes_with_camel_case_fields() {
        let db = Db::open_memory().await.unwrap();
        let project = insert(
            &db,
            ProjectInput {
                hourly_rate: Some(50.0),
                ..input("P")
            },
        )
        .await
        .unwrap();
        record(&db, &project.id, 2 * HOUR, Some(3 * HOUR)).await;

        let items = list_items(&db, false, 4 * HOUR).await.unwrap();

        assert_eq!(
            serde_json::to_value(&items[0]).unwrap(),
            serde_json::json!({
                "project": {
                    "id": project.id,
                    "clientId": null,
                    "name": "P",
                    "color": "blue",
                    "billable": true,
                    "hourlyRate": 50.0,
                    "archivedAt": null,
                    "createdAt": project.created_at,
                },
                "client": null,
                "lastUsed": 2 * HOUR,
                "recordedMs30d": HOUR,
            })
        );
    }
}
