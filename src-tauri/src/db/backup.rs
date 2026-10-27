//! The JSON backup: every row of the data store, read in one transaction so it is consistent.

use std::path::Path;

use serde::Serialize;
use ts_rs::TS;

use super::activities::ActivityRow;
use super::away::AwayRow;
use super::clients::Client;
use super::projects::Project;
use super::recordings::Recording;
use super::stored_suggestions::{self, StoredSuggestion};
use super::tags::Tag;
use super::time::{Ms, now_ms};
use super::{Db, activities, away, clients, projects, recordings, tags};
use crate::error::{AppError, AppResult};
use crate::reports::saved::{self, SavedReport};
use crate::rules::{self, types::Rule};

/// 2: recordings and rules carry `tagId` instead of `tagIds` (ADR 0019). 3: recordings carry `spans`
/// instead of `start` and `end` (ADR 0020). 4: `storedSuggestions` (ADR 0028).
pub const BACKUP_VERSION: u32 = 4;

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
    pub stored_suggestions: Vec<StoredSuggestion>,
    pub rules: Vec<Rule>,
    pub saved_reports: Vec<SavedReport>,
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
        stored_suggestions: stored_suggestions::all(&mut tx).await?,
        rules: rules::store::all(&mut tx).await?,
        saved_reports: saved::all(&mut tx).await?,
    };
    tx.commit().await?;
    Ok(backup)
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct BackupWritten {
    pub bytes: u64,
    pub recordings: usize,
    pub activities: usize,
}

/// Writes the backup as pretty JSON to an absolute path, through a temp file beside it.
pub async fn write_to(db: &Db, path: &Path) -> AppResult<BackupWritten> {
    if !path.is_absolute() {
        return Err(AppError::InvalidInput(format!(
            "Not an absolute path: {}",
            path.display()
        )));
    }
    let backup = export(db).await?;
    Ok(BackupWritten {
        bytes: write_json(&backup, path).await?,
        recordings: backup.recordings.len(),
        activities: backup.activities.len(),
    })
}

