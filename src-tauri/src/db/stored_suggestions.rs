//! Suggestions other apps stored (ADR 0028): a proposed recording that waits for the owner's review.

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};
use ts_rs::TS;

use super::Db;
use super::recordings::Span;
use super::time::Day;
use crate::db::time::Ms;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct StoredSuggestion {
    pub id: String,
    /// None until the owner picks one; it can't be approved without (ADR 0030).
    pub project_id: Option<String>,
    pub note: String,
    pub tag_id: Option<String>,
    /// The app that stored it (`cli`, `claude`, or the caller's `--source`).
    pub source: String,
    /// The local day all of its spans lie on.
    pub day: Day,
    /// By start, never overlapping each other; at least one unless it runs.
    pub spans: Vec<Span>,
    pub created_at: Ms,
    pub updated_at: Ms,
    /// Where its open span starts while it runs as a timer (ADR 0041); after every span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub open_start: Option<Ms>,
}

impl StoredSuggestion {
    /// The sum of its spans, the open one up to `now`.
    pub fn duration(&self, now: Ms) -> Ms {
        let closed: Ms = self.spans.iter().map(|s| s.end - s.start).sum();
        closed + self.open_start.map_or(0, |start| (now - start).max(0))
    }

    pub fn is_running(&self) -> bool {
        self.open_start.is_some()
    }

    /// Where it starts: its first span, else its open one.
    pub fn start(&self) -> Option<Ms> {
        self.spans.first().map(|s| s.start).or(self.open_start)
    }
}

macro_rules! select {
    ($rest:literal) => {
        concat!(
            "SELECT id, project_id, note, tag_id, source, day, spans, created_at, updated_at, open_start ",
            "FROM stored_suggestions ",
            $rest
        )
    };
}

fn from_row(row: &SqliteRow) -> AppResult<StoredSuggestion> {
    let day: String = row.try_get("day")?;
    let spans: String = row.try_get("spans")?;
    Ok(StoredSuggestion {
        id: row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        note: row.try_get("note")?,
        tag_id: row.try_get("tag_id")?,
        source: row.try_get("source")?,
        day: Day::try_from(day)?,
        spans: serde_json::from_str(&spans)?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        open_start: row.try_get("open_start")?,
    })
}

pub async fn get(db: &Db, id: &str) -> AppResult<StoredSuggestion> {
    let row = sqlx::query(select!("WHERE id = ?1"))
        .bind(id)
        .fetch_optional(db.pool())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("No suggestion {id}")))?;
    from_row(&row)
}

/// The suggestions of the days `from` to `to`, both included, by day and first start.
pub async fn list_for_days(db: &Db, from: Day, to: Day) -> AppResult<Vec<StoredSuggestion>> {
    let rows = sqlx::query(select!(
        "WHERE day >= ?1 AND day <= ?2 ORDER BY day, COALESCE(json_extract(spans, '$[0].start'), open_start), id"
    ))
    .bind(from.to_string())
    .bind(to.to_string())
    .fetch_all(db.pool())
    .await?;
    rows.iter().map(from_row).collect()
}

pub async fn list_for_day(db: &Db, day: Day) -> AppResult<Vec<StoredSuggestion>> {
    list_for_days(db, day, day).await
}

/// The suggestion that runs as a timer, if one does.
pub async fn running(db: &Db) -> AppResult<Option<StoredSuggestion>> {
    let row = sqlx::query(select!("WHERE open_start IS NOT NULL LIMIT 1"))
        .fetch_optional(db.pool())
        .await?;
    row.as_ref().map(from_row).transpose()
}

/// Writes the suggestion exactly, inserting or replacing it; undo restores it this way.
pub async fn put_on(conn: &mut SqliteConnection, suggestion: &StoredSuggestion) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO stored_suggestions
             (id, project_id, note, tag_id, source, day, spans, created_at, updated_at, open_start)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
           ON CONFLICT(id) DO UPDATE SET project_id = ?2, note = ?3, tag_id = ?4, source = ?5, day = ?6,
             spans = ?7, created_at = ?8, updated_at = ?9, open_start = ?10"#,
    )
    .bind(&suggestion.id)
    .bind(&suggestion.project_id)
    .bind(&suggestion.note)
    .bind(&suggestion.tag_id)
    .bind(&suggestion.source)
    .bind(suggestion.day.to_string())
    .bind(serde_json::to_string(&suggestion.spans)?)
    .bind(suggestion.created_at)
    .bind(suggestion.updated_at)
    .bind(suggestion.open_start)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn remove_on(conn: &mut SqliteConnection, id: &str) -> AppResult<()> {
    let done = sqlx::query("DELETE FROM stored_suggestions WHERE id = ?1")
        .bind(id)
        .execute(conn)
        .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("No suggestion {id}")));
    }
    Ok(())
}

