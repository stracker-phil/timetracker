//! Recordings and their tags. Every write is one transaction. The rule that recordings never overlap
//! lives in the recordings feature; `overlapping` is only the query.

use serde::{Deserialize, Deserializer, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, Row, SqliteConnection};
use ts_rs::TS;

use super::Db;
use super::time::{Ms, now_ms};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Recording {
    pub id: String,
    pub project_id: String,
    pub start: Ms,
    /// `None` only for the running timer.
    pub end: Option<Ms>,
    /// One line (ADR 0018).
    pub note: String,
    /// Ordered by tag name.
    pub tag_ids: Vec<String>,
    pub created_at: Ms,
    pub updated_at: Ms,
}

/// The caller picks the id, so an undo can restore a deleted recording under its old id.
#[derive(Debug, Clone, PartialEq)]
pub struct NewRecording {
    pub id: String,
    pub project_id: String,
    pub start: Ms,
    pub end: Option<Ms>,
    pub note: String,
    pub tag_ids: Vec<String>,
}

/// Absent fields stay as they are; `end: null` makes the recording running.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct RecordingPatch {
    #[ts(optional)]
    pub project_id: Option<String>,
    #[ts(optional)]
    pub start: Option<Ms>,
    #[serde(default, deserialize_with = "present")]
    #[ts(optional)]
    pub end: Option<Option<Ms>>,
    #[ts(optional)]
    pub note: Option<String>,
    #[ts(optional)]
    pub tag_ids: Option<Vec<String>>,
}

/// A field that is present, even as `null`, is `Some`.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

/// Selects every recording column plus `tag_ids`, a JSON array ordered by tag name.
macro_rules! select {
    ($tail:literal) => {
        concat!(
            r#"SELECT r.*, (SELECT json_group_array(tag_id) FROM (
                 SELECT rt.tag_id FROM recording_tags rt JOIN tags t ON t.id = rt.tag_id
                 WHERE rt.recording_id = r.id ORDER BY t.name COLLATE NOCASE)) AS tag_ids
               FROM recordings r "#,
            $tail
        )
    };
}

