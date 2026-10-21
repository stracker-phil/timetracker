//! The JSON backup: every row of the data store, read in one transaction so it is consistent.

use std::path::Path;

use serde::Serialize;
use ts_rs::TS;

use super::activities::ActivityRow;
use super::away::AwayRow;
use super::clients::Client;
use super::projects::Project;
use super::recordings::Recording;
use super::tags::Tag;
use super::time::{Ms, now_ms};
use super::{Db, activities, away, clients, projects, recordings, tags};
use crate::error::{AppError, AppResult};

pub const BACKUP_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Backup {
    pub version: u32,
    pub exported_at: Ms,
    pub clients: Vec<Client>,
    pub projects: Vec<Project>,
    pub tags: Vec<Tag>,
    pub activities: Vec<ActivityRow>,
    pub away_spans: Vec<AwayRow>,
    pub recordings: Vec<Recording>,
    /// Filled once rules exist.
    #[ts(type = "unknown[]")]
    pub rules: Vec<serde_json::Value>,
    /// Filled once saved reports exist.
    #[ts(type = "unknown[]")]
    pub saved_reports: Vec<serde_json::Value>,
}

pub async fn export(db: &Db) -> AppResult<Backup> {
    let mut tx = db.pool().begin().await?;
    let backup = Backup {
        version: BACKUP_VERSION,
        exported_at: now_ms(),
        clients: clients::all(&mut tx).await?,
        projects: projects::all(&mut tx).await?,
        tags: tags::all(&mut tx).await?,
        activities: activities::all(&mut tx).await?,
        away_spans: away::all(&mut tx).await?,
        recordings: recordings::all(&mut tx).await?,
        rules: Vec::new(),
        saved_reports: Vec::new(),
    };
    tx.commit().await?;
    Ok(backup)
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct BackupWritten {
    pub bytes: u64,
}

/// Writes the backup as pretty JSON to an absolute path, through a temp file beside it.
pub async fn write_to(db: &Db, path: &Path) -> AppResult<BackupWritten> {
    if !path.is_absolute() {
        return Err(AppError::InvalidInput(format!(
            "Not an absolute path: {}",
            path.display()
        )));
    }
    let json = serde_json::to_vec_pretty(&export(db).await?)?;
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    tokio::fs::write(&temp, &json).await?;
    tokio::fs::rename(&temp, path).await?;
    Ok(BackupWritten {
        bytes: json.len() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::away::{AwayReason, NewAway};
    use crate::db::recordings::NewRecording;
    use crate::test_support::{activity, project};
    use serde_json::{Value, json};

    async fn fixture() -> (Db, Value, i64, String) {
        let db = Db::open_memory().await.unwrap();
        clients::insert(&db, "Acme").await.unwrap();
        let p = project(&db, "Website").await;
        let act = activity(&db, 1_000, Some(2_000), "Code", "main.rs").await;
        away::insert(
            &db,
            NewAway {
                start: 3_000,
                reason: AwayReason::Idle,
            },
        )
        .await
        .unwrap();
        let tag = tags::list(&db).await.unwrap().remove(0);
        let rec = recordings::insert(
            &db,
            NewRecording {
                id: "rec-1".into(),
                project_id: p.id,
                start: 1_000,
                end: Some(2_000),
                note: "work".into(),
                billable: true,
                tag_ids: vec![tag.id.clone()],
            },
        )
        .await
        .unwrap();
        let value = serde_json::to_value(export(&db).await.unwrap()).unwrap();
        assert_eq!(rec.id, "rec-1");
        (db, value, act.id, tag.id)
    }

    #[tokio::test]
    async fn export_has_exactly_the_documented_keys_and_counts() {
        let (_db, backup, _, _) = fixture().await;
        let object = backup.as_object().unwrap();

        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "activities",
                "awaySpans",
                "clients",
                "exportedAt",
                "projects",
                "recordings",
                "rules",
                "savedReports",
                "tags",
                "version",
            ]
        );
        assert_eq!(backup["version"], 1);
        assert!(backup["exportedAt"].as_i64().unwrap() > 0);
        assert_eq!(backup["rules"], json!([]));
        assert_eq!(backup["savedReports"], json!([]));
        for (key, len) in [
            ("clients", 1),
            ("projects", 1),
            ("tags", tags::DEFAULT_TAGS.len()),
            ("activities", 1),
            ("awaySpans", 1),
            ("recordings", 1),
        ] {
            assert_eq!(backup[key].as_array().unwrap().len(), len, "{key}");
        }
    }

    #[tokio::test]
    async fn export_serializes_rows_with_camel_case_fields() {
        let (_db, backup, activity_id, tag_id) = fixture().await;

        assert_eq!(
            backup["activities"][0],
            json!({
                "id": activity_id,
                "source": "tracker",
                "start": 1_000,
                "end": 2_000,
                "app": "Code",
                "bundleId": null,
                "title": "main.rs",
                "url": null,
                "path": null,
                "private": false,
            })
        );
        assert_eq!(
            backup["awaySpans"][0],
            json!({ "id": 1, "start": 3_000, "end": null, "reason": "idle" })
        );
        assert_eq!(backup["recordings"][0]["tagIds"], json!([tag_id]));
        assert_eq!(backup["recordings"][0]["id"], "rec-1");
    }

    #[tokio::test]
    async fn export_of_an_empty_store_has_only_the_default_tags() {
        let db = Db::open_memory().await.unwrap();
        let backup = export(&db).await.unwrap();
        assert_eq!(backup.version, BACKUP_VERSION);
        assert_eq!(backup.tags.len(), tags::DEFAULT_TAGS.len());
        assert!(backup.clients.is_empty());
        assert!(backup.projects.is_empty());
        assert!(backup.activities.is_empty());
        assert!(backup.away_spans.is_empty());
        assert!(backup.recordings.is_empty());
    }

    #[tokio::test]
    async fn write_to_writes_json_and_leaves_no_temp_file() {
        let (dir, db) = crate::test_support::temp_db().await;
        let target = dir.path().join("backup.json");

        let written = write_to(&db, &target).await.unwrap();

        let bytes = std::fs::read(&target).unwrap();
        assert_eq!(written.bytes, bytes.len() as u64);
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["version"], 1);
        assert!(!dir.path().join("backup.json.tmp").exists());
    }

    #[tokio::test]
    async fn write_to_rejects_a_relative_path() {
        let db = Db::open_memory().await.unwrap();
        let err = write_to(&db, Path::new("backup.json")).await.unwrap_err();
        assert_eq!(err.kind(), crate::error::ErrorKind::InvalidInput);
    }
}
