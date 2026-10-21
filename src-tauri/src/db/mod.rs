//! The SQLite data store (ADR 0006): one file in the data folder, a small sqlx pool and embedded
//! forward-only migrations. Each table has its repository module.

pub mod activities;
pub mod away;
pub mod backup;
pub mod clients;
pub mod commands;
pub mod projects;
pub mod recordings;
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

pub const DB_FILE: &str = "timetracker.sqlite";
/// SQLite has one writer; WAL lets readers run next to it.
const MAX_CONNECTIONS: u32 = 4;
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// A cheap handle to the pool; clones share it.
#[derive(Clone, Debug)]
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    /// Creates the file and its private parent folder when missing, then migrates.
    pub async fn open(path: &Path) -> AppResult<Db> {
        if let Some(dir) = path.parent() {
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
        Db::init(pool).await
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
        Db::init(pool).await
    }

    async fn init(pool: SqlitePool) -> AppResult<Db> {
        sqlx::migrate!("./migrations").run(&pool).await?;
        let db = Db { pool };
        tags::seed_defaults(&db).await?;
        Ok(db)
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
        _ => config_dir.join("data"),
    }
}

/// The database file and the open database, or why it could not be opened. The app keeps running
/// without it; commands that need it fail with `db`.
pub struct DataStore {
    file: PathBuf,
    db: RwLock<Result<Db, String>>,
}

impl DataStore {
    pub async fn open(file: PathBuf) -> DataStore {
        let db = Db::open(&file).await.map_err(|e| {
            log::error!("Cannot open the data store at {}: {e}", file.display());
            e.to_string()
        });
        DataStore {
            file,
            db: RwLock::new(db),
        }
    }

    pub fn file(&self) -> &Path {
        &self.file
    }

    pub fn db(&self) -> AppResult<Db> {
        self.db.read_unpoisoned().clone().map_err(|_| AppError::Db {
            message: "The data store is not available".into(),
            code: None,
        })
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

/// `field` is the input's key as the page sends it (`clientId`).
fn invalid_field(field: &str, message: impl Into<String>) -> AppError {
    AppError::InvalidField {
        field: field.into(),
        message: message.into(),
    }
}

/// The trimmed name of a client, project or tag.
fn required_name(name: &str) -> AppResult<String> {
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

fn new_id() -> String {
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
            "recording_tags",
        ] {
            assert!(tables.iter().any(|t| t == table), "{table} in {tables:?}");
        }
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
