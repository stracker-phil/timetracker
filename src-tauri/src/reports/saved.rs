//! Saved reports: a name and a `ReportFilter`, in the order they were saved.

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, Row, SqliteConnection};
use ts_rs::TS;

use super::ReportFilter;
use crate::db::{Db, new_id, required_name};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct SavedReport {
    pub id: String,
    pub name: String,
    pub filter: ReportFilter,
    pub position: i64,
}

impl FromRow<'_, SqliteRow> for SavedReport {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        let filter: String = row.try_get("filter")?;
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            // A filter this version can't read (hand-edited, or from a newer version) opens as the default.
            filter: serde_json::from_str(&filter).unwrap_or_default(),
            position: row.try_get("position")?,
        })
    }
}

pub async fn list(db: &Db) -> AppResult<Vec<SavedReport>> {
    let mut conn = db.pool().acquire().await?;
    all(&mut conn).await
}

pub async fn all(conn: &mut SqliteConnection) -> AppResult<Vec<SavedReport>> {
    Ok(
        sqlx::query_as("SELECT * FROM saved_reports ORDER BY position, id")
            .fetch_all(conn)
            .await?,
    )
}

/// Appended after the others.
pub async fn save(db: &Db, name: &str, filter: &ReportFilter) -> AppResult<SavedReport> {
    let name = required_name(name)?;
    let id = new_id();
    sqlx::query(
        "INSERT INTO saved_reports (id, name, filter, position)
         VALUES (?1, ?2, ?3, (SELECT coalesce(max(position), -1) + 1 FROM saved_reports))",
    )
    .bind(&id)
    .bind(&name)
    .bind(serde_json::to_string(filter)?)
    .execute(db.pool())
    .await?;
    get(db, &id).await
}

pub async fn rename(db: &Db, id: &str, name: &str) -> AppResult<SavedReport> {
    let name = required_name(name)?;
    let changed = sqlx::query("UPDATE saved_reports SET name = ?2 WHERE id = ?1")
        .bind(id)
        .bind(&name)
        .execute(db.pool())
        .await?
        .rows_affected();
    if changed == 0 {
        return Err(not_found(id));
    }
    get(db, id).await
}

