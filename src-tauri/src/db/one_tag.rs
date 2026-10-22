//! Migration `0006_one_tag.sql` keeps one tag per recording and rule (ADR 0019). Before it runs on data
//! where a row has several, the data is written as a version 1 backup beside the database file.

use std::path::{Path, PathBuf};

use serde::Serialize;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, Row, SqliteConnection, SqlitePool};

use super::activities::ActivityRow;
use super::away::AwayRow;
use super::clients::Client;
use super::tags::Tag;
use super::time::{Day, Ms, now_ms};
use super::{activities, away, backup, clients, tags};
use crate::error::AppResult;
use crate::reports::saved::{self, SavedReport};

const ONE_TAG_MIGRATION: i64 = 6;
const LEGACY_BACKUP_VERSION: u32 = 1;

/// What migration 0006 dropped and where the backup of the data before it is.
#[derive(Debug, Clone, PartialEq)]
pub struct TagsDropped {
    pub recordings: i64,
    pub rules: i64,
    pub backup: PathBuf,
}

impl TagsDropped {
    /// The launch toast.
    pub fn message(&self) -> String {
        let rows = match (self.recordings, self.rules) {
            (n, 0) => plural(n, "recording"),
            (0, n) => plural(n, "rule"),
            (r, n) => format!("{} and {}", plural(r, "recording"), plural(n, "rule")),
        };
        format!(
            "One tag per recording: {rows} lost tags (each kept its first by name). Backup: {}",
            self.backup.display()
        )
    }
}

fn plural(n: i64, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// Runs the migrations before 0006; when 0006 is still to run and a recording or rule has more than one
/// tag, writes `pre-0006-<today>.json` into `dir`. A backup that fails stops the migration.
pub(super) async fn back_up(
    migrator: &Migrator,
    pool: &SqlitePool,
    dir: &Path,
) -> AppResult<Option<TagsDropped>> {
    migrator.run_to(ONE_TAG_MIGRATION - 1, pool).await?;
    let mut tx = pool.begin().await?;
    let Some((recordings, rules)) = dropped_counts(&mut tx).await? else {
        return Ok(None);
    };
    let legacy = export(&mut tx).await?;
    tx.commit().await?;
    let path = dir.join(format!("pre-0006-{}.json", Day::today()));
    backup::write_json(&legacy, &path).await?;
    Ok(Some(TagsDropped {
        recordings,
        rules,
        backup: path,
    }))
}

/// The recordings and rules with more than one existing tag, while 0006 is still to run and some do.
async fn dropped_counts(conn: &mut SqliteConnection) -> AppResult<Option<(i64, i64)>> {
    let pending: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'recording_tags')",
    )
    .fetch_one(&mut *conn)
    .await?;
    if !pending {
        return Ok(None);
    }
    let recordings: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM (SELECT rt.recording_id FROM recording_tags rt JOIN tags t ON t.id = rt.tag_id
         GROUP BY rt.recording_id HAVING count(*) > 1)",
    )
    .fetch_one(&mut *conn)
    .await?;
    let rules: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM rules
         WHERE (SELECT count(*) FROM json_each(rules.tag_ids) j JOIN tags t ON t.id = j.value) > 1",
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok((recordings > 0 || rules > 0).then_some((recordings, rules)))
}

/// `db::backup::Backup` as it was before 0006: recordings and rules with `tagIds`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LegacyBackup {
    version: u32,
    exported_at: Ms,
    clients: Vec<Client>,
    projects: Vec<LegacyProject>,
    tags: Vec<Tag>,
    activities: Vec<ActivityRow>,
    away_spans: Vec<AwayRow>,
    recordings: Vec<LegacyRecording>,
    rules: Vec<LegacyRule>,
    saved_reports: Vec<SavedReport>,
}

