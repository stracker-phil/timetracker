use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use ts_rs::TS;

use super::time::{Ms, now_ms};
use super::{Db, new_id, required_name};
use crate::error::{AppError, AppResult};

/// The kinds of work a new data store starts with (ADR 0018); the user can rename or delete them.
pub const DEFAULT_TAGS: &[&str] = &[
    "admin",
    "research",
    "development",
    "maintenance",
    "internal meeting",
    "external meeting",
    "support",
];

/// Set in `meta` once the defaults were written, so deleting them all does not bring them back.
const SEEDED_KEY: &str = "tags_seeded";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Tag {
    pub id: String,
    /// Unique, ignoring case.
    pub name: String,
    pub created_at: Ms,
}

pub async fn list(db: &Db) -> AppResult<Vec<Tag>> {
    Ok(
        sqlx::query_as("SELECT * FROM tags ORDER BY name COLLATE NOCASE")
            .fetch_all(db.pool())
            .await?,
    )
}

/// A name that exists already (in any case) returns the existing tag.
pub async fn insert(db: &Db, name: &str) -> AppResult<Tag> {
    let name = required_name(name, "A tag")?;
    let mut conn = db.pool().acquire().await?;
    insert_on(&mut conn, &name, now_ms()).await
}

async fn insert_on(conn: &mut SqliteConnection, name: &str, now: Ms) -> AppResult<Tag> {
    sqlx::query(
        "INSERT INTO tags (id, name, created_at) VALUES (?1, ?2, ?3) ON CONFLICT (name) DO NOTHING",
    )
    .bind(new_id())
    .bind(name)
    .bind(now)
    .execute(&mut *conn)
    .await?;
    Ok(sqlx::query_as("SELECT * FROM tags WHERE name = ?1")
        .bind(name)
        .fetch_one(&mut *conn)
        .await?)
}

/// `invalid_input` when another tag has the name.
pub async fn rename(db: &Db, id: &str, name: &str) -> AppResult<Tag> {
    let name = required_name(name, "A tag")?;
    let taken: Option<String> =
        sqlx::query_scalar("SELECT id FROM tags WHERE name = ?1 AND id != ?2")
            .bind(&name)
            .bind(id)
            .fetch_optional(db.pool())
            .await?;
    if taken.is_some() {
        return Err(AppError::InvalidInput(format!("A tag named {name} exists")));
    }
    sqlx::query_as("UPDATE tags SET name = ?2 WHERE id = ?1 RETURNING *")
        .bind(id)
        .bind(&name)
        .fetch_optional(db.pool())
        .await?
        .ok_or_else(|| not_found(id))
}