impl FromRow<'_, SqliteRow> for Recording {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        let tag_ids: String = row.try_get("tag_ids")?;
        Ok(Recording {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            start: row.try_get("start")?,
            end: row.try_get("end")?,
            note: row.try_get("note")?,
            tag_ids: serde_json::from_str(&tag_ids).map_err(|e| sqlx::Error::ColumnDecode {
                index: "tag_ids".into(),
                source: Box::new(e),
            })?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

pub async fn insert(db: &Db, new: NewRecording) -> AppResult<Recording> {
    let note = checked_note(&new.note)?;
    check_span(new.start, new.end)?;
    let mut tx = db.pool().begin().await?;
    check_project(&mut tx, &new.project_id).await?;
    let now = now_ms();
    sqlx::query(
        r#"INSERT INTO recordings (id, project_id, start, "end", note, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)"#,
    )
    .bind(&new.id)
    .bind(&new.project_id)
    .bind(new.start)
    .bind(new.end)
    .bind(note)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    write_tags(&mut tx, &new.id, &new.tag_ids).await?;
    let recording = fetch(&mut tx, &new.id).await?;
    tx.commit().await?;
    Ok(recording)
}

pub async fn get(db: &Db, id: &str) -> AppResult<Recording> {
    fetch(&mut *db.pool().acquire().await?, id).await
}

pub async fn update(db: &Db, id: &str, patch: RecordingPatch) -> AppResult<Recording> {
    let mut tx = db.pool().begin().await?;
    let current = fetch(&mut tx, id).await?;
    let project_id = patch.project_id.unwrap_or(current.project_id);
    let start = patch.start.unwrap_or(current.start);
    let end = patch.end.unwrap_or(current.end);
    let note = checked_note(patch.note.as_deref().unwrap_or(&current.note))?;
    check_span(start, end)?;
    check_project(&mut tx, &project_id).await?;
    sqlx::query(
        r#"UPDATE recordings SET project_id = ?2, start = ?3, "end" = ?4, note = ?5, updated_at = ?6
           WHERE id = ?1"#,
    )
    .bind(id)
    .bind(&project_id)
    .bind(start)
    .bind(end)
    .bind(note)
    .bind(now_ms())
    .execute(&mut *tx)
    .await?;
    if let Some(tag_ids) = &patch.tag_ids {
        write_tags(&mut tx, id, tag_ids).await?;
    }
    let recording = fetch(&mut tx, id).await?;
    tx.commit().await?;
    Ok(recording)
}

/// Returns the deleted recording, so it can be restored.
pub async fn delete(db: &Db, id: &str) -> AppResult<Recording> {
    let mut tx = db.pool().begin().await?;
    let recording = fetch(&mut tx, id).await?;
    sqlx::query("DELETE FROM recordings WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(recording)
}

/// Recordings that share time with `[from, to)`, the running one included, by start.
pub async fn list_for_range(db: &Db, from: Ms, to: Ms) -> AppResult<Vec<Recording>> {
    overlapping(db, from, to, None).await
}

/// The running timer: the recording without an end.
pub async fn running(db: &Db) -> AppResult<Option<Recording>> {
    Ok(sqlx::query_as(select!(
        r#"WHERE r."end" IS NULL ORDER BY r.start DESC LIMIT 1"#
    ))
    .fetch_optional(db.pool())
    .await?)
}

/// Recordings that share time with `[start, end)`, except the one with id `except`. Touching edges
/// do not overlap; the running recording overlaps everything after its start.
pub async fn overlapping(
    db: &Db,
    start: Ms,
    end: Ms,
    except: Option<&str>,
) -> AppResult<Vec<Recording>> {
    overlapping_on(&mut *db.pool().acquire().await?, start, end, except).await
}

pub async fn overlapping_on(
    conn: &mut SqliteConnection,
    start: Ms,
    end: Ms,
    except: Option<&str>,
) -> AppResult<Vec<Recording>> {
    Ok(sqlx::query_as(select!(
        r#"WHERE r.start < ?2 AND (r."end" IS NULL OR r."end" > ?1) AND (?3 IS NULL OR r.id != ?3)
           ORDER BY r.start, r.id"#
    ))
    .bind(start)
    .bind(end)
    .bind(except)
    .fetch_all(conn)
    .await?)
}

/// Writes `recording` exactly as given (timestamps and tags included), creating or replacing the row,
/// so an undo restores what was there. The caller validates.
pub async fn put_on(conn: &mut SqliteConnection, recording: &Recording) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO recordings (id, project_id, start, "end", note, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
           ON CONFLICT(id) DO UPDATE SET project_id = ?2, start = ?3, "end" = ?4, note = ?5,
             created_at = ?6, updated_at = ?7"#,
    )
    .bind(&recording.id)
    .bind(&recording.project_id)
    .bind(recording.start)
    .bind(recording.end)
    .bind(&recording.note)
    .bind(recording.created_at)
    .bind(recording.updated_at)
    .execute(&mut *conn)
    .await?;
    write_tags(conn, &recording.id, &recording.tag_ids).await
}

/// Deletes the row; `not_found` when there is none.
pub async fn remove_on(conn: &mut SqliteConnection, id: &str) -> AppResult<()> {
    let done = sqlx::query("DELETE FROM recordings WHERE id = ?1")
        .bind(id)
        .execute(conn)
        .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("No recording {id}")));
    }
    Ok(())
}

pub async fn get_on(conn: &mut SqliteConnection, id: &str) -> AppResult<Recording> {
    fetch(conn, id).await
}

pub async fn count(db: &Db) -> AppResult<i64> {
    Ok(sqlx::query_scalar("SELECT count(*) FROM recordings")
        .fetch_one(db.pool())
        .await?)
}

