//! Activity rows are never edited, except to close the open span (ADR 0005). Readers clip spans to
//! the range they show (`time::clip`); nothing is split on write.

use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use ts_rs::TS;

use super::Db;
use super::time::Ms;
use crate::error::AppResult;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ActivityRow {
    pub id: i64,
    /// `tracker` or `external:<name>`.
    pub source: String,
    pub start: Ms,
    /// `None` while the span is open.
    pub end: Option<Ms>,
    pub app: String,
    pub bundle_id: Option<String>,
    pub title: String,
    pub url: Option<String>,
    pub path: Option<String>,
    pub private: bool,
}

/// An open span; `close_open` ends it.
#[derive(Debug, Clone, PartialEq)]
pub struct NewActivity {
    pub source: String,
    pub start: Ms,
    pub app: String,
    pub bundle_id: Option<String>,
    pub title: String,
    pub url: Option<String>,
    pub path: Option<String>,
    pub private: bool,
}

pub async fn insert(db: &Db, activity: NewActivity) -> AppResult<i64> {
    Ok(sqlx::query(
        "INSERT INTO activities (source, start, app, bundle_id, title, url, path, private)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )
    .bind(activity.source)
    .bind(activity.start)
    .bind(activity.app)
    .bind(activity.bundle_id)
    .bind(activity.title)
    .bind(activity.url)
    .bind(activity.path)
    .bind(activity.private)
    .execute(db.pool())
    .await?
    .last_insert_rowid())
}

/// Ends every open span of `source` at `end` (never before its start); returns how many it closed.
pub async fn close_open(db: &Db, source: &str, end: Ms) -> AppResult<u64> {
    Ok(sqlx::query(
        r#"UPDATE activities SET "end" = max(?2, start) WHERE source = ?1 AND "end" IS NULL"#,
    )
    .bind(source)
    .bind(end)
    .execute(db.pool())
    .await?
    .rows_affected())
}

/// The latest open span of `source`.
pub async fn open_by_source(db: &Db, source: &str) -> AppResult<Option<ActivityRow>> {
    Ok(sqlx::query_as(
        r#"SELECT * FROM activities WHERE source = ?1 AND "end" IS NULL ORDER BY start DESC, id DESC LIMIT 1"#,
    )
    .bind(source)
    .fetch_optional(db.pool())
    .await?)
}

/// Rows of `source` that share time with `[start, end)`; touching edges do not count.
pub async fn overlapping(db: &Db, source: &str, start: Ms, end: Ms) -> AppResult<Vec<ActivityRow>> {
    Ok(sqlx::query_as(
        r#"SELECT * FROM activities WHERE source = ?1 AND start < ?3 AND ("end" IS NULL OR "end" > ?2)
           ORDER BY start, id"#,
    )
    .bind(source)
    .bind(start)
    .bind(end)
    .fetch_all(db.pool())
    .await?)
}

/// Rows that share time with `[from, to)`, open ones included, by start. Not clipped.
pub async fn list_for_range(db: &Db, from: Ms, to: Ms) -> AppResult<Vec<ActivityRow>> {
    Ok(sqlx::query_as(
        r#"SELECT * FROM activities WHERE start < ?2 AND ("end" IS NULL OR "end" > ?1) ORDER BY start, id"#,
    )
    .bind(from)
    .bind(to)
    .fetch_all(db.pool())
    .await?)
}

/// The distinct sources of the rows in `[from, to)`, sorted.
pub async fn list_sources(db: &Db, from: Ms, to: Ms) -> AppResult<Vec<String>> {
    Ok(sqlx::query_scalar(
        r#"SELECT DISTINCT source FROM activities WHERE start < ?2 AND ("end" IS NULL OR "end" > ?1)
           ORDER BY source"#,
    )
    .bind(from)
    .bind(to)
    .fetch_all(db.pool())
    .await?)
}

/// Deletes every row that shares time with `[from, to)`, whole: activity is never trimmed.
pub async fn delete_range(db: &Db, from: Ms, to: Ms) -> AppResult<u64> {
    Ok(
        sqlx::query(r#"DELETE FROM activities WHERE start < ?2 AND ("end" IS NULL OR "end" > ?1)"#)
            .bind(from)
            .bind(to)
            .execute(db.pool())
            .await?
            .rows_affected(),
    )
}

pub async fn count(db: &Db) -> AppResult<i64> {
    Ok(sqlx::query_scalar("SELECT count(*) FROM activities")
        .fetch_one(db.pool())
        .await?)
}

pub async fn first_start(db: &Db) -> AppResult<Option<Ms>> {
    Ok(sqlx::query_scalar("SELECT min(start) FROM activities")
        .fetch_one(db.pool())
        .await?)
}

pub(super) async fn all(conn: &mut SqliteConnection) -> AppResult<Vec<ActivityRow>> {
    Ok(sqlx::query_as("SELECT * FROM activities ORDER BY id")
        .fetch_all(conn)
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::activity;

    fn external(source: &str, start: Ms) -> NewActivity {
        NewActivity {
            source: source.into(),
            start,
            app: "git".into(),
            bundle_id: None,
            title: "commit".into(),
            url: None,
            path: None,
            private: false,
        }
    }

    fn ids(rows: Vec<ActivityRow>) -> Vec<i64> {
        rows.into_iter().map(|r| r.id).collect()
    }

    #[tokio::test]
    async fn insert_returns_increasing_ids_and_an_open_row() {
        let db = Db::open_memory().await.unwrap();
        let a = insert(&db, external("tracker", 100)).await.unwrap();
        let b = insert(&db, external("tracker", 200)).await.unwrap();
        assert!(b > a);
        let open = open_by_source(&db, "tracker").await.unwrap().unwrap();
        assert_eq!((open.id, open.start, open.end), (b, 200, None));
    }

    #[tokio::test]
    async fn close_open_closes_only_that_sources_open_rows() {
        let db = Db::open_memory().await.unwrap();
        let closed = activity(&db, 100, Some(150), "Mail", "").await;
        let open = activity(&db, 200, None, "Code", "").await;
        let other = insert(&db, external("external:git", 200)).await.unwrap();

        assert_eq!(close_open(&db, "tracker", 300).await.unwrap(), 1);

        let rows = list_for_range(&db, 0, 1_000).await.unwrap();
        let end_of = |id: i64| rows.iter().find(|r| r.id == id).unwrap().end;
        assert_eq!(end_of(closed.id), Some(150));
        assert_eq!(end_of(open.id), Some(300));
        assert_eq!(end_of(other), None);
        assert_eq!(open_by_source(&db, "tracker").await.unwrap(), None);
        assert_eq!(close_open(&db, "tracker", 400).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn close_open_never_ends_before_the_start() {
        let db = Db::open_memory().await.unwrap();
        let row = activity(&db, 500, None, "Code", "").await;
        close_open(&db, "tracker", 100).await.unwrap();
        let rows = list_for_range(&db, 0, 1_000).await.unwrap();
        assert_eq!(rows[0].id, row.id);
        assert_eq!(rows[0].end, Some(500));
    }

    #[tokio::test]
    async fn open_by_source_returns_the_latest_open_row_of_that_source() {
        let db = Db::open_memory().await.unwrap();
        activity(&db, 100, None, "Old", "").await;
        let latest = activity(&db, 200, None, "New", "").await;
        insert(&db, external("external:git", 300)).await.unwrap();
        assert_eq!(open_by_source(&db, "tracker").await.unwrap(), Some(latest));
        assert_eq!(open_by_source(&db, "external:none").await.unwrap(), None);
    }

    #[tokio::test]
    async fn overlapping_excludes_touching_edges_and_other_sources() {
        let db = Db::open_memory().await.unwrap();
        let before = activity(&db, 0, Some(100), "A", "").await;
        let inside = activity(&db, 100, Some(200), "B", "").await;
        let after = activity(&db, 200, Some(300), "C", "").await;
        let open = activity(&db, 300, None, "D", "").await;
        insert(&db, external("external:git", 120)).await.unwrap();

        assert_eq!(tracker_ids(&db, 100, 200).await, [inside.id]);
        assert_eq!(
            tracker_ids(&db, 50, 250).await,
            [before.id, inside.id, after.id]
        );
        assert_eq!(tracker_ids(&db, 250, 300).await, [after.id]);
        assert_eq!(tracker_ids(&db, 1_000, 2_000).await, [open.id]);
        assert_eq!(tracker_ids(&db, 299, 301).await, [after.id, open.id]);
    }

    async fn tracker_ids(db: &Db, from: Ms, to: Ms) -> Vec<i64> {
        ids(overlapping(db, "tracker", from, to).await.unwrap())
    }

    #[tokio::test]
    async fn list_for_range_returns_crossing_and_open_rows_whole_in_start_order() {
        let db = Db::open_memory().await.unwrap();
        let late = activity(&db, 150, None, "Open", "").await;
        let crossing = activity(&db, 50, Some(150), "Cross", "").await;
        let early = activity(&db, 10, Some(20), "Early", "").await;

        let day1 = list_for_range(&db, 0, 100).await.unwrap();
        let day2 = list_for_range(&db, 100, 200).await.unwrap();

        assert_eq!(day1, [early.clone(), crossing.clone()]);
        assert_eq!(day2, [crossing, late]);
        assert_eq!(day1[1].start, 50);
        assert_eq!(day1[1].end, Some(150));
        assert!(list_for_range(&db, 20, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_sources_are_distinct_and_sorted_for_the_range() {
        let db = Db::open_memory().await.unwrap();
        activity(&db, 10, Some(20), "A", "").await;
        activity(&db, 20, Some(30), "B", "").await;
        insert(&db, external("external:git", 25)).await.unwrap();
        insert(&db, external("external:zed", 5_000)).await.unwrap();

        assert_eq!(
            list_sources(&db, 0, 100).await.unwrap(),
            ["external:git", "tracker"]
        );
        assert_eq!(list_sources(&db, 100, 200).await.unwrap(), ["external:git"]);
        assert!(list_sources(&db, 0, 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn delete_range_removes_crossing_rows_whole_and_keeps_the_rest() {
        let db = Db::open_memory().await.unwrap();
        let before = activity(&db, 0, Some(100), "A", "").await;
        activity(&db, 90, Some(110), "Cross", "").await;
        activity(&db, 150, Some(160), "In", "").await;
        let after = activity(&db, 200, Some(300), "C", "").await;

        assert_eq!(delete_range(&db, 100, 200).await.unwrap(), 2);

        assert_eq!(
            list_for_range(&db, 0, 1_000).await.unwrap(),
            [before, after]
        );
        assert_eq!(delete_range(&db, 100, 200).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn ids_are_not_reused_after_deleting_the_newest_row() {
        let db = Db::open_memory().await.unwrap();
        let newest = activity(&db, 100, Some(200), "A", "").await;
        delete_range(&db, 0, 1_000).await.unwrap();
        let next = activity(&db, 100, Some(200), "A", "").await;
        assert!(next.id > newest.id);
    }

    #[tokio::test]
    async fn count_and_first_start_describe_the_table() {
        let db = Db::open_memory().await.unwrap();
        assert_eq!(count(&db).await.unwrap(), 0);
        assert_eq!(first_start(&db).await.unwrap(), None);

        activity(&db, 300, Some(400), "A", "").await;
        activity(&db, 100, Some(200), "B", "").await;
        assert_eq!(count(&db).await.unwrap(), 2);
        assert_eq!(first_start(&db).await.unwrap(), Some(100));
    }
}