/// Also removes the tag from every recording.
pub async fn delete(db: &Db, id: &str) -> AppResult<()> {
    let deleted = sqlx::query("DELETE FROM tags WHERE id = ?1")
        .bind(id)
        .execute(db.pool())
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

/// Writes `DEFAULT_TAGS` once in the life of a data store.
pub async fn seed_defaults(db: &Db) -> AppResult<()> {
    let mut tx = db.pool().begin().await?;
    let seeded =
        sqlx::query("INSERT INTO meta (key, value) VALUES (?1, '1') ON CONFLICT (key) DO NOTHING")
            .bind(SEEDED_KEY)
            .execute(&mut *tx)
            .await?
            .rows_affected();
    if seeded == 1 {
        let now = now_ms();
        for name in DEFAULT_TAGS {
            insert_on(&mut tx, name, now).await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

pub(super) async fn all(conn: &mut SqliteConnection) -> AppResult<Vec<Tag>> {
    Ok(sqlx::query_as("SELECT * FROM tags ORDER BY created_at, id")
        .fetch_all(conn)
        .await?)
}

fn not_found(id: &str) -> AppError {
    AppError::NotFound(format!("No tag {id}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::recordings;
    use crate::error::ErrorKind;
    use crate::test_support::{project, recording};

    fn names(tags: Vec<Tag>) -> Vec<String> {
        tags.into_iter().map(|t| t.name).collect()
    }

    #[tokio::test]
    async fn a_fresh_store_lists_the_default_tags_by_name() {
        let db = Db::open_memory().await.unwrap();
        let mut expected = DEFAULT_TAGS.to_vec();
        expected.sort_by_key(|name| name.to_lowercase());
        assert_eq!(names(list(&db).await.unwrap()), expected);
    }

    #[tokio::test]
    async fn seeding_again_adds_nothing_even_after_deleting_defaults() {
        let db = Db::open_memory().await.unwrap();
        seed_defaults(&db).await.unwrap();
        assert_eq!(list(&db).await.unwrap().len(), DEFAULT_TAGS.len());

        let first = list(&db).await.unwrap().remove(0);
        delete(&db, &first.id).await.unwrap();
        seed_defaults(&db).await.unwrap();
        assert_eq!(list(&db).await.unwrap().len(), DEFAULT_TAGS.len() - 1);
    }

    #[tokio::test]
    async fn inserting_an_existing_name_in_another_case_returns_the_existing_tag() {
        let db = Db::open_memory().await.unwrap();
        let existing = insert(&db, "Billing").await.unwrap();
        let again = insert(&db, " billing ").await.unwrap();
        assert_eq!(again.id, existing.id);
        assert_eq!(list(&db).await.unwrap().len(), DEFAULT_TAGS.len() + 1);
    }

    #[tokio::test]
    async fn blank_names_are_rejected() {
        let db = Db::open_memory().await.unwrap();
        let tag = insert(&db, "x").await.unwrap();
        for name in ["", "  "] {
            assert_eq!(
                insert(&db, name).await.unwrap_err().kind(),
                ErrorKind::InvalidInput
            );
            assert_eq!(
                rename(&db, &tag.id, name).await.unwrap_err().kind(),
                ErrorKind::InvalidInput
            );
        }
    }

    #[tokio::test]
    async fn rename_changes_the_name() {
        let db = Db::open_memory().await.unwrap();
        let tag = insert(&db, "old").await.unwrap();
        let renamed = rename(&db, &tag.id, " new ").await.unwrap();
        assert_eq!(
            renamed,
            Tag {
                name: "new".into(),
                ..tag
            }
        );
    }

    #[tokio::test]
    async fn rename_to_another_tags_name_is_rejected_but_own_name_in_another_case_works() {
        let db = Db::open_memory().await.unwrap();
        let a = insert(&db, "alpha").await.unwrap();
        insert(&db, "beta").await.unwrap();

        let err = rename(&db, &a.id, "beta").await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidInput);

        let renamed = rename(&db, &a.id, "Alpha").await.unwrap();
        assert_eq!(renamed.name, "Alpha");
    }

    #[tokio::test]
    async fn unknown_tags_are_not_found() {
        let db = Db::open_memory().await.unwrap();
        assert_eq!(
            rename(&db, "nope", "x").await.unwrap_err().kind(),
            ErrorKind::NotFound
        );
        assert_eq!(
            delete(&db, "nope").await.unwrap_err().kind(),
            ErrorKind::NotFound
        );
    }

    #[tokio::test]
    async fn deleting_a_tag_removes_it_from_recordings() {
        let db = Db::open_memory().await.unwrap();
        let keep = insert(&db, "keep").await.unwrap();
        let drop = insert(&db, "drop").await.unwrap();
        let p = project(&db, "P").await;
        let rec = recording(&db, &p.id, 1_000, Some(2_000)).await;
        let patch = recordings::RecordingPatch {
            tag_ids: Some(vec![keep.id.clone(), drop.id.clone()]),
            ..Default::default()
        };
        recordings::update(&db, &rec.id, patch).await.unwrap();

        delete(&db, &drop.id).await.unwrap();

        assert_eq!(
            recordings::get(&db, &rec.id).await.unwrap().tag_ids,
            [keep.id]
        );
        assert!(!list(&db).await.unwrap().iter().any(|t| t.id == drop.id));
    }
}