pub async fn delete(db: &Db, id: &str) -> AppResult<()> {
    let deleted = sqlx::query("DELETE FROM saved_reports WHERE id = ?1")
        .bind(id)
        .execute(db.pool())
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

async fn get(db: &Db, id: &str) -> AppResult<SavedReport> {
    sqlx::query_as("SELECT * FROM saved_reports WHERE id = ?1")
        .bind(id)
        .fetch_optional(db.pool())
        .await?
        .ok_or_else(|| not_found(id))
}

fn not_found(id: &str) -> AppError {
    AppError::NotFound(format!("No saved report {id}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::time::Day;
    use crate::error::ErrorKind;
    use crate::reports::Range;
    use crate::store::settings::{Group, Rounding};

    async fn db() -> Db {
        Db::open_memory().await.unwrap()
    }

    fn filter(range: Range) -> ReportFilter {
        ReportFilter {
            range,
            ..ReportFilter::default()
        }
    }

    #[tokio::test]
    async fn a_new_database_has_no_saved_reports() {
        assert!(list(&db().await).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn saved_reports_are_listed_in_the_order_they_were_saved() {
        let db = db().await;
        let a = save(&db, "Weekly", &filter(Range::LastWeek)).await.unwrap();
        let b = save(&db, "Monthly", &filter(Range::ThisMonth))
            .await
            .unwrap();
        let c = save(&db, "Daily", &filter(Range::Today)).await.unwrap();

        let listed = list(&db).await.unwrap();

        assert_eq!(listed, vec![a.clone(), b.clone(), c.clone()]);
        assert_eq!(
            listed.iter().map(|r| r.position).collect::<Vec<_>>(),
            [0, 1, 2]
        );
    }

    #[tokio::test]
    async fn the_name_is_trimmed() {
        let saved = save(&db().await, "  Weekly  ", &ReportFilter::default())
            .await
            .unwrap();
        assert_eq!(saved.name, "Weekly");
    }

    #[tokio::test]
    async fn the_filter_round_trips_with_relative_and_custom_ranges() {
        let db = db().await;
        let custom = ReportFilter {
            range: Range::Custom {
                from: Day::parse("2026-10-01").unwrap(),
                to: Day::parse("2026-10-07").unwrap(),
            },
            client_ids: vec!["c1".into()],
            project_ids: vec!["p1".into(), "p2".into()],
            tag_ids: vec!["t1".into()],
            group_by: Group::Tag,
            rounding: Rounding::Up15,
            include_archived: true,
        };
        let relative = filter(Range::ThisWeek);
        save(&db, "Custom", &custom).await.unwrap();
        save(&db, "Relative", &relative).await.unwrap();

        let listed = list(&db).await.unwrap();

        assert_eq!(listed[0].filter, custom);
        assert_eq!(listed[1].filter, relative);
    }

    #[tokio::test]
    async fn a_blank_name_is_invalid_input() {
        let db = db().await;
        for name in ["", "   "] {
            let err = save(&db, name, &ReportFilter::default()).await.unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "{name:?}");
        }
        assert!(list(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn rename_changes_only_the_name() {
        let db = db().await;
        let saved = save(&db, "Weekly", &filter(Range::LastWeek)).await.unwrap();

        let renamed = rename(&db, &saved.id, " Last week ").await.unwrap();

        assert_eq!(
            renamed,
            SavedReport {
                name: "Last week".into(),
                ..saved
            }
        );
        assert_eq!(list(&db).await.unwrap(), vec![renamed]);
    }

    #[tokio::test]
    async fn rename_rejects_a_blank_name_and_an_unknown_id() {
        let db = db().await;
        let saved = save(&db, "Weekly", &ReportFilter::default()).await.unwrap();

        let blank = rename(&db, &saved.id, " ").await.unwrap_err();
        let unknown = rename(&db, "missing", "Name").await.unwrap_err();

        assert_eq!(blank.kind(), ErrorKind::InvalidInput);
        assert_eq!(unknown.kind(), ErrorKind::NotFound);
        assert_eq!(list(&db).await.unwrap()[0].name, "Weekly");
    }

    #[tokio::test]
    async fn delete_removes_the_report_and_unknown_ids_are_not_found() {
        let db = db().await;
        let a = save(&db, "A", &ReportFilter::default()).await.unwrap();
        let b = save(&db, "B", &ReportFilter::default()).await.unwrap();

        delete(&db, &a.id).await.unwrap();

        assert_eq!(list(&db).await.unwrap(), vec![b]);
        assert_eq!(
            delete(&db, &a.id).await.unwrap_err().kind(),
            ErrorKind::NotFound
        );
    }

    #[tokio::test]
    async fn a_report_saved_after_a_delete_goes_last() {
        let db = db().await;
        save(&db, "A", &ReportFilter::default()).await.unwrap();
        let b = save(&db, "B", &ReportFilter::default()).await.unwrap();
        delete(&db, &b.id).await.unwrap();

        let c = save(&db, "C", &ReportFilter::default()).await.unwrap();

        assert_eq!(
            list(&db)
                .await
                .unwrap()
                .iter()
                .map(|r| r.name.as_str())
                .collect::<Vec<_>>(),
            ["A", "C"]
        );
        assert_eq!(c.position, 1);
    }

    #[tokio::test]
    async fn an_unreadable_filter_loads_as_the_default_filter() {
        let db = db().await;
        sqlx::query(
            "INSERT INTO saved_reports (id, name, filter, position) VALUES ('x', 'Old', '{\"range\": 5}', 0)",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let listed = list(&db).await.unwrap();

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Old");
        assert_eq!(listed[0].filter, ReportFilter::default());
    }
}