/// A project before migration 0008 (no `billable`).
#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct LegacyProject {
    id: String,
    client_id: Option<String>,
    name: String,
    color: String,
    shade: u8,
    archived_at: Option<Ms>,
    created_at: Ms,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LegacyRecording {
    id: String,
    project_id: String,
    start: Ms,
    end: Option<Ms>,
    note: String,
    tag_ids: Vec<String>,
    created_at: Ms,
    updated_at: Ms,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LegacyRule {
    id: String,
    name: String,
    project_id: String,
    position: i64,
    enabled: bool,
    note: String,
    tag_ids: Vec<String>,
    conditions: Vec<LegacyCondition>,
}

#[derive(Serialize, sqlx::FromRow)]
struct LegacyCondition {
    #[serde(skip)]
    rule_id: String,
    id: String,
    field: String,
    op: String,
    value: String,
}

fn json_ids(row: &SqliteRow, column: &str) -> sqlx::Result<Vec<String>> {
    let text: String = row.try_get(column)?;
    serde_json::from_str(&text).map_err(|e| sqlx::Error::ColumnDecode {
        index: column.into(),
        source: Box::new(e),
    })
}

impl FromRow<'_, SqliteRow> for LegacyRecording {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            start: row.try_get("start")?,
            end: row.try_get("end")?,
            note: row.try_get("note")?,
            tag_ids: json_ids(row, "tag_ids")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl FromRow<'_, SqliteRow> for LegacyRule {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            project_id: row.try_get("project_id")?,
            position: row.try_get("position")?,
            enabled: row.try_get("enabled")?,
            note: row.try_get("note")?,
            tag_ids: json_ids(row, "tag_ids")?,
            conditions: Vec::new(),
        })
    }
}

async fn export(conn: &mut SqliteConnection) -> AppResult<LegacyBackup> {
    let recordings = sqlx::query_as(
        r#"SELECT r.*, (SELECT json_group_array(tag_id) FROM (
             SELECT rt.tag_id FROM recording_tags rt JOIN tags t ON t.id = rt.tag_id
             WHERE rt.recording_id = r.id ORDER BY t.name COLLATE NOCASE)) AS tag_ids
           FROM recordings r ORDER BY r.start, r.id"#,
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut rules: Vec<LegacyRule> = sqlx::query_as("SELECT * FROM rules ORDER BY position, id")
        .fetch_all(&mut *conn)
        .await?;
    let conditions: Vec<LegacyCondition> =
        sqlx::query_as("SELECT * FROM rule_conditions ORDER BY rule_id, position")
            .fetch_all(&mut *conn)
            .await?;
    for condition in conditions {
        if let Some(rule) = rules.iter_mut().find(|r| r.id == condition.rule_id) {
            rule.conditions.push(condition);
        }
    }
    Ok(LegacyBackup {
        version: LEGACY_BACKUP_VERSION,
        exported_at: now_ms(),
        clients: clients::all(conn).await?,
        projects: sqlx::query_as(
            "SELECT id, client_id, name, color, shade, archived_at, created_at
             FROM projects ORDER BY created_at, id",
        )
        .fetch_all(&mut *conn)
        .await?,
        tags: tags::all(conn).await?,
        activities: activities::all(conn).await?,
        away_spans: away::all(conn).await?,
        recordings,
        rules,
        saved_reports: saved::all(conn).await?,
    })
}

/// A database file as it was before migration 0006, for tests of this migration and of its notice.
#[cfg(test)]
pub(crate) mod legacy {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    use super::*;

    /// Project `p` and the tags `Beta` (`t-beta`), `alpha` (`t-alpha`) and `Charlie` (`t-charlie`).
    const BASE: &str =
        "INSERT INTO projects (id, name, color, created_at) VALUES ('p', 'P', 'blue', 1);
        INSERT INTO tags (id, name, created_at) VALUES
          ('t-beta', 'Beta', 1), ('t-alpha', 'alpha', 2), ('t-charlie', 'Charlie', 3);";

    /// One recording with three tags and nothing else.
    pub(crate) const RECORDING_WITH_THREE_TAGS: &str =
        "INSERT INTO recordings (id, project_id, start, \"end\", note, created_at, updated_at)
           VALUES ('r3', 'p', 20, 30, '', 1, 1);
         INSERT INTO recording_tags (recording_id, tag_id) VALUES
           ('r3', 't-beta'), ('r3', 't-charlie'), ('r3', 't-alpha');";

