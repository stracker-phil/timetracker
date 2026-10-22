//! Recordings, each with at most one tag (ADR 0019) and one or more spans of time (ADR 0020). Every write
//! is one transaction. The rule that recordings never overlap lives in the recordings feature;
//! `overlapping` is only the query.

mod model;

use std::collections::HashMap;

use sqlx::SqliteConnection;

pub use model::{
    NewRecording, Recording, RecordingPatch, RecordingSpan, Span, check_spans, respanned,
};

use super::Db;
use super::time::{Ms, now_ms};
use crate::error::{AppError, AppResult};

/// Orders recordings by the start of their first span.
macro_rules! by_first_start {
    () => {
        " ORDER BY (SELECT min(s.start) FROM recording_spans s WHERE s.recording_id = r.id), r.id"
    };
}

pub async fn insert(db: &Db, new: NewRecording) -> AppResult<Recording> {
    let note = checked_note(&new.note)?.to_string();
    let mut spans: Vec<RecordingSpan> = new
        .spans
        .iter()
        .map(|&(start, end)| RecordingSpan::new(start, end))
        .collect();
    spans.sort_by_key(|s| s.start);
    check_spans(&spans)?;
    let mut tx = db.pool().begin().await?;
    check_project(&mut tx, &new.project_id).await?;
    ensure_tag(&mut tx, new.tag_id.as_deref()).await?;
    let now = now_ms();
    let recording = Recording {
        id: new.id,
        project_id: new.project_id,
        note,
        tag_id: new.tag_id,
        created_at: now,
        updated_at: now,
        spans,
    };
    write(&mut tx, &recording).await?;
    let recording = fetch(&mut tx, &recording.id).await?;
    tx.commit().await?;
    Ok(recording)
}

pub async fn get(db: &Db, id: &str) -> AppResult<Recording> {
    fetch(&mut *db.pool().acquire().await?, id).await
}

pub async fn update(db: &Db, id: &str, patch: RecordingPatch) -> AppResult<Recording> {
    let mut tx = db.pool().begin().await?;
    let current = fetch(&mut tx, id).await?;
    let spans = match &patch.spans {
        Some(spans) => respanned(&current.spans, spans),
        None => current.spans.clone(),
    };
    check_spans(&spans)?;
    let note = checked_note(patch.note.as_deref().unwrap_or(&current.note))?.to_string();
    let updated = Recording {
        project_id: patch.project_id.unwrap_or(current.project_id),
        note,
        tag_id: patch.tag_id.unwrap_or(current.tag_id),
        updated_at: now_ms(),
        spans,
        ..current
    };
    check_project(&mut tx, &updated.project_id).await?;
    ensure_tag(&mut tx, updated.tag_id.as_deref()).await?;
    write(&mut tx, &updated).await?;
    let recording = fetch(&mut tx, id).await?;
    tx.commit().await?;
    Ok(recording)
}

/// Returns the deleted recording, so it can be restored.
pub async fn delete(db: &Db, id: &str) -> AppResult<Recording> {
    let mut tx = db.pool().begin().await?;
    let recording = fetch(&mut tx, id).await?;
    remove_on(&mut tx, id).await?;
    tx.commit().await?;
    Ok(recording)
}

/// Recordings with a span that shares time with `[from, to)`, the running one included, by first start.
/// Every span is included, also those outside the range.
pub async fn list_for_range(db: &Db, from: Ms, to: Ms) -> AppResult<Vec<Recording>> {
    overlapping(db, from, to, None).await
}

/// The running timer: the recording with an open span.
pub async fn running(db: &Db) -> AppResult<Option<Recording>> {
    let mut conn = db.pool().acquire().await?;
    let row: Option<Recording> = sqlx::query_as(
        r#"SELECT r.* FROM recordings r JOIN recording_spans s ON s.recording_id = r.id
           WHERE s."end" IS NULL ORDER BY s.start DESC LIMIT 1"#,
    )
    .fetch_optional(&mut *conn)
    .await?;
    Ok(with_spans(&mut conn, row.into_iter().collect())
        .await?
        .pop())
}

