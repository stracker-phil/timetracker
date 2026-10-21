use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use ts_rs::TS;

use super::time::{Ms, now_ms};
use super::{Db, name_in_use, name_taken, new_id, required_name, same_name};
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
    let name = required_name(name)?;
    if let Some(existing) = list(db)
        .await?
        .into_iter()
        .find(|t| same_name(&t.name, &name))
    {
        return Ok(existing);
    }
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

/// `invalid_input` on `name` when another tag has the name.
pub async fn rename(db: &Db, id: &str, name: &str) -> AppResult<Tag> {
    let name = required_name(name)?;
    let tags = list(db).await?;
    if name_in_use(
        tags.iter().map(|t| (t.id.as_str(), t.name.as_str())),
        &name,
        Some(id),
    ) {
        return Err(name_taken("A tag"));
    }
    sqlx::query_as("UPDATE tags SET name = ?2 WHERE id = ?1 RETURNING *")
        .bind(id)
        .bind(&name)
        .fetch_optional(db.pool())
        .await?
        .ok_or_else(|| not_found(id))
}

/// The tags with these names, ignoring case, in the order given; a missing one is `not_found` naming it.
pub async fn resolve_names(db: &Db, names: &[String]) -> AppResult<Vec<Tag>> {
    let tags = list(db).await?;
    names
        .iter()
        .map(|name| {
            let wanted = name.trim();
            tags.iter()
                .find(|t| same_name(&t.name, wanted))
                .cloned()
                .ok_or_else(|| AppError::NotFound(format!("No tag named {wanted}")))
        })
        .collect()
}

/// A tag with the number of recordings that carry it.
#[derive(Debug, Clone, PartialEq, Serialize, TS, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct TagListItem {
    #[serde(flatten)]
    #[sqlx(flatten)]
    #[ts(flatten)]
    pub tag: Tag,
    pub recordings: i64,
}

/// Every tag by name, with its use count.
pub async fn list_with_usage(db: &Db) -> AppResult<Vec<TagListItem>> {
    Ok(sqlx::query_as(
        "SELECT t.*, count(rt.recording_id) AS recordings FROM tags t
         LEFT JOIN recording_tags rt ON rt.tag_id = t.id
         GROUP BY t.id ORDER BY t.name COLLATE NOCASE",
    )
    .fetch_all(db.pool())
    .await?)
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
    async fn inserting_an_existing_name_in_another_unicode_case_returns_the_existing_tag() {
        let db = Db::open_memory().await.unwrap();
        let existing = insert(&db, "Äpfel").await.unwrap();
        for name in ["äpfel", "ÄPFEL"] {
            assert_eq!(insert(&db, name).await.unwrap(), existing, "{name:?}");
        }
        assert_eq!(list(&db).await.unwrap().len(), DEFAULT_TAGS.len() + 1);
    }

    fn field_of(err: AppError) -> String {
        match err {
            AppError::InvalidField { field, .. } => field,
            other => panic!("expected an invalid field, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn blank_and_oversized_names_are_rejected_on_the_name_field() {
        let db = Db::open_memory().await.unwrap();
        let tag = insert(&db, "x").await.unwrap();
        let too_long = "t".repeat(crate::db::MAX_NAME_CHARS + 1);
        for name in ["", "  ", too_long.as_str()] {
            let err = insert(&db, name).await.unwrap_err();
            assert_eq!(field_of(err), "name", "insert {name:?}");
            let err = rename(&db, &tag.id, name).await.unwrap_err();
            assert_eq!(field_of(err), "name", "rename {name:?}");
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

        for name in ["beta", "BETA"] {
            let err = rename(&db, &a.id, name).await.unwrap_err();
            assert_eq!(field_of(err), "name", "{name:?}");
        }

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
        let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM recording_tags")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(remaining, 1);
    }

    #[tokio::test]
    async fn resolve_names_ignores_case_trims_and_keeps_the_given_order() {
        let db = Db::open_memory().await.unwrap();
        let apple = insert(&db, "Äpfel").await.unwrap();
        let beta = insert(&db, "beta").await.unwrap();

        let resolved = resolve_names(
            &db,
            &[
                " BETA ".to_string(),
                "äpfel".to_string(),
                "beta".to_string(),
            ],
        )
        .await
        .unwrap();

        assert_eq!(resolved, [beta.clone(), apple, beta]);
        assert!(resolve_names(&db, &[]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn resolve_names_with_a_missing_name_is_not_found_naming_it() {
        let db = Db::open_memory().await.unwrap();
        insert(&db, "beta").await.unwrap();

        let err = resolve_names(&db, &["beta".to_string(), " ghost ".to_string()])
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(err.to_string(), "No tag named ghost");
    }

    #[tokio::test]
    async fn list_with_usage_counts_recordings_per_tag_ordered_by_name() {
        let db = Db::open_memory().await.unwrap();
        sqlx::query("DELETE FROM tags")
            .execute(db.pool())
            .await
            .unwrap();
        let beta = insert(&db, "beta").await.unwrap();
        let alpha = insert(&db, "Alpha").await.unwrap();
        let unused = insert(&db, "zeta").await.unwrap();
        let p = project(&db, "P").await;
        let first = recording(&db, &p.id, 1_000, Some(2_000)).await;
        let second = recording(&db, &p.id, 3_000, Some(4_000)).await;
        for (recording, tags) in [(&first, vec![&alpha, &beta]), (&second, vec![&beta])] {
            let patch = recordings::RecordingPatch {
                tag_ids: Some(tags.iter().map(|t| t.id.clone()).collect()),
                ..Default::default()
            };
            recordings::update(&db, &recording.id, patch).await.unwrap();
        }

        let items = list_with_usage(&db).await.unwrap();

        assert_eq!(
            items,
            [
                TagListItem {
                    tag: alpha,
                    recordings: 1
                },
                TagListItem {
                    tag: beta,
                    recordings: 2
                },
                TagListItem {
                    tag: unused,
                    recordings: 0
                },
            ]
        );
    }

    #[tokio::test]
    async fn a_tag_list_item_serializes_flat() {
        let db = Db::open_memory().await.unwrap();
        let tag = insert(&db, "zzz").await.unwrap();

        let items = list_with_usage(&db).await.unwrap();
        let item = items.iter().find(|i| i.tag.id == tag.id).unwrap();

        assert_eq!(
            serde_json::to_value(item).unwrap(),
            serde_json::json!({
                "id": tag.id,
                "name": "zzz",
                "createdAt": tag.created_at,
                "recordings": 0,
            })
        );
    }
}
