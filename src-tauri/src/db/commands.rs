use std::path::Path;

use serde::Serialize;
use tauri::State;
use ts_rs::TS;

use super::activities::{ActivityRow, AppSeen};
use super::away::AwayRow;
use super::backup::{self, BackupWritten};
use super::time::{Day, Ms, now_ms};
use super::{DataStore, Db, activities, away, projects, recordings};
use crate::error::AppResult;
use crate::state::AppState;

/// Settings → Data. When the store is not available the counts are zero and `error` says why.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct DataInfo {
    pub dir: String,
    pub file: String,
    /// The database file plus its WAL side files.
    pub size_bytes: u64,
    pub activities: i64,
    pub recordings: i64,
    pub projects: i64,
    pub first_activity: Option<Ms>,
    pub available: bool,
    pub error: Option<String>,
    /// The `timetracker` command for `claude mcp add`: the installed link, else this executable.
    pub binary_path: String,
}

#[derive(Default)]
struct Counts {
    activities: i64,
    recordings: i64,
    projects: i64,
    first_activity: Option<Ms>,
}

async fn read_counts(db: &Db) -> AppResult<Counts> {
    Ok(Counts {
        activities: activities::count(db).await?,
        recordings: recordings::count(db).await?,
        projects: projects::count(db).await?,
        first_activity: activities::first_start(db).await?,
    })
}

const INSTALLED_CLI: &str = ".local/bin/timetracker";

fn binary_path() -> String {
    std::env::home_dir()
        .map(|home| home.join(INSTALLED_CLI))
        .filter(|link| link.exists())
        .or_else(|| std::env::current_exe().ok())
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "timetracker".into())
}

fn size_on_disk(file: &Path) -> u64 {
    ["", "-wal", "-shm"]
        .iter()
        .filter_map(|suffix| {
            let mut name = file.as_os_str().to_owned();
            name.push(suffix);
            std::fs::metadata(name).ok()
        })
        .filter(|meta| meta.is_file())
        .map(|meta| meta.len())
        .sum()
}

pub async fn info(store: &DataStore) -> DataInfo {
    let file = store.file();
    let (available, counts, error) = match store.db() {
        Ok(db) => match read_counts(&db).await {
            Ok(counts) => (true, counts, None),
            Err(e) => (true, Counts::default(), Some(e.to_string())),
        },
        Err(e) => (
            false,
            Counts::default(),
            Some(store.error().unwrap_or_else(|| e.to_string())),
        ),
    };
    DataInfo {
        dir: file
            .parent()
            .map(|dir| dir.display().to_string())
            .unwrap_or_default(),
        file: file.display().to_string(),
        size_bytes: size_on_disk(file),
        activities: counts.activities,
        recordings: counts.recordings,
        projects: counts.projects,
        first_activity: counts.first_activity,
        available,
        error,
        binary_path: binary_path(),
    }
}

#[tauri::command]
pub async fn data_info(state: State<'_, AppState>) -> AppResult<DataInfo> {
    Ok(info(&state.data).await)
}

#[tauri::command]
pub async fn backup_export(state: State<'_, AppState>, path: String) -> AppResult<BackupWritten> {
    backup::write_to(&state.data.db()?, Path::new(&path)).await
}

const DAY_MS: Ms = 86_400_000;

/// The apps the tracker recorded in the last `days` days.
#[tauri::command]
pub async fn apps_seen(state: State<'_, AppState>, days: u16) -> AppResult<Vec<AppSeen>> {
    let now = now_ms();
    activities::apps_seen(&state.data.db()?, now - Ms::from(days) * DAY_MS, now).await
}

/// The day's activity, cut to the day; open rows end now.
#[tauri::command]
pub async fn activity_list(state: State<'_, AppState>, day: Day) -> AppResult<Vec<ActivityRow>> {
    let (from, to) = day.bounds();
    activities::list_clipped(&state.data.db()?, from, to, now_ms()).await
}

/// The day's away spans, cut to the day; an open span ends now.
#[tauri::command]
pub async fn away_list(state: State<'_, AppState>, day: Day) -> AppResult<Vec<AwayRow>> {
    let (from, to) = day.bounds();
    away::list_clipped(&state.data.db()?, from, to, now_ms()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DB_FILE;
    use crate::test_support::{activity, project, recording, temp_config_dir};

    #[tokio::test]
    async fn info_of_a_fresh_store_is_empty_but_available() {
        let tmp = temp_config_dir();
        let file = tmp.path().join("data").join(DB_FILE);
        let store = DataStore::open(file.clone()).await;

        let info = info(&store).await;

        assert!(info.available);
        assert_eq!(info.error, None);
        assert_eq!(info.file, file.display().to_string());
        assert_eq!(info.dir, file.parent().unwrap().display().to_string());
        assert_eq!((info.activities, info.recordings, info.projects), (0, 0, 0));
        assert_eq!(info.first_activity, None);
        assert!(info.size_bytes > 0);
        store.close().await;
    }

    #[tokio::test]
    async fn info_counts_rows_and_reports_the_first_activity() {
        let tmp = temp_config_dir();
        let store = DataStore::open(tmp.path().join("data").join(DB_FILE)).await;
        let db = store.db().unwrap();
        activity(&db, 1000, Some(2000), "Xcode", "main.rs").await;
        activity(&db, 3000, None, "Safari", "Docs").await;
        let p = project(&db, "Website").await;
        recording(&db, &p.id, 1000, Some(2000)).await;

        let info = info(&store).await;

        assert_eq!((info.activities, info.recordings, info.projects), (2, 1, 1));
        assert_eq!(info.first_activity, Some(1000));
        store.close().await;
    }

    #[tokio::test]
    async fn info_names_the_command_to_register_with_mcp_clients() {
        let tmp = temp_config_dir();
        let store = DataStore::open(tmp.path().join("data").join(DB_FILE)).await;
        let installed = std::env::home_dir().map(|home| home.join(INSTALLED_CLI));
        let expected = match installed.filter(|link| link.exists()) {
            Some(link) => link,
            None => std::env::current_exe().unwrap(),
        };

        let info = info(&store).await;

        assert_eq!(info.binary_path, expected.display().to_string());
        assert_eq!(
            serde_json::to_value(&info).unwrap()["binaryPath"],
            serde_json::Value::String(info.binary_path.clone())
        );
        store.close().await;
    }

    #[tokio::test]
    async fn info_of_an_unavailable_store_reports_why() {
        let tmp = temp_config_dir();
        let file = tmp.path().join(DB_FILE);
        std::fs::create_dir(&file).unwrap();
        let store = DataStore::open(file).await;

        let info = info(&store).await;

        assert!(!info.available);
        assert!(info.error.is_some());
        assert_eq!((info.activities, info.recordings, info.projects), (0, 0, 0));
        assert_eq!(info.first_activity, None);
    }
}
