//! Away spans: time without input, asleep, locked or paused. Like activity, never edited except to
//! close the open span.

use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use ts_rs::TS;

use super::Db;
use super::time::Ms;
use crate::error::AppResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum AwayReason {
    Idle,
    Sleep,
    Lock,
    Paused,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct AwayRow {
    pub id: i64,
    pub start: Ms,
    /// `None` while the span is open.
    pub end: Option<Ms>,
    pub reason: AwayReason,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NewAway {
    pub start: Ms,
    pub reason: AwayReason,
}

pub async fn insert(db: &Db, away: NewAway) -> AppResult<i64> {
    Ok(
        sqlx::query("INSERT INTO away_spans (start, reason) VALUES (?1, ?2)")
            .bind(away.start)
            .bind(away.reason)
            .execute(db.pool())
            .await?
            .last_insert_rowid(),
    )
}

/// Ends every open span at `end` (never before its start); returns how many it closed.
pub async fn close_open(db: &Db, end: Ms) -> AppResult<u64> {
    Ok(
        sqlx::query(r#"UPDATE away_spans SET "end" = max(?1, start) WHERE "end" IS NULL"#)
            .bind(end)
            .execute(db.pool())
            .await?
            .rows_affected(),
    )
}

/// The latest open span.
pub async fn open(db: &Db) -> AppResult<Option<AwayRow>> {
    Ok(sqlx::query_as(
        r#"SELECT * FROM away_spans WHERE "end" IS NULL ORDER BY start DESC, id DESC LIMIT 1"#,
    )
    .fetch_optional(db.pool())
    .await?)
}

/// Spans that share time with `[from, to)`, open ones included, by start. Not clipped.
pub async fn list_for_range(db: &Db, from: Ms, to: Ms) -> AppResult<Vec<AwayRow>> {
    Ok(sqlx::query_as(
        r#"SELECT * FROM away_spans WHERE start < ?2 AND ("end" IS NULL OR "end" > ?1) ORDER BY start, id"#,
    )
    .bind(from)
    .bind(to)
    .fetch_all(db.pool())
    .await?)
}

/// Deletes every span that shares time with `[from, to)`, whole.
pub async fn delete_range(db: &Db, from: Ms, to: Ms) -> AppResult<u64> {
    Ok(
        sqlx::query(r#"DELETE FROM away_spans WHERE start < ?2 AND ("end" IS NULL OR "end" > ?1)"#)
            .bind(from)
            .bind(to)
            .execute(db.pool())
            .await?
            .rows_affected(),
    )
}

pub(super) async fn all(conn: &mut SqliteConnection) -> AppResult<Vec<AwayRow>> {
    Ok(sqlx::query_as("SELECT * FROM away_spans ORDER BY id")
        .fetch_all(conn)
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn away(start: Ms, reason: AwayReason) -> NewAway {
        NewAway { start, reason }
    }

    #[test]
    fn reasons_serialize_in_lowercase() {
        for (reason, expected) in [
            (AwayReason::Idle, "idle"),
            (AwayReason::Sleep, "sleep"),
            (AwayReason::Lock, "lock"),
            (AwayReason::Paused, "paused"),
        ] {
            assert_eq!(serde_json::to_value(reason).unwrap(), expected);
        }
    }

    #[tokio::test]
    async fn every_reason_round_trips_through_sqlite() {
        let db = Db::open_memory().await.unwrap();
        for (i, reason) in [
            AwayReason::Idle,
            AwayReason::Sleep,
            AwayReason::Lock,
            AwayReason::Paused,
        ]
        .into_iter()
        .enumerate()
        {
            let start = i as Ms * 10;
            let id = insert(&db, away(start, reason)).await.unwrap();
            let rows = list_for_range(&db, start, start + 1).await.unwrap();
            let row = rows.into_iter().find(|r| r.id == id).unwrap();
            assert_eq!(
                row,
                AwayRow {
                    id,
                    start,
                    end: None,
                    reason
                },
                "{reason:?}"
            );
        }
    }

    #[tokio::test]
    async fn close_open_closes_all_open_spans_but_never_before_their_start() {
        let db = Db::open_memory().await.unwrap();
        let done = insert(&db, away(0, AwayReason::Idle)).await.unwrap();
        close_open(&db, 10).await.unwrap();
        let early = insert(&db, away(100, AwayReason::Lock)).await.unwrap();
        let late = insert(&db, away(300, AwayReason::Sleep)).await.unwrap();

        assert_eq!(close_open(&db, 200).await.unwrap(), 2);

        let rows = list_for_range(&db, 0, 1_000).await.unwrap();
        let end_of = |id: i64| rows.iter().find(|r| r.id == id).unwrap().end;
        assert_eq!(end_of(done), Some(10));
        assert_eq!(end_of(early), Some(200));
        assert_eq!(end_of(late), Some(300));
        assert_eq!(open(&db).await.unwrap(), None);
    }

    #[tokio::test]
    async fn open_returns_the_latest_open_span() {
        let db = Db::open_memory().await.unwrap();
        assert_eq!(open(&db).await.unwrap(), None);
        insert(&db, away(100, AwayReason::Idle)).await.unwrap();
        let latest = insert(&db, away(200, AwayReason::Sleep)).await.unwrap();
        let row = open(&db).await.unwrap().unwrap();
        assert_eq!(
            (row.id, row.start, row.reason),
            (latest, 200, AwayReason::Sleep)
        );
    }

    #[tokio::test]
    async fn list_and_delete_range_use_whole_spans_that_share_time() {
        let db = Db::open_memory().await.unwrap();
        let before = insert(&db, away(0, AwayReason::Idle)).await.unwrap();
        close_open(&db, 100).await.unwrap();
        let crossing = insert(&db, away(150, AwayReason::Lock)).await.unwrap();
        close_open(&db, 250).await.unwrap();
        let running = insert(&db, away(400, AwayReason::Paused)).await.unwrap();

        let ids = |rows: Vec<AwayRow>| rows.into_iter().map(|r| r.id).collect::<Vec<_>>();
        assert_eq!(
            ids(list_for_range(&db, 100, 200).await.unwrap()),
            [crossing]
        );
        assert_eq!(
            ids(list_for_range(&db, 200, 1_000).await.unwrap()),
            [crossing, running]
        );

        assert_eq!(delete_range(&db, 200, 300).await.unwrap(), 1);
        assert_eq!(
            ids(list_for_range(&db, 0, 1_000).await.unwrap()),
            [before, running]
        );
    }
}
