use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use ts_rs::TS;

use super::time::{Ms, now_ms};
use super::{Db, name_in_use, name_taken, new_id, required_name};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Client {
    pub id: String,
    pub name: String,
    pub archived_at: Option<Ms>,
    pub created_at: Ms,
}

pub async fn list(db: &Db, include_archived: bool) -> AppResult<Vec<Client>> {
    Ok(sqlx::query_as(
        "SELECT * FROM clients WHERE ?1 OR archived_at IS NULL ORDER BY name COLLATE NOCASE",
    )
    .bind(include_archived)
    .fetch_all(db.pool())
    .await?)
}

pub async fn get(db: &Db, id: &str) -> AppResult<Client> {
    found(
        id,
        sqlx::query_as("SELECT * FROM clients WHERE id = ?1")
            .bind(id)
            .fetch_optional(db.pool())
            .await?,
    )
}

pub async fn insert(db: &Db, name: &str) -> AppResult<Client> {
    let name = unique_name(db, name, None).await?;
    Ok(
        sqlx::query_as(
            "INSERT INTO clients (id, name, created_at) VALUES (?1, ?2, ?3) RETURNING *",
        )
        .bind(new_id())
        .bind(name)
        .bind(now_ms())
        .fetch_one(db.pool())
        .await?,
    )
}

pub async fn update(db: &Db, id: &str, name: &str) -> AppResult<Client> {
    let name = unique_name(db, name, Some(id)).await?;
    found(
        id,
        sqlx::query_as("UPDATE clients SET name = ?2 WHERE id = ?1 RETURNING *")
            .bind(id)
            .bind(name)
            .fetch_optional(db.pool())
            .await?,
    )
}

/// The trimmed name, unless another client (archived ones too) has it.
async fn unique_name(db: &Db, name: &str, except: Option<&str>) -> AppResult<String> {
    let name = required_name(name)?;
    let clients = list(db, true).await?;
    if name_in_use(
        clients.iter().map(|c| (c.id.as_str(), c.name.as_str())),
        &name,
        except,
    ) {
        return Err(name_taken("A client"));
    }
    Ok(name)
}