pub(super) async fn all(conn: &mut SqliteConnection) -> AppResult<Vec<StoredSuggestion>> {
    let rows = sqlx::query(select!(
        "ORDER BY day, COALESCE(json_extract(spans, '$[0].start'), open_start), id"
    ))
    .fetch_all(conn)
    .await?;
    rows.iter().map(from_row).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tags;
    use crate::error::ErrorKind;
    use crate::test_support::project;
    use serde_json::json;

    fn day(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    fn suggestion(id: &str, project_id: &str, day: Day, spans: &[(Ms, Ms)]) -> StoredSuggestion {
        StoredSuggestion {
            id: id.into(),
            project_id: Some(project_id.into()),
            note: "note".into(),
            tag_id: None,
            source: "cli".into(),
            day,
            spans: spans
                .iter()
                .map(|&(start, end)| Span { start, end })
                .collect(),
            created_at: 1_000,
            updated_at: 2_000,
            open_start: None,
        }
    }

    async fn put(db: &Db, suggestion: &StoredSuggestion) {
        let mut conn = db.pool().acquire().await.unwrap();
        put_on(&mut conn, suggestion).await.unwrap();
    }

    #[tokio::test]
    async fn a_stored_suggestion_reads_back_exactly_as_written() {
        let db = Db::open_memory().await.unwrap();
        let project = project(&db, "Portal").await;
        let tag = tags::insert(&db, "billing").await.unwrap();
        let written = StoredSuggestion {
            tag_id: Some(tag.id),
            ..suggestion(
                "s1",
                &project.id,
                day("2026-01-05"),
                &[(1_000, 2_000), (3_000, 4_000)],
            )
        };

        put(&db, &written).await;

        assert_eq!(get(&db, "s1").await.unwrap(), written);
    }

    #[tokio::test]
    async fn a_suggestion_without_a_project_reads_back_exactly_as_written() {
        let db = Db::open_memory().await.unwrap();
        let written = StoredSuggestion {
            project_id: None,
            ..suggestion("s1", "unused", day("2026-01-05"), &[(1_000, 2_000)])
        };

        put(&db, &written).await;

        assert_eq!(get(&db, "s1").await.unwrap(), written);
    }

    #[test]
    fn a_suggestion_without_a_project_serializes_the_project_id_as_null() {
        let s = StoredSuggestion {
            project_id: None,
            ..suggestion("s1", "unused", day("2026-01-05"), &[(1_000, 2_000)])
        };

        assert_eq!(
            serde_json::to_value(&s).unwrap()["projectId"],
            serde_json::Value::Null
        );
    }

    #[tokio::test]
    async fn putting_an_existing_id_replaces_the_suggestion() {
        let db = Db::open_memory().await.unwrap();
        let project = project(&db, "Portal").await;
        let monday = day("2026-01-05");
        put(
            &db,
            &suggestion("s1", &project.id, monday, &[(1_000, 2_000)]),
        )
        .await;
        let replaced = StoredSuggestion {
            note: "changed".into(),
            updated_at: 5_000,
            ..suggestion("s1", &project.id, monday, &[(3_000, 4_000)])
        };

        put(&db, &replaced).await;

        assert_eq!(list_for_day(&db, monday).await.unwrap(), [replaced]);
    }

    #[tokio::test]
    async fn a_day_lists_its_suggestions_by_first_start() {
        let db = Db::open_memory().await.unwrap();
        let project = project(&db, "Portal").await;
        let (monday, tuesday) = (day("2026-01-05"), day("2026-01-06"));
        let late = suggestion("late", &project.id, monday, &[(3_000, 4_000)]);
        let early = suggestion("early", &project.id, monday, &[(1_000, 2_000)]);
        let other_day = suggestion("other", &project.id, tuesday, &[(500, 600)]);
        for s in [&late, &early, &other_day] {
            put(&db, s).await;
        }

        assert_eq!(list_for_day(&db, monday).await.unwrap(), [early, late]);
        assert_eq!(list_for_day(&db, tuesday).await.unwrap(), [other_day]);
        assert!(
            list_for_day(&db, day("2026-01-07"))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_range_of_days_includes_both_ends_and_lists_by_day_then_start() {
        let db = Db::open_memory().await.unwrap();
        let project = project(&db, "Portal").await;
        let days = ["2026-01-04", "2026-01-05", "2026-01-06", "2026-01-07"];
        let mut all = Vec::new();
        for (i, d) in days.iter().enumerate() {
            let start = 10_000 - i as Ms * 1_000;
            all.push(suggestion(
                &format!("s{i}"),
                &project.id,
                day(d),
                &[(start, start + 100)],
            ));
        }
        for s in all.iter().rev() {
            put(&db, s).await;
        }

        let found = list_for_days(&db, day("2026-01-05"), day("2026-01-06"))
            .await
            .unwrap();

        assert_eq!(found, [all[1].clone(), all[2].clone()]);
    }

    #[tokio::test]
    async fn removing_a_suggestion_deletes_it_and_an_unknown_id_is_not_found() {
        let db = Db::open_memory().await.unwrap();
        let project = project(&db, "Portal").await;
        let monday = day("2026-01-05");
        let keep = suggestion("keep", &project.id, monday, &[(1_000, 2_000)]);
        put(&db, &keep).await;
        put(
            &db,
            &suggestion("drop", &project.id, monday, &[(3_000, 4_000)]),
        )
        .await;
        let mut conn = db.pool().acquire().await.unwrap();

        remove_on(&mut conn, "drop").await.unwrap();
        let again = remove_on(&mut conn, "drop").await.unwrap_err();
        drop(conn);

        assert_eq!(list_for_day(&db, monday).await.unwrap(), [keep]);
        assert_eq!(again.kind(), ErrorKind::NotFound);
        assert_eq!(again.to_string(), "No suggestion drop");
    }

    #[tokio::test]
    async fn getting_an_unknown_suggestion_is_not_found() {
        let db = Db::open_memory().await.unwrap();

        let err = get(&db, "nope").await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(err.to_string(), "No suggestion nope");
    }

    #[tokio::test]
    async fn deleting_a_tag_leaves_the_suggestion_without_one() {
        let db = Db::open_memory().await.unwrap();
        let project = project(&db, "Portal").await;
        let tag = tags::insert(&db, "billing").await.unwrap();
        let monday = day("2026-01-05");
        put(
            &db,
            &StoredSuggestion {
                tag_id: Some(tag.id.clone()),
                ..suggestion("s1", &project.id, monday, &[(1_000, 2_000)])
            },
        )
        .await;

        tags::delete(&db, &tag.id).await.unwrap();

        assert_eq!(get(&db, "s1").await.unwrap().tag_id, None);
    }

    fn running_suggestion(id: &str, project_id: &str, open_start: Ms) -> StoredSuggestion {
        StoredSuggestion {
            open_start: Some(open_start),
            ..suggestion(id, project_id, day("2026-01-05"), &[])
        }
    }

    #[tokio::test]
    async fn a_running_suggestion_reads_back_with_its_open_start_and_is_the_running_one() {
        let db = Db::open_memory().await.unwrap();
        let project = project(&db, "Portal").await;
        let finished = suggestion("s1", &project.id, day("2026-01-05"), &[(1_000, 2_000)]);
        let running = running_suggestion("s2", &project.id, 5_000);
        put(&db, &finished).await;
        assert_eq!(running_of(&db).await, None);

        put(&db, &running).await;

        assert_eq!(get(&db, "s2").await.unwrap(), running);
        assert_eq!(running_of(&db).await, Some(running));
    }

    async fn running_of(db: &Db) -> Option<StoredSuggestion> {
        running(db).await.unwrap()
    }

    #[tokio::test]
    async fn a_second_running_suggestion_is_refused() {
        let db = Db::open_memory().await.unwrap();
        let project = project(&db, "Portal").await;
        put(&db, &running_suggestion("s1", &project.id, 5_000)).await;
        let mut conn = db.pool().acquire().await.unwrap();

        let err = put_on(&mut conn, &running_suggestion("s2", &project.id, 6_000)).await;

        assert!(err.is_err());
        drop(conn);
        assert_eq!(running_of(&db).await.map(|s| s.id), Some("s1".to_string()));
    }

    #[test]
    fn the_duration_counts_the_open_part_up_to_now() {
        let mut s = suggestion("s1", "p1", day("2026-01-05"), &[(1_000, 3_000)]);
        assert_eq!(s.duration(10_000), 2_000);
        assert!(!s.is_running());

        s.open_start = Some(5_000);

        assert_eq!(s.duration(9_000), 6_000);
        assert_eq!(s.duration(4_000), 2_000);
        assert!(s.is_running());
    }

    #[test]
    fn a_running_suggestion_without_spans_starts_at_its_open_start() {
        assert_eq!(running_suggestion("s1", "p1", 5_000).start(), Some(5_000));
    }

    #[test]
    fn the_open_start_is_serialized_only_while_it_runs() {
        let stopped = suggestion("s1", "p1", day("2026-01-05"), &[(1_000, 2_000)]);
        let running = running_suggestion("s1", "p1", 5_000);

        assert_eq!(
            serde_json::to_value(&stopped).unwrap().get("openStart"),
            None
        );
        assert_eq!(serde_json::to_value(&running).unwrap()["openStart"], 5_000);
    }

    #[test]
    fn a_stored_suggestion_serializes_with_camel_case_fields_and_the_day_as_text() {
        let s = StoredSuggestion {
            tag_id: Some("t1".into()),
            ..suggestion("s1", "p1", day("2026-01-05"), &[(1_000, 2_000)])
        };

        assert_eq!(
            serde_json::to_value(&s).unwrap(),
            json!({
                "id": "s1",
                "projectId": "p1",
                "note": "note",
                "tagId": "t1",
                "source": "cli",
                "day": "2026-01-05",
                "spans": [{"start": 1_000, "end": 2_000}],
                "createdAt": 1_000,
                "updatedAt": 2_000,
            })
        );
    }
}