/// Finished recordings, the one with the latest span first.
pub async fn latest(db: &Db, limit: u32) -> AppResult<Vec<Recording>> {
    let mut conn = db.pool().acquire().await?;
    let rows = sqlx::query_as(
        r#"SELECT r.* FROM recordings r
           WHERE NOT EXISTS (SELECT 1 FROM recording_spans s WHERE s.recording_id = r.id
             AND s."end" IS NULL)
           ORDER BY (SELECT max(s.start) FROM recording_spans s WHERE s.recording_id = r.id) DESC
           LIMIT ?1"#,
    )
    .bind(limit)
    .fetch_all(&mut *conn)
    .await?;
    with_spans(&mut conn, rows).await
}

/// The project and tag of the recording with the latest span whose note is `note` (ignoring ASCII case), on an
/// active project.
pub async fn latest_with_note(db: &Db, note: &str) -> AppResult<Option<(String, Option<String>)>> {
    Ok(sqlx::query_as(
        r#"SELECT r.project_id, r.tag_id FROM recordings r JOIN projects p ON p.id = r.project_id
           WHERE p.archived_at IS NULL AND r.note = ?1 COLLATE NOCASE
           ORDER BY (SELECT max(s.start) FROM recording_spans s WHERE s.recording_id = r.id) DESC
           LIMIT 1"#,
    )
    .bind(note)
    .fetch_optional(db.pool())
    .await?)
}

/// Recordings with a span that shares time with `[start, end)`, except the one with id `except`, by first
/// start. Touching edges do not overlap; an open span overlaps everything after its start.
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
    let rows = sqlx::query_as(concat!(
        r#"SELECT r.* FROM recordings r
           WHERE EXISTS (SELECT 1 FROM recording_spans s WHERE s.recording_id = r.id
             AND s.start < ?2 AND (s."end" IS NULL OR s."end" > ?1))
           AND (?3 IS NULL OR r.id != ?3)"#,
        by_first_start!()
    ))
    .bind(start)
    .bind(end)
    .bind(except)
    .fetch_all(&mut *conn)
    .await?;
    with_spans(conn, rows).await
}

/// Writes `recording` exactly as given (timestamps, tag and spans included), creating or replacing the
/// row and its spans, so an undo restores what was there. The caller validates.
pub async fn put_on(conn: &mut SqliteConnection, recording: &Recording) -> AppResult<()> {
    ensure_tag(conn, recording.tag_id.as_deref()).await?;
    write(conn, recording).await
}

async fn write(conn: &mut SqliteConnection, recording: &Recording) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO recordings (id, project_id, note, tag_id, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6)
           ON CONFLICT(id) DO UPDATE SET project_id = ?2, note = ?3, tag_id = ?4, created_at = ?5,
             updated_at = ?6"#,
    )
    .bind(&recording.id)
    .bind(&recording.project_id)
    .bind(&recording.note)
    .bind(&recording.tag_id)
    .bind(recording.created_at)
    .bind(recording.updated_at)
    .execute(&mut *conn)
    .await?;
    sqlx::query("DELETE FROM recording_spans WHERE recording_id = ?1")
        .bind(&recording.id)
        .execute(&mut *conn)
        .await?;
    for span in &recording.spans {
        sqlx::query(
            r#"INSERT INTO recording_spans (id, recording_id, start, "end") VALUES (?1, ?2, ?3, ?4)
               ON CONFLICT(id) DO UPDATE SET recording_id = ?2, start = ?3, "end" = ?4"#,
        )
        .bind(&span.id)
        .bind(&recording.id)
        .bind(span.start)
        .bind(span.end)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Deletes the row, and its spans with it (`ON DELETE CASCADE`); `not_found` when there is none.
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

/// The start and end (`None` while running) of every span that shares time with `[from, to)`, by start.
pub async fn spans_in(db: &Db, from: Ms, to: Ms) -> AppResult<Vec<(Ms, Option<Ms>)>> {
    Ok(sqlx::query_as(
        r#"SELECT start, "end" FROM recording_spans
           WHERE start < ?2 AND ("end" IS NULL OR "end" > ?1) ORDER BY start"#,
    )
    .bind(from)
    .bind(to)
    .fetch_all(db.pool())
    .await?)
}

pub async fn count(db: &Db) -> AppResult<i64> {
    Ok(sqlx::query_scalar("SELECT count(*) FROM recordings")
        .fetch_one(db.pool())
        .await?)
}

pub(super) async fn all(conn: &mut SqliteConnection) -> AppResult<Vec<Recording>> {
    let rows = sqlx::query_as(concat!("SELECT r.* FROM recordings r", by_first_start!()))
        .fetch_all(&mut *conn)
        .await?;
    with_spans(conn, rows).await
}

async fn fetch(conn: &mut SqliteConnection, id: &str) -> AppResult<Recording> {
    let row: Recording = sqlx::query_as("SELECT r.* FROM recordings r WHERE r.id = ?1")
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("No recording {id}")))?;
    let mut rows = with_spans(conn, vec![row]).await?;
    Ok(rows.remove(0))
}

