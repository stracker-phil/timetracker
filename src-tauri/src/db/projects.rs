use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use ts_rs::TS;

use super::time::{Ms, now_ms};
use super::{Db, clients, new_id, required_name};
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

/// The input with a trimmed name, after checking the color, the rate and that the client exists.
async fn validated(db: &Db, input: ProjectInput) -> AppResult<ProjectInput> {
    let name = required_name(&input.name, "A project")?;
    if !ACCENTS.contains(&input.color.as_str()) {
        return Err(AppError::InvalidInput(format!(
            "Unknown project color {}",
            input.color
        )));
    }
    if input.hourly_rate.is_some_and(|r| !r.is_finite() || r < 0.0) {
        return Err(AppError::InvalidInput(
            "The hourly rate must be zero or more".into(),
        ));
    }
    if let Some(client_id) = &input.client_id {
        clients::get(db, client_id).await?;
    }
    Ok(ProjectInput { name, ..input })
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
    let input = validated(db, input).await?;
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

pub async fn update(db: &Db, id: &str, input: ProjectInput) -> AppResult<Project> {
    let input = validated(db, input).await?;
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

/// Active projects starting with `prefix`, ignoring case beyond ASCII (SQLite `NOCASE` folds ASCII only).
pub async fn find_by_name_prefix(db: &Db, prefix: &str) -> AppResult<Vec<Project>> {
    let prefix = prefix.trim().to_lowercase();
    let mut projects = list(db, false).await?;
    projects.retain(|p| p.name.to_lowercase().starts_with(&prefix));
    Ok(projects)
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
    async fn prefix_search_ignores_case_including_non_ascii_and_skips_archived() {
        let db = Db::open_memory().await.unwrap();
        for name in ["Ärger", "arbeit", "Ärmel", "Other"] {
            insert(&db, input(name)).await.unwrap();
        }
        let gone = insert(&db, input("Ärztin")).await.unwrap();
        set_archived(&db, &gone.id, true).await.unwrap();

        for (prefix, expected) in [
            ("är", vec!["Ärger", "Ärmel"]),
            ("ÄR", vec!["Ärger", "Ärmel"]),
            (" ar ", vec!["arbeit"]),
            ("zzz", vec![]),
        ] {
            assert_eq!(
                names(find_by_name_prefix(&db, prefix).await.unwrap()),
                expected,
                "{prefix:?}"
            );
        }
    }
}
