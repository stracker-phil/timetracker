//! The SQLite data store (ADR 0006): one file in the data folder, a small sqlx pool and embedded
//! forward-only migrations. Each table has its repository module.

pub mod activities;
pub mod away;
pub mod backup;
pub mod clients;
pub mod commands;
pub mod one_tag;
pub mod projects;
pub mod recordings;
pub mod relocate;
pub mod tags;
pub mod time;

use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::Duration;

use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};

use crate::error::{AppError, AppResult};
use crate::lock::RwLockExt;
use crate::store::json_file::create_private_dir;
use crate::store::settings::Settings;
use one_tag::TagsDropped;

pub const DB_FILE: &str = "timetracker.sqlite";
/// SQLite has one writer; WAL lets readers run next to it.
const MAX_CONNECTIONS: u32 = 4;
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
/// A reader beside the running app (`timetracker mcp`) waits less for its write lock.
const READ_ONLY_BUSY_TIMEOUT: Duration = Duration::from_secs(2);
const READ_ONLY_CONNECTIONS: u32 = 2;

/// A cheap handle to the pool; clones share it.
#[derive(Clone, Debug)]
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    /// Creates the file and its private parent folder when missing, then migrates.
    pub async fn open(path: &Path) -> AppResult<Db> {
        Ok(Db::open_migrating(path).await?.0)
    }

    /// `open`, also saying which tags migration 0006 dropped (`db::one_tag`), after backing up the
    /// data beside the file.
    pub async fn open_migrating(path: &Path) -> AppResult<(Db, Option<TagsDropped>)> {
        let dir = path.parent();
        if let Some(dir) = dir {
            create_private_dir(dir)?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(BUSY_TIMEOUT)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(MAX_CONNECTIONS)
            .connect_with(options)
            .await?;
        Db::init(pool, dir).await
    }

    /// An existing file for reading only, beside the app that writes it: the journal mode stays as
    /// the app set it (WAL readers never block the writer), nothing is created, migrated or seeded. A
    /// file older than this build's migrations is refused; a newer one is read as it is.
    pub async fn open_read_only(path: &Path) -> AppResult<Db> {
        if !path.is_file() {
            return Err(AppError::NotFound(format!(
                "TimeTracker has no data yet ({} not found)",
                path.display()
            )));
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .read_only(true)
            .create_if_missing(false)
            .busy_timeout(READ_ONLY_BUSY_TIMEOUT);
        let pool = SqlitePoolOptions::new()
            .max_connections(READ_ONLY_CONNECTIONS)
            .connect_with(options)
            .await?;
        let db = Db { pool };
        db.check_schema().await?;
        Ok(db)
    }

    async fn check_schema(&self) -> AppResult<()> {
        let latest = sqlx::migrate!("./migrations")
            .iter()
            .map(|m| m.version)
            .max()
            .unwrap_or(0);
        let applied: Option<i64> = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(self.pool())
            .await
            .unwrap_or(None);
        if applied.unwrap_or(0) < latest {
            return Err(AppError::InvalidInput(
                "Open TimeTracker once to update its data".into(),
            ));
        }
        Ok(())
    }

    /// One connection that is never closed, since an in-memory database lives only as long as it.
    #[cfg(any(test, feature = "test-util"))]
    pub async fn open_memory() -> AppResult<Db> {
        let options = SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(options)
            .await?;
        Ok(Db::init(pool, None).await?.0)
    }

    /// Migrates; with `backup_dir`, backs up the data there first when a migration drops some.
    async fn init(
        pool: SqlitePool,
        backup_dir: Option<&Path>,
    ) -> AppResult<(Db, Option<TagsDropped>)> {
        let migrator = sqlx::migrate!("./migrations");
        let dropped = match backup_dir {
            Some(dir) => one_tag::back_up(&migrator, &pool, dir).await?,
            None => None,
        };
        migrator.run(&pool).await?;
        let db = Db { pool };
        tags::seed_defaults(&db).await?;
        Ok((db, dropped))
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}

/// `dataDir` from the settings when it is an absolute path, else `<config dir>/data`.
pub fn data_dir(settings: &Settings, config_dir: &Path) -> PathBuf {
    match settings.data_dir.as_deref().map(Path::new) {
        Some(dir) if dir.is_absolute() => dir.to_path_buf(),
        _ => default_data_dir(config_dir),
    }
}

pub fn default_data_dir(config_dir: &Path) -> PathBuf {
    config_dir.join("data")
}

/// The database file and the open database, or why it could not be opened. The app keeps running
/// without it; commands that need it fail with `db`. Both change when the data folder moves.
pub struct DataStore {
    file: RwLock<PathBuf>,
    db: RwLock<Result<Db, String>>,
    /// Held for a whole move, so two moves never overlap.
    moving: tokio::sync::Mutex<()>,
    /// What opening dropped from the data, for the launch toast.
    migration_notice: Option<String>,
}

impl DataStore {
    pub async fn open(file: PathBuf) -> DataStore {
        let (db, migration_notice) = match Db::open_migrating(&file).await {
            Ok((db, dropped)) => {
                let notice = dropped.map(|d| d.message());
                if let Some(notice) = &notice {
                    log::warn!("{notice}");
                }
                (Ok(db), notice)
            }
            Err(e) => {
                log::error!("Cannot open the data store at {}: {e}", file.display());
                (Err(e.to_string()), None)
            }
        };
        DataStore {
            file: RwLock::new(file),
            db: RwLock::new(db),
            moving: tokio::sync::Mutex::new(()),
            migration_notice,
        }
    }

    pub fn migration_notice(&self) -> Option<&str> {
        self.migration_notice.as_deref()
    }

    pub fn file(&self) -> PathBuf {
        self.file.read_unpoisoned().clone()
    }

    pub fn db(&self) -> AppResult<Db> {
        self.db.read_unpoisoned().clone().map_err(|_| AppError::Db {
            message: "The data store is not available".into(),
            code: None,
        })
    }

    /// Makes the store unavailable for `reason` and returns the database it had open.
    fn take(&self, reason: &str) -> AppResult<Db> {
        let db = self.db()?;
        *self.db.write_unpoisoned() = Err(reason.to_string());
        Ok(db)
    }

    fn put(&self, file: PathBuf, db: Result<Db, String>) {
        *self.file.write_unpoisoned() = file;
        *self.db.write_unpoisoned() = db;
    }

    pub async fn close(&self) {
        let db = self.db.read_unpoisoned().clone();
        if let Ok(db) = db {
            db.close().await;
        }
    }

    /// Why the database could not be opened.
    pub fn error(&self) -> Option<String> {
        self.db.read_unpoisoned().as_ref().err().cloned()
    }
}

pub const MAX_NAME_CHARS: usize = 100;

/// `None` for a blank color, else the accent it names; anything else is an error on `color`.
fn chosen_accent(color: &str) -> AppResult<Option<String>> {
    match color.trim() {
        "" => Ok(None),
        color if crate::store::settings::is_accent(color) => Ok(Some(color.to_string())),
        _ => Err(invalid_field("color", "must be one of the accent colors")),
    }
}

/// `field` is the input's key as the page sends it (`clientId`).
fn invalid_field(field: &str, message: impl Into<String>) -> AppError {
    AppError::InvalidField {
        field: field.into(),
        message: message.into(),
    }
}

/// The trimmed name of a client, project or tag.
pub(crate) fn required_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(invalid_field("name", "must not be empty"));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(invalid_field(
            "name",
            format!("must be at most {MAX_NAME_CHARS} characters"),
        ));
    }
    Ok(name.to_string())
}