/// `rows` with their spans, by start.
async fn with_spans(
    conn: &mut SqliteConnection,
    mut rows: Vec<Recording>,
) -> AppResult<Vec<Recording>> {
    if rows.is_empty() {
        return Ok(rows);
    }
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    let spans: Vec<(String, String, Ms, Option<Ms>)> = sqlx::query_as(
        r#"SELECT recording_id, id, start, "end" FROM recording_spans
           WHERE recording_id IN (SELECT value FROM json_each(?1)) ORDER BY start, id"#,
    )
    .bind(serde_json::to_string(&ids)?)
    .fetch_all(&mut *conn)
    .await?;
    let mut by_recording: HashMap<String, Vec<RecordingSpan>> = HashMap::new();
    for (recording_id, id, start, end) in spans {
        by_recording
            .entry(recording_id)
            .or_default()
            .push(RecordingSpan { id, start, end });
    }
    for row in &mut rows {
        row.spans = by_recording.remove(&row.id).unwrap_or_default();
    }
    Ok(rows)
}

async fn ensure_tag(conn: &mut SqliteConnection, tag_id: Option<&str>) -> AppResult<()> {
    match tag_id {
        Some(id) => ensure_exists(conn, "SELECT 1 FROM tags WHERE id = ?1", "tag", id).await,
        None => Ok(()),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tags;
    use crate::error::ErrorKind;
    use crate::test_support::{project, recording};

    fn new(project_id: &str, start: Ms, end: Option<Ms>) -> NewRecording {
        new_spans(project_id, &[(start, end)])
    }

    fn new_spans(project_id: &str, spans: &[(Ms, Option<Ms>)]) -> NewRecording {
        NewRecording {
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project_id.into(),
            spans: spans.to_vec(),
            note: String::new(),
            tag_id: None,
        }
    }

    fn ids(recordings: Vec<Recording>) -> Vec<String> {
        recordings.into_iter().map(|r| r.id).collect()
    }

    fn times(recording: &Recording) -> Vec<(Ms, Option<Ms>)> {
        recording.spans.iter().map(|s| (s.start, s.end)).collect()
    }

    fn span(start: Ms, end: Ms) -> Span {
        Span { start, end }
    }

    async fn multi(db: &Db, project_id: &str, spans: &[(Ms, Option<Ms>)]) -> Recording {
        insert(db, new_spans(project_id, spans)).await.unwrap()
    }

    #[tokio::test]
    async fn insert_stores_the_tag_and_trims_the_note() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let zeta = tags::insert(&db, "zeta").await.unwrap();

        let rec = insert(
            &db,
            NewRecording {
                note: "  wrote docs ".into(),
                tag_id: Some(zeta.id.clone()),
                ..new(&p.id, 100, Some(200))
            },
        )
        .await
        .unwrap();

        assert_eq!(rec.tag_id, Some(zeta.id));
        assert_eq!(rec.note, "wrote docs");
        assert_eq!((rec.start(), rec.end()), (100, Some(200)));
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
                tag_id: Some("nope".into()),
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
                spans: Some(vec![span(100, end)]),
                ..Default::default()
            };
            let err = update(&db, &rec.id, patch).await.unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "update end {end}");
        }
        assert_eq!(get(&db, &rec.id).await.unwrap(), rec);
    }

    #[tokio::test]
    async fn invalid_span_lists_are_rejected_on_insert_and_update() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let rec = recording(&db, &p.id, 100, Some(200)).await;
        let cases: [&[(Ms, Option<Ms>)]; 3] = [
            &[],
            &[(100, Some(300)), (200, Some(400))],
            &[(100, None), (200, Some(300))],
        ];
        for case in cases {
            let err = insert(&db, new_spans(&p.id, case)).await.unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "insert {case:?}");
        }
        let overlapping = RecordingPatch {
            spans: Some(vec![span(100, 300), span(200, 400)]),
            ..Default::default()
        };
        let err = update(&db, &rec.id, overlapping).await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        let empty = RecordingPatch {
            spans: Some(vec![]),
            ..Default::default()
        };
        let err = update(&db, &rec.id, empty).await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(get(&db, &rec.id).await.unwrap(), rec);
        assert_eq!(count(&db).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn insert_sorts_the_spans_and_allows_touching_ones() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;

        let rec = multi(
            &db,
            &p.id,
            &[(300, Some(400)), (100, Some(200)), (200, Some(300))],
        )
        .await;

        assert_eq!(
            times(&rec),
            [(100, Some(200)), (200, Some(300)), (300, Some(400))]
        );
        assert_eq!((rec.start(), rec.end()), (100, Some(400)));
        assert_eq!(rec.duration(10_000), 300);
        assert_eq!(get(&db, &rec.id).await.unwrap(), rec);
    }

    #[tokio::test]
    async fn a_recording_with_an_open_span_is_the_running_one() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        assert_eq!(running(&db).await.unwrap(), None);
        recording(&db, &p.id, 100, Some(200)).await;
        let timer = recording(&db, &p.id, 300, None).await;
        assert_eq!(timer.end(), None);
        assert_eq!(running(&db).await.unwrap(), Some(timer));
    }

    #[tokio::test]
    async fn running_finds_the_open_span_of_a_multi_span_recording() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        multi(&db, &p.id, &[(10, Some(20)), (30, Some(40))]).await;
        let timer = multi(&db, &p.id, &[(100, Some(200)), (300, None)]).await;

        let found = running(&db).await.unwrap().unwrap();

        assert_eq!(found, timer);
        assert_eq!(found.open_span().unwrap().id, timer.spans[1].id);
        assert_eq!(found.spans.len(), 2);
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
                tag_id: Some(tag.id.clone()),
                ..new(&p.id, 100, Some(200))
            },
        )
        .await
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;

        let patch = RecordingPatch {
            project_id: Some(other.id.clone()),
            spans: Some(vec![span(150, 200)]),
            ..Default::default()
        };
        let updated = update(&db, &rec.id, patch).await.unwrap();

        assert_eq!(
            Recording {
                project_id: other.id,
                spans: vec![RecordingSpan {
                    id: rec.spans[0].id.clone(),
                    start: 150,
                    end: Some(200),
                }],
                updated_at: updated.updated_at,
                ..rec.clone()
            },
            updated
        );
        assert!(updated.updated_at > rec.updated_at);
        assert_eq!(updated.created_at, rec.created_at);
    }

    #[tokio::test]
    async fn update_can_clear_the_tag_and_trims_the_note() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let tag = tags::insert(&db, "focus").await.unwrap();
        let rec = insert(
            &db,
            NewRecording {
                tag_id: Some(tag.id),
                ..new(&p.id, 100, Some(200))
            },
        )
        .await
        .unwrap();

        let patch = RecordingPatch {
            tag_id: Some(None),
            note: Some(" new ".into()),
            ..Default::default()
        };
        let updated = update(&db, &rec.id, patch).await.unwrap();

        assert_eq!(updated.tag_id, None);
        assert_eq!(updated.note, "new");
        assert_eq!(updated.spans, rec.spans);
    }

    #[tokio::test]
    async fn update_with_spans_keeps_the_ids_by_position() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let rec = multi(&db, &p.id, &[(100, Some(200)), (300, Some(400))]).await;
        let (first, second) = (rec.spans[0].id.clone(), rec.spans[1].id.clone());

        let patch = RecordingPatch {
            spans: Some(vec![span(700, 800), span(500, 600), span(100, 200)]),
            ..Default::default()
        };
        let grown = update(&db, &rec.id, patch).await.unwrap();

        assert_eq!(
            times(&grown),
            [(100, Some(200)), (500, Some(600)), (700, Some(800))]
        );
        assert_eq!(grown.spans[0].id, first);
        assert_eq!(grown.spans[1].id, second);
        assert!(![&first, &second].contains(&&grown.spans[2].id));
        assert_eq!(get(&db, &rec.id).await.unwrap(), grown);

        let patch = RecordingPatch {
            spans: Some(vec![span(900, 950)]),
            ..Default::default()
        };
        let shrunk = update(&db, &rec.id, patch).await.unwrap();

        assert_eq!(
            shrunk.spans,
            [RecordingSpan {
                id: first,
                start: 900,
                end: Some(950),
            }]
        );
        let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM recording_spans")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(stored, 1);
    }

    #[tokio::test]
    async fn update_sets_and_replaces_the_tag() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let first = tags::insert(&db, "first").await.unwrap();
        let second = tags::insert(&db, "second").await.unwrap();
        let rec = recording(&db, &p.id, 100, Some(200)).await;
        assert_eq!(rec.tag_id, None);

        for tag in [&first, &second] {
            let patch = RecordingPatch {
                tag_id: Some(Some(tag.id.clone())),
                ..Default::default()
            };
            let updated = update(&db, &rec.id, patch).await.unwrap();
            assert_eq!(updated.tag_id, Some(tag.id.clone()), "{}", tag.name);
            assert_eq!(get(&db, &rec.id).await.unwrap(), updated);
        }
    }

    #[tokio::test]
    async fn an_absent_tag_id_keeps_the_tag_and_a_null_one_clears_it() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let tag = tags::insert(&db, "focus").await.unwrap();
        let rec = insert(
            &db,
            NewRecording {
                tag_id: Some(tag.id.clone()),
                ..new(&p.id, 100, Some(200))
            },
        )
        .await
        .unwrap();

        let absent: RecordingPatch = serde_json::from_str(r#"{"note": "x"}"#).unwrap();
        let kept = update(&db, &rec.id, absent).await.unwrap();
        assert_eq!(kept.tag_id, Some(tag.id));

        let null: RecordingPatch = serde_json::from_str(r#"{"tagId": null}"#).unwrap();
        let cleared = update(&db, &rec.id, null).await.unwrap();
        assert_eq!(cleared.tag_id, None);
    }

    #[tokio::test]
    async fn update_with_an_unknown_tag_or_project_changes_nothing() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let rec = recording(&db, &p.id, 100, Some(200)).await;

        let bad_tag = RecordingPatch {
            spans: Some(vec![span(10, 200)]),
            tag_id: Some(Some("nope".into())),
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

    #[tokio::test]
    async fn delete_returns_the_recording_with_its_tag_and_removes_it() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let tag = tags::insert(&db, "focus").await.unwrap();
        let rec = insert(
            &db,
            NewRecording {
                tag_id: Some(tag.id),
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
    async fn delete_returns_a_multi_span_recording_and_removes_its_spans() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let rec = multi(&db, &p.id, &[(100, Some(200)), (300, Some(400))]).await;

        assert_eq!(delete(&db, &rec.id).await.unwrap(), rec);

        let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM recording_spans")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(stored, 0);
    }

    #[tokio::test]
    async fn put_on_replaces_the_spans_and_restores_a_deleted_recording() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let rec = multi(&db, &p.id, &[(100, Some(200)), (300, Some(400))]).await;
        let edited = Recording {
            note: "edited".into(),
            spans: vec![RecordingSpan::new(500, Some(600))],
            ..rec.clone()
        };
        let mut conn = db.pool().acquire().await.unwrap();

        put_on(&mut conn, &edited).await.unwrap();
        assert_eq!(get_on(&mut conn, &rec.id).await.unwrap(), edited);

        remove_on(&mut conn, &rec.id).await.unwrap();
        put_on(&mut conn, &rec).await.unwrap();
        assert_eq!(get_on(&mut conn, &rec.id).await.unwrap(), rec);
    }

    #[tokio::test]
    async fn remove_on_deletes_the_spans_and_reports_a_missing_recording() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let rec = multi(&db, &p.id, &[(100, Some(200)), (300, Some(400))]).await;
        let mut conn = db.pool().acquire().await.unwrap();

        remove_on(&mut conn, &rec.id).await.unwrap();

        let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM recording_spans")
            .fetch_one(&mut *conn)
            .await
            .unwrap();
        assert_eq!(stored, 0);
        let err = remove_on(&mut conn, &rec.id).await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn list_for_range_returns_a_multi_span_recording_whole_when_any_span_hits() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let late = multi(&db, &p.id, &[(1_000, Some(1_100))]).await;
        let split = multi(&db, &p.id, &[(100, Some(200)), (500, Some(600))]).await;
        let early = multi(&db, &p.id, &[(0, Some(50))]).await;

        let first = list_for_range(&db, 550, 560).await.unwrap();
        assert_eq!(ids(first.clone()), [split.id.clone()]);
        assert_eq!(first[0].spans, split.spans);

        assert_eq!(
            ids(list_for_range(&db, 0, 2_000).await.unwrap()),
            [early.id, split.id.clone(), late.id]
        );
        assert!(list_for_range(&db, 250, 450).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn latest_skips_the_running_recording_and_orders_by_the_latest_span() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let spread = multi(&db, &p.id, &[(100, Some(150)), (1_000, Some(1_100))]).await;
        let middle = multi(&db, &p.id, &[(500, Some(600))]).await;
        multi(&db, &p.id, &[(50, Some(60)), (2_000, None)]).await;

        assert_eq!(
            ids(latest(&db, 10).await.unwrap()),
            [spread.id.clone(), middle.id]
        );
        assert_eq!(ids(latest(&db, 1).await.unwrap()), [spread.id]);
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
    async fn overlapping_matches_per_span_so_gaps_do_not_count() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        let split = multi(&db, &p.id, &[(100, Some(200)), (400, Some(500))]).await;
        let open = multi(&db, &p.id, &[(10, Some(20)), (900, None)]).await;

        let (split_id, open_id) = (split.id.as_str(), open.id.as_str());
        let cases: [(Ms, Ms, Option<&str>, Vec<&str>); 8] = [
            (200, 400, None, vec![]),
            (250, 350, None, vec![]),
            (150, 450, None, vec![split_id]),
            (450, 460, None, vec![split_id]),
            (500, 900, None, vec![]),
            (0, 15, None, vec![open_id]),
            (950, 960, None, vec![open_id]),
            (0, 2_000, Some(split_id), vec![open_id]),
        ];
        for (start, end, except, expected) in cases {
            let found = overlapping(&db, start, end, except).await.unwrap();
            assert_eq!(ids(found), expected, "[{start}, {end}) except {except:?}");
        }
        let found = overlapping(&db, 450, 460, None).await.unwrap();
        assert_eq!(found[0].spans, split.spans);
    }

    #[tokio::test]
    async fn spans_in_lists_the_spans_sharing_time_with_the_window_by_start() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "P").await;
        for (start, end) in [
            (400, Some(450)),
            (50, Some(100)),
            (190, Some(300)),
            (90, Some(150)),
            (200, Some(250)),
            (600, None),
        ] {
            recording(&db, &p.id, start, end).await;
        }

        assert_eq!(
            spans_in(&db, 100, 200).await.unwrap(),
            [(90, Some(150)), (190, Some(300))]
        );
        assert_eq!(spans_in(&db, 1_000, 2_000).await.unwrap(), [(600, None)]);
        assert!(spans_in(&db, 0, 50).await.unwrap().is_empty());
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
