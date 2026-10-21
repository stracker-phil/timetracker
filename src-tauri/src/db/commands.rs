use std::path::Path;

use serde::Serialize;
use tauri::State;
use ts_rs::TS;

use super::backup::{self, BackupWritten};
use super::time::Ms;
use super::{DataStore, Db, activities, projects, recordings};
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