/// Archiving also archives the client's active projects; unarchiving restores the client only.
pub async fn set_archived(db: &Db, id: &str, archived: bool) -> AppResult<Client> {
    let now = now_ms();
    let mut tx = db.pool().begin().await?;
    let client = found(
        id,
        sqlx::query_as(
            "UPDATE clients SET archived_at = CASE WHEN ?2 THEN coalesce(archived_at, ?3) END
             WHERE id = ?1 RETURNING *",
        )
        .bind(id)
        .bind(archived)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await?,
    )?;
    if archived {
        sqlx::query(
            "UPDATE projects SET archived_at = ?2 WHERE client_id = ?1 AND archived_at IS NULL",
        )
        .bind(id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(client)
}

pub(super) async fn all(conn: &mut SqliteConnection) -> AppResult<Vec<Client>> {
    Ok(
        sqlx::query_as("SELECT * FROM clients ORDER BY created_at, id")
            .fetch_all(conn)
            .await?,
    )
}

fn found(id: &str, client: Option<Client>) -> AppResult<Client> {
    client.ok_or_else(|| AppError::NotFound(format!("No client {id}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;

    #[tokio::test]
    async fn insert_trims_the_name() {
        let db = Db::open_memory().await.unwrap();
        let client = insert(&db, "  Acme  ").await.unwrap();
        assert_eq!(client.name, "Acme");
        assert_eq!(client.archived_at, None);
        assert_eq!(get(&db, &client.id).await.unwrap(), client);
    }

    fn name_error(err: AppError) -> String {
        match err {
            AppError::InvalidField { field, message } => {
                assert_eq!(field, "name");
                message
            }
            other => panic!("expected an invalid field, got {other:?}"),
        }
    }

    async fn project_of(db: &Db, client_id: &str, name: &str) -> crate::db::projects::Project {
        crate::db::projects::insert(
            db,
            crate::db::projects::ProjectInput {
                client_id: Some(client_id.into()),
                name: name.into(),
                color: "blue".into(),
                billable: true,
                hourly_rate: None,
            },
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn blank_names_are_rejected_on_the_name_field() {
        let db = Db::open_memory().await.unwrap();
        let existing = insert(&db, "Acme").await.unwrap();
        for name in ["", "   "] {
            let err = insert(&db, name).await.unwrap_err();
            assert_eq!(name_error(err), "must not be empty", "insert {name:?}");
            let err = update(&db, &existing.id, name).await.unwrap_err();
            assert_eq!(name_error(err), "must not be empty", "update {name:?}");
        }
    }

    #[tokio::test]
    async fn names_over_the_limit_are_rejected_and_the_limit_itself_is_fine() {
        let db = Db::open_memory().await.unwrap();
        let existing = insert(&db, "x".repeat(crate::db::MAX_NAME_CHARS).as_str())
            .await
            .unwrap();
        let too_long = "y".repeat(crate::db::MAX_NAME_CHARS + 1);
        let err = insert(&db, &too_long).await.unwrap_err();
        assert_eq!(name_error(err), "must be at most 100 characters");
        let err = update(&db, &existing.id, &too_long).await.unwrap_err();
        assert_eq!(name_error(err), "must be at most 100 characters");
    }

    #[tokio::test]
    async fn names_count_characters_not_bytes() {
        let db = Db::open_memory().await.unwrap();
        let name = "ä".repeat(crate::db::MAX_NAME_CHARS);
        assert_eq!(insert(&db, &name).await.unwrap().name, name);
    }

    #[tokio::test]
    async fn a_name_another_client_has_is_refused_ignoring_case_and_archive_state() {
        let db = Db::open_memory().await.unwrap();
        insert(&db, "Acme").await.unwrap();
        insert(&db, "Äpfel").await.unwrap();
        let old = insert(&db, "Globex").await.unwrap();
        set_archived(&db, &old.id, true).await.unwrap();
        let other = insert(&db, "Initech").await.unwrap();

        for name in ["Acme", "acme", " ACME ", "äpfel", "ÄPFEL", "globex"] {
            let err = insert(&db, name).await.unwrap_err();
            assert_eq!(
                name_error(err),
                "A client with this name already exists",
                "insert {name:?}"
            );
            let err = update(&db, &other.id, name).await.unwrap_err();
            assert_eq!(
                name_error(err),
                "A client with this name already exists",
                "update {name:?}"
            );
        }
        assert_eq!(list(&db, true).await.unwrap().len(), 4);
    }

    #[tokio::test]
    async fn a_client_can_be_renamed_to_its_own_name_in_another_case() {
        let db = Db::open_memory().await.unwrap();
        let client = insert(&db, "acme").await.unwrap();
        assert_eq!(update(&db, &client.id, "Acme").await.unwrap().name, "Acme");
        assert_eq!(update(&db, &client.id, "Acme").await.unwrap().name, "Acme");
    }

    #[tokio::test]
    async fn unknown_ids_are_not_found() {
        let db = Db::open_memory().await.unwrap();
        assert_eq!(
            get(&db, "nope").await.unwrap_err().kind(),
            ErrorKind::NotFound
        );
        assert_eq!(
            update(&db, "nope", "x").await.unwrap_err().kind(),
            ErrorKind::NotFound
        );
        assert_eq!(
            set_archived(&db, "nope", true).await.unwrap_err().kind(),
            ErrorKind::NotFound
        );
    }

    #[tokio::test]
    async fn update_renames_the_client() {
        let db = Db::open_memory().await.unwrap();
        let client = insert(&db, "Acme").await.unwrap();
        let renamed = update(&db, &client.id, "Globex").await.unwrap();
        assert_eq!(
            renamed,
            Client {
                name: "Globex".into(),
                ..client
            }
        );
    }

    #[tokio::test]
    async fn list_hides_archived_clients_unless_asked_and_sorts_ignoring_case() {
        let db = Db::open_memory().await.unwrap();
        insert(&db, "banana").await.unwrap();
        insert(&db, "Cherry").await.unwrap();
        insert(&db, "apple").await.unwrap();
        let old = insert(&db, "Berry").await.unwrap();
        set_archived(&db, &old.id, true).await.unwrap();

        let names = |clients: Vec<Client>| clients.into_iter().map(|c| c.name).collect::<Vec<_>>();
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
    async fn archiving_keeps_the_first_timestamp_and_unarchiving_clears_it() {
        let db = Db::open_memory().await.unwrap();
        let client = insert(&db, "Acme").await.unwrap();

        let first = set_archived(&db, &client.id, true).await.unwrap();
        let first_at = first.archived_at.expect("archived");
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let second = set_archived(&db, &client.id, true).await.unwrap();
        assert_eq!(second.archived_at, Some(first_at));

        let restored = set_archived(&db, &client.id, false).await.unwrap();
        assert_eq!(restored.archived_at, None);
    }

    #[tokio::test]
    async fn archiving_a_client_archives_its_active_projects_only() {
        let db = Db::open_memory().await.unwrap();
        let client = insert(&db, "Acme").await.unwrap();
        let other_client = insert(&db, "Globex").await.unwrap();
        let active = project_of(&db, &client.id, "Active").await;
        let earlier = project_of(&db, &client.id, "Earlier").await;
        let elsewhere = project_of(&db, &other_client.id, "Elsewhere").await;
        let earlier_at = crate::db::projects::set_archived(&db, &earlier.id, true)
            .await
            .unwrap()
            .archived_at
            .expect("archived");
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;

        let archived = set_archived(&db, &client.id, true).await.unwrap();

        let get = |id: String| {
            let db = db.clone();
            async move { crate::db::projects::get(&db, &id).await.unwrap() }
        };
        assert_eq!(get(active.id).await.archived_at, archived.archived_at);
        assert_eq!(get(earlier.id).await.archived_at, Some(earlier_at));
        assert_eq!(get(elsewhere.id).await.archived_at, None);
    }

    #[tokio::test]
    async fn unarchiving_a_client_leaves_its_projects_archived() {
        let db = Db::open_memory().await.unwrap();
        let client = insert(&db, "Acme").await.unwrap();
        let project = project_of(&db, &client.id, "P").await;
        set_archived(&db, &client.id, true).await.unwrap();

        let restored = set_archived(&db, &client.id, false).await.unwrap();

        assert_eq!(restored.archived_at, None);
        assert!(
            crate::db::projects::get(&db, &project.id)
                .await
                .unwrap()
                .archived_at
                .is_some()
        );
    }
}