pub(super) async fn all(conn: &mut SqliteConnection) -> AppResult<Vec<Recording>> {
    Ok(sqlx::query_as(select!("ORDER BY r.start, r.id"))
        .fetch_all(conn)
        .await?)
}

async fn fetch(conn: &mut SqliteConnection, id: &str) -> AppResult<Recording> {
    sqlx::query_as(select!("WHERE r.id = ?1"))
        .bind(id)
        .fetch_optional(conn)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("No recording {id}")))
}

/// Replaces the recording's tags; every id must name a tag.
async fn write_tags(conn: &mut SqliteConnection, id: &str, tag_ids: &[String]) -> AppResult<()> {
    sqlx::query("DELETE FROM recording_tags WHERE recording_id = ?1")
        .bind(id)
        .execute(&mut *conn)
        .await?;
    for tag_id in tag_ids {
        ensure_exists(conn, "SELECT 1 FROM tags WHERE id = ?1", "tag", tag_id).await?;
        sqlx::query("INSERT OR IGNORE INTO recording_tags (recording_id, tag_id) VALUES (?1, ?2)")
            .bind(id)
            .bind(tag_id)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

async fn check_project(conn: &mut SqliteConnection, project_id: &str) -> AppResult<()> {
    ensure_exists(
        conn,
        "SELECT 1 FROM projects WHERE id = ?1",
        "project",
        project_id,
    )
    .await
}

/// `not_found` unless `sql` (selecting by `?1`) returns a row.
async fn ensure_exists(
    conn: &mut SqliteConnection,
    sql: &'static str,
    what: &str,
    id: &str,
) -> AppResult<()> {
    let row: Option<i64> = sqlx::query_scalar(sql)
        .bind(id)
        .fetch_optional(conn)
        .await?;
    row.map(|_| ())
        .ok_or_else(|| AppError::NotFound(format!("No {what} {id}")))
}

/// Notes are one line (ADR 0018), stored trimmed.
fn checked_note(note: &str) -> AppResult<&str> {
    if note.contains(['\n', '\r']) {
        return Err(AppError::InvalidInput("A note is a single line".into()));
    }
    Ok(note.trim())
}

fn check_span(start: Ms, end: Option<Ms>) -> AppResult<()> {
    match end {
        Some(end) if end <= start => Err(AppError::InvalidInput(
            "A recording must end after it starts".into(),
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tags;
    use crate::error::ErrorKind;
    use crate::test_support::{project, recording};

    fn new(project_id: &str, start: Ms, end: Option<Ms>) -> NewRecording {
        NewRecording {
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project_id.into(),
            start,
            end,
            note: String::new(),
            tag_ids: Vec::new(),
        }
    }

    fn ids(recordings: Vec<Recording>) -> Vec<String> {
        recordings.into_iter().map(|r| r.id).collect()
    }

    #[tokio::test]
    async fn insert_returns_tags_ordered_by_tag_name() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let zeta = tags::insert(&db, "zeta").await.unwrap();
        let alpha = tags::insert(&db, "Alpha").await.unwrap();

        let rec = insert(
            &db,
            NewRecording {
                note: "  wrote docs ".into(),
                tag_ids: vec![zeta.id.clone(), alpha.id.clone()],
                ..new(&p.id, 100, Some(200))
            },
        )
        .await
        .unwrap();

        assert_eq!(rec.tag_ids, [alpha.id, zeta.id]);
        assert_eq!(rec.note, "wrote docs");
        assert_eq!((rec.start, rec.end), (100, Some(200)));
        assert_eq!(rec.created_at, rec.updated_at);
        assert_eq!(get(&db, &rec.id).await.unwrap(), rec);
        assert_eq!(count(&db).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn an_unknown_tag_leaves_no_recording_behind() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let err = insert(
            &db,
            NewRecording {
                tag_ids: vec!["nope".into()],
                ..new(&p.id, 100, Some(200))
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(count(&db).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn an_unknown_project_is_not_found() {
        let db = Db::open_memory().await.unwrap();
        let err = insert(&db, new("nope", 100, Some(200))).await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(count(&db).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn multi_line_notes_are_rejected_on_insert_and_update() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let rec = recording(&db, &p.id, 100, Some(200)).await;
        for note in ["a\nb", "a\rb", "a\r\nb"] {
            let err = insert(
                &db,
                NewRecording {
                    note: note.into(),
                    ..new(&p.id, 300, Some(400))
                },
            )
            .await
            .unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "insert {note:?}");
            let patch = RecordingPatch {
                note: Some(note.into()),
                ..Default::default()
            };
            let err = update(&db, &rec.id, patch).await.unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "update {note:?}");
        }
        assert_eq!(count(&db).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn a_recording_must_end_after_it_starts() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let rec = recording(&db, &p.id, 100, Some(200)).await;
        for end in [100, 50] {
            let err = insert(&db, new(&p.id, 100, Some(end))).await.unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "insert end {end}");
            let patch = RecordingPatch {
                end: Some(Some(end)),
                ..Default::default()
            };
            let err = update(&db, &rec.id, patch).await.unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "update end {end}");
        }
    }

    #[tokio::test]
    async fn a_recording_without_end_is_the_running_one() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        assert_eq!(running(&db).await.unwrap(), None);
        recording(&db, &p.id, 100, Some(200)).await;
        let timer = recording(&db, &p.id, 300, None).await;
        assert_eq!(timer.end, None);
        assert_eq!(running(&db).await.unwrap(), Some(timer));
    }

    #[tokio::test]
    async fn unknown_recordings_are_not_found() {
        let db = Db::open_memory().await.unwrap();
        assert_eq!(
            get(&db, "nope").await.unwrap_err().kind(),
            ErrorKind::NotFound
        );
        assert_eq!(
            update(&db, "nope", RecordingPatch::default())
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::NotFound
        );
        assert_eq!(
            delete(&db, "nope").await.unwrap_err().kind(),
            ErrorKind::NotFound
        );
    }

    #[tokio::test]
    async fn update_changes_only_the_given_fields() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let other = project(&db, "Q").await;
        let tag = tags::insert(&db, "focus").await.unwrap();
        let rec = insert(
            &db,
            NewRecording {
                note: "keep".into(),
                tag_ids: vec![tag.id.clone()],
                ..new(&p.id, 100, Some(200))
            },
        )
        .await
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;

        let patch = RecordingPatch {
            project_id: Some(other.id.clone()),
            start: Some(150),
            ..Default::default()
        };
        let updated = update(&db, &rec.id, patch).await.unwrap();

        assert_eq!(
            Recording {
                project_id: other.id,
                start: 150,
                updated_at: updated.updated_at,
                ..rec.clone()
            },
            updated
        );
        assert!(updated.updated_at > rec.updated_at);
        assert_eq!(updated.created_at, rec.created_at);
    }

    #[tokio::test]
    async fn update_can_clear_tags_and_make_the_recording_running() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let tag = tags::insert(&db, "focus").await.unwrap();
        let rec = insert(
            &db,
            NewRecording {
                tag_ids: vec![tag.id],
                ..new(&p.id, 100, Some(200))
            },
        )
        .await
        .unwrap();

        let patch = RecordingPatch {
            tag_ids: Some(vec![]),
            end: Some(None),
            note: Some(" new ".into()),
            ..Default::default()
        };
        let updated = update(&db, &rec.id, patch).await.unwrap();

        assert!(updated.tag_ids.is_empty());
        assert_eq!(updated.end, None);
        assert_eq!(updated.note, "new");
        assert_eq!(running(&db).await.unwrap().unwrap().id, rec.id);
    }

    #[tokio::test]
    async fn update_with_an_unknown_tag_or_project_changes_nothing() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let rec = recording(&db, &p.id, 100, Some(200)).await;

        let bad_tag = RecordingPatch {
            start: Some(10),
            tag_ids: Some(vec!["nope".into()]),
            ..Default::default()
        };
        assert_eq!(
            update(&db, &rec.id, bad_tag).await.unwrap_err().kind(),
            ErrorKind::NotFound
        );
        let bad_project = RecordingPatch {
            project_id: Some("nope".into()),
            ..Default::default()
        };
        assert_eq!(
            update(&db, &rec.id, bad_project).await.unwrap_err().kind(),
            ErrorKind::NotFound
        );

        assert_eq!(get(&db, &rec.id).await.unwrap(), rec);
    }

    #[test]
    fn patch_distinguishes_a_null_end_from_an_absent_one() {
        let null: RecordingPatch = serde_json::from_str(r#"{"end": null}"#).unwrap();
        assert_eq!(null.end, Some(None));
        let set: RecordingPatch = serde_json::from_str(r#"{"end": 5}"#).unwrap();
        assert_eq!(set.end, Some(Some(5)));
        let absent: RecordingPatch = serde_json::from_str("{}").unwrap();
        assert_eq!(absent, RecordingPatch::default());
        assert_eq!(absent.end, None);
    }

    #[tokio::test]
    async fn delete_returns_the_recording_with_its_tags_and_removes_it() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let tag = tags::insert(&db, "focus").await.unwrap();
        let rec = insert(
            &db,
            NewRecording {
                tag_ids: vec![tag.id],
                ..new(&p.id, 100, Some(200))
            },
        )
        .await
        .unwrap();

        assert_eq!(delete(&db, &rec.id).await.unwrap(), rec);

        assert_eq!(
            get(&db, &rec.id).await.unwrap_err().kind(),
            ErrorKind::NotFound
        );
        assert_eq!(count(&db).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn list_for_range_includes_crossing_and_running_recordings_by_start() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let timer = recording(&db, &p.id, 300, None).await;
        let crossing = recording(&db, &p.id, 50, Some(150)).await;
        let before = recording(&db, &p.id, 0, Some(40)).await;

        assert_eq!(
            ids(list_for_range(&db, 0, 100).await.unwrap()),
            [before.id, crossing.id.clone()]
        );
        assert_eq!(
            ids(list_for_range(&db, 100, 200).await.unwrap()),
            [crossing.id]
        );
        assert_eq!(
            ids(list_for_range(&db, 200, 400).await.unwrap()),
            [timer.id]
        );
    }

    #[tokio::test]
    async fn overlapping_ignores_touching_edges_and_the_excepted_recording() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let a = recording(&db, &p.id, 100, Some(200)).await;
        let b = recording(&db, &p.id, 200, Some(300)).await;
        let timer = recording(&db, &p.id, 500, None).await;

        let (a, b, timer) = (a.id.as_str(), b.id.as_str(), timer.id.as_str());
        let cases: [(Ms, Ms, Option<&str>, Vec<&str>); 8] = [
            (0, 100, None, vec![]),
            (300, 500, None, vec![]),
            (110, 190, None, vec![a]),
            (50, 400, None, vec![a, b]),
            (150, 250, None, vec![a, b]),
            (150, 250, Some(a), vec![b]),
            (90, 120, Some(a), vec![]),
            (10_000, 20_000, None, vec![timer]),
        ];
        for (start, end, except, expected) in cases {
            let found = ids(overlapping(&db, start, end, except).await.unwrap());
            assert_eq!(found, expected, "[{start}, {end}) except {except:?}");
        }
    }

    #[tokio::test]
    async fn count_counts_recordings() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        assert_eq!(count(&db).await.unwrap(), 0);
        recording(&db, &p.id, 100, Some(200)).await;
        recording(&db, &p.id, 200, Some(300)).await;
        assert_eq!(count(&db).await.unwrap(), 2);
    }
}