/// Pretty JSON through a temp file beside `path`; returns the bytes written.
pub(super) async fn write_json(value: &impl Serialize, path: &Path) -> AppResult<u64> {
    let json = serde_json::to_vec_pretty(value)?;
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    tokio::fs::write(&temp, &json).await?;
    tokio::fs::rename(&temp, path).await?;
    Ok(json.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::away::{AwayReason, NewAway};
    use crate::db::recordings::NewRecording;
    use crate::test_support::{activity, client, project};
    use serde_json::{Value, json};

    async fn fixture() -> (Db, Value, i64, String) {
        let db = Db::open_memory().await.unwrap();
        client(&db, "Acme").await;
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
                spans: vec![(1_000, Some(2_000)), (3_000, Some(4_000))],
                note: "work".into(),
                tag_id: Some(tag.id.clone()),
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
                "storedSuggestions",
                "tags",
                "version",
            ]
        );
        assert_eq!(backup["version"], 4);
        assert!(backup["exportedAt"].as_i64().unwrap() > 0);
        assert_eq!(backup["rules"], json!([]));
        assert_eq!(backup["savedReports"], json!([]));
        assert_eq!(backup["storedSuggestions"], json!([]));
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
        assert_eq!(backup["recordings"][0]["tagId"], json!(tag_id));
        assert_eq!(backup["recordings"][0].get("tagIds"), None);
        assert_eq!(backup["recordings"][0]["id"], "rec-1");
    }

    #[tokio::test]
    async fn export_carries_the_spans_of_a_recording_instead_of_start_and_end() {
        let (db, backup, _, _) = fixture().await;
        let stored = recordings::get(&db, "rec-1").await.unwrap();

        let recording = &backup["recordings"][0];

        assert_eq!(
            recording["spans"],
            json!([
                { "id": stored.spans[0].id, "start": 1_000, "end": 2_000 },
                { "id": stored.spans[1].id, "start": 3_000, "end": 4_000 },
            ])
        );
        assert_eq!(recording.get("start"), None);
        assert_eq!(recording.get("end"), None);
    }

    #[tokio::test]
    async fn export_has_a_null_end_for_the_running_recordings_open_span() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "Website").await;
        let timer = crate::test_support::recording(&db, &p.id, 5_000, None).await;

        let backup = serde_json::to_value(export(&db).await.unwrap()).unwrap();

        assert_eq!(
            backup["recordings"][0]["spans"],
            json!([{ "id": timer.spans[0].id, "start": 5_000, "end": null }])
        );
    }

    #[tokio::test]
    async fn export_has_a_null_tag_id_for_untagged_recordings() {
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "Website").await;
        crate::test_support::recording(&db, &p.id, 1_000, Some(2_000)).await;

        let backup = serde_json::to_value(export(&db).await.unwrap()).unwrap();

        assert_eq!(backup["recordings"][0]["tagId"], Value::Null);
    }

    #[tokio::test]
    async fn export_carries_a_rules_tag_as_tag_id() {
        use crate::rules::types::{ConditionInput, RuleField, RuleInput, RuleOp};
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "Website").await;
        let tag = tags::list(&db).await.unwrap().remove(0);
        let rule = rules::store::save(
            &db,
            None,
            &RuleInput {
                name: "Slack".into(),
                project_id: p.id,
                enabled: true,
                note: String::new(),
                tag_id: Some(tag.id.clone()),
                conditions: vec![ConditionInput {
                    field: RuleField::App,
                    op: RuleOp::Is,
                    value: "Slack".into(),
                }],
            },
        )
        .await
        .unwrap();

        let backup = serde_json::to_value(export(&db).await.unwrap()).unwrap();

        assert_eq!(backup["rules"][0]["id"], json!(rule.id));
        assert_eq!(backup["rules"][0]["tagId"], json!(tag.id));
        assert_eq!(backup["rules"][0].get("tagIds"), None);
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
        assert_eq!(json["version"], 4);
        assert!(!dir.path().join("backup.json.tmp").exists());
    }

    #[tokio::test]
    async fn export_contains_the_rules_in_order_with_their_conditions() {
        use crate::rules::testing::save_rule;
        use crate::rules::types::{RuleField, RuleOp};
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "Website").await;
        let first = save_rule(
            &db,
            "Slack",
            &p.id,
            &[
                (RuleField::App, RuleOp::Is, "Slack"),
                (RuleField::Title, RuleOp::Contains, "standup"),
            ],
        )
        .await;
        let second = save_rule(
            &db,
            "Docs",
            &p.id,
            &[(RuleField::Domain, RuleOp::Is, "docs.rs")],
        )
        .await;

        let backup = serde_json::to_value(export(&db).await.unwrap()).unwrap();

        assert_eq!(
            backup["rules"],
            json!([
                {
                    "id": first.id,
                    "name": "Slack",
                    "projectId": p.id,
                    "position": 0,
                    "enabled": true,
                    "note": "",
                    "tagId": null,
                    "conditions": [
                        { "id": first.conditions[0].id, "field": "app", "op": "is", "value": "Slack" },
                        { "id": first.conditions[1].id, "field": "title", "op": "contains", "value": "standup" },
                    ],
                },
                {
                    "id": second.id,
                    "name": "Docs",
                    "projectId": p.id,
                    "position": 1,
                    "enabled": true,
                    "note": "",
                    "tagId": null,
                    "conditions": [
                        { "id": second.conditions[0].id, "field": "domain", "op": "is", "value": "docs.rs" },
                    ],
                },
            ])
        );
    }

    #[tokio::test]
    async fn export_contains_the_stored_suggestions_with_their_spans() {
        use crate::db::recordings::Span;
        use crate::db::time::Day;
        let db = Db::open_memory().await.unwrap();
        let p = project(&db, "Website").await;
        let stored = StoredSuggestion {
            id: "s1".into(),
            project_id: Some(p.id.clone()),
            note: "review".into(),
            tag_id: None,
            source: "claude".into(),
            day: Day::parse("2026-01-05").unwrap(),
            spans: vec![Span {
                start: 1_000,
                end: 2_000,
            }],
            created_at: 10,
            updated_at: 20,
        };
        let mut conn = db.pool().acquire().await.unwrap();
        stored_suggestions::put_on(&mut conn, &stored)
            .await
            .unwrap();
        drop(conn);

        let backup = serde_json::to_value(export(&db).await.unwrap()).unwrap();

        assert_eq!(
            backup["storedSuggestions"],
            json!([{
                "id": "s1",
                "projectId": p.id,
                "note": "review",
                "tagId": null,
                "source": "claude",
                "day": "2026-01-05",
                "spans": [{ "start": 1_000, "end": 2_000 }],
                "createdAt": 10,
                "updatedAt": 20,
            }])
        );
    }

    #[tokio::test]
    async fn export_contains_the_saved_reports_in_order() {
        use crate::reports::{Range, ReportFilter};
        let db = Db::open_memory().await.unwrap();
        let weekly = ReportFilter {
            range: Range::LastWeek,
            ..ReportFilter::default()
        };
        let first = saved::save(&db, "Weekly", &weekly).await.unwrap();
        let second = saved::save(&db, "Today", &ReportFilter::default())
            .await
            .unwrap();

        let backup = serde_json::to_value(export(&db).await.unwrap()).unwrap();

        assert_eq!(backup["savedReports"].as_array().unwrap().len(), 2);
        assert_eq!(backup["savedReports"][0]["id"], first.id);
        assert_eq!(backup["savedReports"][0]["name"], "Weekly");
        assert_eq!(
            backup["savedReports"][0]["filter"]["range"],
            json!({ "kind": "lastWeek" })
        );
        assert_eq!(backup["savedReports"][1]["id"], second.id);
        assert_eq!(backup["savedReports"][1]["position"], 1);
    }

    #[tokio::test]
    async fn write_to_reports_the_recording_and_activity_counts() {
        let (dir, db) = crate::test_support::temp_db().await;
        let p = project(&db, "Website").await;
        for i in 0..3 {
            crate::test_support::recording(&db, &p.id, i * 10_000, Some(i * 10_000 + 1_000)).await;
        }
        for i in 0..2 {
            activity(&db, i * 10_000, Some(i * 10_000 + 1_000), "Code", "main.rs").await;
        }

        let written = write_to(&db, &dir.path().join("backup.json"))
            .await
            .unwrap();

        assert_eq!((written.recordings, written.activities), (3, 2));
    }

    #[tokio::test]
    async fn write_to_rejects_a_relative_path() {
        let db = Db::open_memory().await.unwrap();
        let err = write_to(&db, Path::new("backup.json")).await.unwrap_err();
        assert_eq!(err.kind(), crate::error::ErrorKind::InvalidInput);
    }
}