/// Names compare ignoring case, beyond ASCII (SQLite's `NOCASE` folds ASCII only).
fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Whether one of the `(id, name)` rows other than `except` has `name`.
fn name_in_use<'a>(
    rows: impl IntoIterator<Item = (&'a str, &'a str)>,
    name: &str,
    except: Option<&str>,
) -> bool {
    rows.into_iter()
        .any(|(id, row_name)| Some(id) != except && same_name(row_name, name))
}

fn name_taken(what: &str) -> AppError {
    invalid_field("name", format!("{what} with this name already exists"))
}

pub(crate) fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::time::Ms;
    use super::*;
    use crate::error::ErrorKind;
    use crate::test_support::temp_config_dir;

    async fn scalar_text(db: &Db, sql: &'static str) -> String {
        sqlx::query_scalar(sql).fetch_one(db.pool()).await.unwrap()
    }

    async fn tag_names(db: &Db) -> Vec<String> {
        let mut names: Vec<String> = sqlx::query_scalar("SELECT name FROM tags")
            .fetch_all(db.pool())
            .await
            .unwrap();
        names.sort();
        names
    }

    fn sorted_default_tags() -> Vec<String> {
        let mut names: Vec<String> = tags::DEFAULT_TAGS.iter().map(|s| s.to_string()).collect();
        names.sort();
        names
    }

    #[tokio::test]
    async fn open_creates_a_private_folder_the_file_and_wal_side_files() {
        let tmp = temp_config_dir();
        let dir = tmp.path().join("nested").join("data");
        let file = dir.join(DB_FILE);
        let db = Db::open(&file).await.unwrap();

        assert_eq!(
            std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert!(file.is_file());
        for suffix in ["-wal", "-shm"] {
            let mut side = file.as_os_str().to_owned();
            side.push(suffix);
            assert!(std::path::Path::new(&side).exists(), "{suffix}");
        }
        assert_eq!(scalar_text(&db, "PRAGMA journal_mode").await, "wal");
        let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(foreign_keys, 1);
    }

    #[tokio::test]
    async fn open_migrates_the_schema() {
        let db = Db::open_memory().await.unwrap();
        let tables: Vec<String> =
            sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table'")
                .fetch_all(db.pool())
                .await
                .unwrap();
        for table in [
            "meta",
            "clients",
            "projects",
            "tags",
            "activities",
            "away_spans",
            "recordings",
        ] {
            assert!(tables.iter().any(|t| t == table), "{table} in {tables:?}");
        }
        assert!(!tables.iter().any(|t| t == "recording_tags"), "{tables:?}");
        for table in ["recordings", "rules"] {
            let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?1)")
                .bind(table)
                .fetch_all(db.pool())
                .await
                .unwrap();
            assert!(
                columns.iter().any(|c| c == "tag_id"),
                "{table}: {columns:?}"
            );
            assert!(
                !columns.iter().any(|c| c == "tag_ids"),
                "{table}: {columns:?}"
            );
        }
    }

    async fn migrated_from_recordings_with_start_and_end() -> (tempfile::TempDir, Db) {
        let tmp = temp_config_dir();
        let file = tmp.path().join(DB_FILE);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(&file)
                    .create_if_missing(true)
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        let migrator = sqlx::migrate!("./migrations");
        migrator.run_to(6, &pool).await.unwrap();
        sqlx::raw_sql(
            "INSERT INTO projects (id, name, color, created_at) VALUES ('p', 'P', 'blue', 1);
             INSERT INTO recordings (id, project_id, start, \"end\", note, created_at, updated_at) VALUES
               ('done', 'p', 1000, 3500, 'n', 1, 1),
               ('timer', 'p', 9000, NULL, '', 1, 1);",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;
        let db = Db::open(&file).await.unwrap();
        (tmp, db)
    }

    #[tokio::test]
    async fn the_spans_migration_turns_each_recording_into_one_span_under_its_id() {
        let (_tmp, db) = migrated_from_recordings_with_start_and_end().await;

        let spans: Vec<(String, String, i64, Option<i64>)> = sqlx::query_as(
            "SELECT id, recording_id, start, \"end\" FROM recording_spans ORDER BY start",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();

        assert_eq!(
            spans,
            [
                ("done".to_string(), "done".to_string(), 1000, Some(3500)),
                ("timer".to_string(), "timer".to_string(), 9000, None),
            ]
        );
        let done = recordings::get(&db, "done").await.unwrap();
        assert_eq!((done.start(), done.end()), (1000, Some(3500)));
        assert_eq!(done.duration(0), 2500);
        assert_eq!(done.note, "n");
        let timer = recordings::running(&db).await.unwrap().unwrap();
        assert_eq!(timer.id, "timer");
        assert_eq!(timer.open_span().unwrap().start, 9000);
    }

    #[tokio::test]
    async fn the_spans_migration_drops_the_start_and_end_columns_and_their_index() {
        let (_tmp, db) = migrated_from_recordings_with_start_and_end().await;

        let columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info('recordings') ORDER BY cid")
                .fetch_all(db.pool())
                .await
                .unwrap();
        assert_eq!(
            columns,
            [
                "id",
                "project_id",
                "note",
                "created_at",
                "updated_at",
                "tag_id"
            ]
        );
        let indexes: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = 'index' AND name NOT LIKE 'sqlite_%'",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert!(
            !indexes.iter().any(|i| i == "recordings_start"),
            "{indexes:?}"
        );
        for index in ["recording_spans_start", "recording_spans_recording"] {
            assert!(indexes.iter().any(|i| i == index), "{index} in {indexes:?}");
        }
    }

    #[tokio::test]
    async fn the_client_colors_migration_derives_colors_and_shades_from_existing_projects() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .in_memory(true)
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        for sql in [
            include_str!("../../migrations/0001_init.sql"),
            include_str!("../../migrations/0002_drop_billing.sql"),
            "INSERT INTO clients (id, name, created_at) VALUES
               ('a', 'A', 1), ('b', 'B', 2), ('tie', 'Tie', 3);
             INSERT INTO projects (id, client_id, name, color, created_at) VALUES
               ('a-late', 'a', 'Late', 'teal', 30),
               ('a-first', 'a', 'First', 'teal', 10),
               ('a-mid', 'a', 'Mid', 'red', 20),
               ('tie-1', 'tie', 'One', 'teal', 40),
               ('tie-2', 'tie', 'Two', 'red', 50),
               ('loose', NULL, 'Loose', 'purple', 60);",
            include_str!("../../migrations/0003_client_colors.sql"),
        ] {
            sqlx::raw_sql(sql).execute(&pool).await.unwrap();
        }

        let clients: Vec<(String, String)> =
            sqlx::query_as("SELECT id, color FROM clients ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            clients,
            [
                ("a".to_string(), "teal".to_string()),
                ("b".to_string(), "blue".to_string()),
                ("tie".to_string(), "red".to_string()),
            ]
        );

        let projects: Vec<(String, String, i64)> =
            sqlx::query_as("SELECT id, color, shade FROM projects ORDER BY created_at")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            projects,
            [
                ("a-first".to_string(), "teal".to_string(), 4),
                ("a-mid".to_string(), "teal".to_string(), 1),
                ("a-late".to_string(), "teal".to_string(), 7),
                ("tie-1".to_string(), "red".to_string(), 4),
                ("tie-2".to_string(), "red".to_string(), 1),
                ("loose".to_string(), "purple".to_string(), 4),
            ]
        );
    }

    #[tokio::test]
    async fn meta_records_the_schema_version_and_creation_time() {
        let db = Db::open_memory().await.unwrap();
        assert_eq!(
            scalar_text(&db, "SELECT value FROM meta WHERE key = 'schema'").await,
            "1"
        );
        let created: Ms = scalar_text(&db, "SELECT value FROM meta WHERE key = 'created_at'")
            .await
            .parse()
            .unwrap();
        assert!((time::now_ms() - created).abs() < 60_000, "{created}");
    }

    #[tokio::test]
    async fn default_tags_are_seeded_once_per_data_store() {
        let tmp = temp_config_dir();
        let file = tmp.path().join(DB_FILE);

        let db = Db::open(&file).await.unwrap();
        assert_eq!(tag_names(&db).await, sorted_default_tags());
        db.close().await;

        let db = Db::open(&file).await.unwrap();
        assert_eq!(tag_names(&db).await, sorted_default_tags());
        sqlx::query("DELETE FROM tags")
            .execute(db.pool())
            .await
            .unwrap();
        db.close().await;

        let db = Db::open(&file).await.unwrap();
        assert!(tag_names(&db).await.is_empty());
    }

    #[tokio::test]
    async fn open_fails_with_db_when_the_path_is_a_directory() {
        let tmp = temp_config_dir();
        let file = tmp.path().join(DB_FILE);
        std::fs::create_dir(&file).unwrap();
        let err = Db::open(&file).await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Db);
    }

    #[tokio::test]
    async fn read_only_open_of_a_missing_file_says_there_is_no_data_yet() {
        let tmp = temp_config_dir();
        let file = tmp.path().join(DB_FILE);

        let err = Db::open_read_only(&file).await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(
            err.to_string(),
            format!("TimeTracker has no data yet ({} not found)", file.display())
        );
        assert!(!file.exists());
    }

    #[tokio::test]
    async fn read_only_open_reads_what_the_app_wrote_and_refuses_writes() {
        let tmp = temp_config_dir();
        let file = tmp.path().join(DB_FILE);
        let writer = Db::open(&file).await.unwrap();
        tags::insert(&writer, "billing review").await.unwrap();

        let reader = Db::open_read_only(&file).await.unwrap();

        let names: Vec<String> = tags::list(&reader)
            .await
            .unwrap()
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert!(names.contains(&"billing review".to_string()), "{names:?}");
        let err = tags::insert(&reader, "another").await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Db);
        assert_eq!(tags::list(&writer).await.unwrap().len(), names.len());
    }

    #[tokio::test]
    async fn read_only_open_refuses_a_file_older_than_the_migrations() {
        let tmp = temp_config_dir();
        let file = tmp.path().join(DB_FILE);
        let writer = Db::open(&file).await.unwrap();
        sqlx::query("DELETE FROM _sqlx_migrations")
            .execute(writer.pool())
            .await
            .unwrap();
        writer.close().await;

        let err = Db::open_read_only(&file).await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(err.to_string(), "Open TimeTracker once to update its data");
    }

    #[test]
    fn data_dir_uses_an_absolute_setting_and_else_the_config_data_folder() {
        let config = Path::new("/config");
        for (setting, expected) in [
            (None, "/config/data"),
            (Some("/Volumes/backup/tt"), "/Volumes/backup/tt"),
            (Some("relative/dir"), "/config/data"),
        ] {
            let settings = Settings {
                data_dir: setting.map(String::from),
                ..Settings::default()
            };
            assert_eq!(
                data_dir(&settings, config),
                PathBuf::from(expected),
                "{setting:?}"
            );
        }
    }

    #[tokio::test]
    async fn data_store_opens_a_good_path() {
        let tmp = temp_config_dir();
        let store = DataStore::open(tmp.path().join(DB_FILE)).await;
        assert!(store.db().is_ok());
        assert_eq!(store.error(), None);
        store.close().await;
    }

    #[tokio::test]
    async fn data_store_keeps_running_when_the_file_cannot_be_opened() {
        let tmp = temp_config_dir();
        let file = tmp.path().join(DB_FILE);
        std::fs::create_dir(&file).unwrap();
        let store = DataStore::open(file).await;

        let err = store.db().unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Db);
        assert_eq!(err.to_string(), "The data store is not available");
        assert!(store.error().is_some());
    }
}
