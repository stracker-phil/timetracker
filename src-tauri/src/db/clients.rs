use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use ts_rs::TS;

use super::time::{Ms, now_ms};
use super::{Db, new_id, required_name};
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
    Ok(
        sqlx::query_as(
            "INSERT INTO clients (id, name, created_at) VALUES (?1, ?2, ?3) RETURNING *",
        )
        .bind(new_id())
        .bind(required_name(name, "A client")?)
        .bind(now_ms())
        .fetch_one(db.pool())
        .await?,
    )
}

pub async fn update(db: &Db, id: &str, name: &str) -> AppResult<Client> {
    found(
        id,
        sqlx::query_as("UPDATE clients SET name = ?2 WHERE id = ?1 RETURNING *")
            .bind(id)
            .bind(required_name(name, "A client")?)
            .fetch_optional(db.pool())
            .await?,
    )
}

/// Archiving keeps the first archive time; unarchiving clears it.
pub async fn set_archived(db: &Db, id: &str, archived: bool) -> AppResult<Client> {
    found(
        id,
        sqlx::query_as(
            "UPDATE clients SET archived_at = CASE WHEN ?2 THEN coalesce(archived_at, ?3) END
             WHERE id = ?1 RETURNING *",
        )
        .bind(id)
        .bind(archived)
        .bind(now_ms())
        .fetch_optional(db.pool())
        .await?,
    )
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

    #[tokio::test]
    async fn blank_names_are_rejected() {
        let db = Db::open_memory().await.unwrap();
        let existing = insert(&db, "Acme").await.unwrap();
        for name in ["", "   "] {
            let err = insert(&db, name).await.unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "insert {name:?}");
            let err = update(&db, &existing.id, name).await.unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "update {name:?}");
        }
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
}