    /// Applies migrations 0001 to 0005 to a new file at `file`, then `BASE` and `seed`.
    pub(crate) async fn create(file: &Path, seed: &'static str) {
        let options = SqliteConnectOptions::new()
            .filename(file)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::migrate!("./migrations")
            .run_to(ONE_TAG_MIGRATION - 1, &pool)
            .await
            .unwrap();
        sqlx::raw_sql(BASE).execute(&pool).await.unwrap();
        sqlx::raw_sql(seed).execute(&pool).await.unwrap();
        pool.close().await;
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::legacy::{self, RECORDING_WITH_THREE_TAGS};
    use super::*;
    use crate::db::{DB_FILE, DataStore, Db};
    use crate::test_support::{project, recording, temp_config_dir};

    const SEVERAL_TAGS: &str = "
        INSERT INTO recordings (id, project_id, start, \"end\", note, created_at, updated_at) VALUES
          ('r0', 'p', 0, 10, '', 1, 1), ('r1', 'p', 10, 20, '', 1, 1), ('r3', 'p', 20, 30, '', 1, 1);
        INSERT INTO recording_tags (recording_id, tag_id) VALUES
          ('r1', 't-beta'), ('r3', 't-beta'), ('r3', 't-charlie'), ('r3', 't-alpha');
        INSERT INTO rules (id, name, project_id, position, tag_ids) VALUES
          ('u1', 'One', 'p', 0, '[\"t-charlie\", \"t-beta\"]'),
          ('u2', 'Two', 'p', 1, '[]'),
          ('u3', 'Three', 'p', 2, '[\"gone\", \"t-alpha\"]');";

    const ONE_TAG_EACH: &str = "
        INSERT INTO recordings (id, project_id, start, \"end\", note, created_at, updated_at) VALUES
          ('r1', 'p', 10, 20, '', 1, 1);
        INSERT INTO recording_tags (recording_id, tag_id) VALUES ('r1', 't-beta');
        INSERT INTO rules (id, name, project_id, position, tag_ids) VALUES
          ('u1', 'One', 'p', 0, '[\"t-beta\"]');";

    fn backup_path(dir: &Path) -> PathBuf {
        dir.join(format!("pre-0006-{}.json", Day::today()))
    }

    async fn tags_of(db: &Db, table: &'static str) -> Vec<(String, Option<String>)> {
        let sql = match table {
            "recordings" => "SELECT id, tag_id FROM recordings ORDER BY id",
            _ => "SELECT id, tag_id FROM rules ORDER BY id",
        };
        sqlx::query_as(sql).fetch_all(db.pool()).await.unwrap()
    }

    fn some(id: &str) -> Option<String> {
        Some(id.to_string())
    }

    #[tokio::test]
    async fn a_recording_keeps_its_first_tag_by_name_ignoring_case() {
        let tmp = temp_config_dir();
        let file = tmp.path().join(DB_FILE);
        legacy::create(&file, SEVERAL_TAGS).await;

        let (db, _) = Db::open_migrating(&file).await.unwrap();

        assert_eq!(
            tags_of(&db, "recordings").await,
            [
                ("r0".to_string(), None),
                ("r1".to_string(), some("t-beta")),
                ("r3".to_string(), some("t-alpha")),
            ]
        );
    }

    #[tokio::test]
    async fn a_rule_keeps_the_first_existing_tag_of_its_json_by_name() {
        let tmp = temp_config_dir();
        let file = tmp.path().join(DB_FILE);
        legacy::create(&file, SEVERAL_TAGS).await;

        let (db, _) = Db::open_migrating(&file).await.unwrap();

        assert_eq!(
            tags_of(&db, "rules").await,
            [
                ("u1".to_string(), some("t-beta")),
                ("u2".to_string(), None),
                ("u3".to_string(), some("t-alpha")),
            ]
        );
    }

    #[tokio::test]
    async fn dropping_tags_writes_a_version_1_backup_with_tag_ids_and_reports_it() {
        let tmp = temp_config_dir();
        let file = tmp.path().join(DB_FILE);
        legacy::create(&file, SEVERAL_TAGS).await;

        let (_db, dropped) = Db::open_migrating(&file).await.unwrap();

        let path = backup_path(tmp.path());
        assert_eq!(
            dropped,
            Some(TagsDropped {
                recordings: 1,
                rules: 1,
                backup: path.clone(),
            })
        );
        let backup: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(backup["version"], 1);
        assert_eq!(
            backup["recordings"],
            json!([
                { "id": "r0", "projectId": "p", "start": 0, "end": 10, "note": "",
                  "tagIds": [], "createdAt": 1, "updatedAt": 1 },
                { "id": "r1", "projectId": "p", "start": 10, "end": 20, "note": "",
                  "tagIds": ["t-beta"], "createdAt": 1, "updatedAt": 1 },
                { "id": "r3", "projectId": "p", "start": 20, "end": 30, "note": "",
                  "tagIds": ["t-alpha", "t-beta", "t-charlie"], "createdAt": 1, "updatedAt": 1 },
            ])
        );
        let rule_tags: Vec<&Value> = backup["rules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| &r["tagIds"])
            .collect();
        assert_eq!(
            rule_tags,
            [
                &json!(["t-charlie", "t-beta"]),
                &json!([]),
                &json!(["gone", "t-alpha"]),
            ]
        );
    }

    #[tokio::test]
    async fn a_database_where_every_row_has_at_most_one_tag_gets_no_backup() {
        let tmp = temp_config_dir();
        let file = tmp.path().join(DB_FILE);
        legacy::create(&file, ONE_TAG_EACH).await;

        let (db, dropped) = Db::open_migrating(&file).await.unwrap();

        assert_eq!(dropped, None);
        assert!(!backup_path(tmp.path()).exists());
        assert_eq!(
            tags_of(&db, "recordings").await,
            [("r1".to_string(), some("t-beta"))]
        );
        assert_eq!(
            tags_of(&db, "rules").await,
            [("u1".to_string(), some("t-beta"))]
        );
    }

    #[tokio::test]
    async fn opening_a_migrated_database_again_does_nothing() {
        let tmp = temp_config_dir();
        let file = tmp.path().join(DB_FILE);
        legacy::create(&file, SEVERAL_TAGS).await;
        let (db, dropped) = Db::open_migrating(&file).await.unwrap();
        assert!(dropped.is_some());
        db.close().await;
        std::fs::remove_file(backup_path(tmp.path())).unwrap();

        let (db, dropped) = Db::open_migrating(&file).await.unwrap();

        assert_eq!(dropped, None);
        assert!(!backup_path(tmp.path()).exists());
        assert_eq!(
            tags_of(&db, "recordings").await[2],
            ("r3".to_string(), some("t-alpha"))
        );
    }

    #[tokio::test]
    async fn a_fresh_database_gets_no_backup_and_untagged_recordings() {
        let tmp = temp_config_dir();
        let file = tmp.path().join(DB_FILE);

        let (db, dropped) = Db::open_migrating(&file).await.unwrap();

        assert_eq!(dropped, None);
        let files: Vec<String> = std::fs::read_dir(tmp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with("pre-0006"))
            .collect();
        assert!(files.is_empty(), "{files:?}");
        let p = project(&db, "P").await;
        assert_eq!(recording(&db, &p.id, 100, Some(200)).await.tag_id, None);
    }

    #[test]
    fn the_notice_names_what_was_dropped_with_plurals() {
        let backup = PathBuf::from("/data/pre-0006-2026-10-21.json");
        for ((recordings, rules), rows) in [
            ((1, 0), "1 recording"),
            ((2, 0), "2 recordings"),
            ((0, 1), "1 rule"),
            ((0, 3), "3 rules"),
            ((1, 1), "1 recording and 1 rule"),
            ((1, 2), "1 recording and 2 rules"),
            ((4, 1), "4 recordings and 1 rule"),
        ] {
            let dropped = TagsDropped {
                recordings,
                rules,
                backup: backup.clone(),
            };
            assert_eq!(
                dropped.message(),
                format!(
                    "One tag per recording: {rows} lost tags (each kept its first by name). \
                     Backup: /data/pre-0006-2026-10-21.json"
                ),
                "{recordings} recordings, {rules} rules"
            );
        }
    }

    #[tokio::test]
    async fn the_data_store_keeps_the_notice_of_a_migration_that_dropped_tags() {
        let tmp = temp_config_dir();
        let file = tmp.path().join(DB_FILE);
        legacy::create(&file, RECORDING_WITH_THREE_TAGS).await;

        let store = DataStore::open(file).await;

        let expected = TagsDropped {
            recordings: 1,
            rules: 0,
            backup: backup_path(tmp.path()),
        }
        .message();
        assert_eq!(store.migration_notice(), Some(expected.as_str()));
    }

    #[tokio::test]
    async fn the_data_store_has_no_notice_when_nothing_was_dropped() {
        let tmp = temp_config_dir();

        let store = DataStore::open(tmp.path().join(DB_FILE)).await;

        assert_eq!(store.migration_notice(), None);
    }
}
