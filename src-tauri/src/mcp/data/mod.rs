//! The data tools: the database opened read-only on first use, the settings read on every call.

mod activity;
mod catalog;
mod day_summary;
#[cfg(test)]
mod fixtures;
mod recordings;
mod report;
mod status;
mod suggestions;

use std::path::PathBuf;

use chrono::Local;
use tokio::sync::OnceCell;

use crate::cli::time;
use crate::db::time::{Day, Ms, now_ms};
use crate::db::{self, DB_FILE, Db};
use crate::error::{AppError, AppResult};
use crate::reports::{self, ReportFilter, ReportResult};
use crate::store::settings::Settings;

#[cfg(test)]
pub use activity::ACTIVITY_HIDDEN;
pub use recordings::RecordingsQuery;
pub use report::{ReportQuery, report_out};
pub use status::status_out;

/// Minutes to one decimal.
fn minutes(ms: Ms) -> f64 {
    (ms as f64 / 6_000.0).round() / 10.0
}

pub fn day_of(text: &str) -> AppResult<Day> {
    time::day(text, now_ms(), &Local).map_err(AppError::InvalidInput)
}

pub struct Core {
    config_dir: PathBuf,
    db: OnceCell<Db>,
}

impl Core {
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            config_dir,
            db: OnceCell::new(),
        }
    }

    fn settings(&self) -> Settings {
        Settings::peek(&self.config_dir)
    }

    pub fn expose_activity(&self) -> bool {
        self.settings().mcp.expose_activity
    }

    /// Opened on first use, so the handshake succeeds before the app ever ran; a failed open is tried
    /// again on the next call.
    async fn db(&self) -> AppResult<&Db> {
        self.db
            .get_or_try_init(|| async {
                let file = db::data_dir(&self.settings(), &self.config_dir).join(DB_FILE);
                Db::open_read_only(&file).await
            })
            .await
    }

    async fn run_filter(&self, filter: &ReportFilter) -> AppResult<ReportResult> {
        reports::run(self.db().await?, filter, Day::today(), now_ms()).await
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::fixtures::{seeded, write_settings};
    use super::*;
    use crate::error::ErrorKind;
    use crate::test_support::{project, temp_config_dir};

    #[tokio::test]
    async fn a_missing_database_file_says_there_is_no_data_yet() {
        let dir = temp_config_dir();
        let file = dir.path().join("data").join(DB_FILE);
        let core = Core::new(dir.path().to_path_buf());
        let expected = format!("TimeTracker has no data yet ({} not found)", file.display());

        for _ in 0..2 {
            let err = core.projects().await.err().unwrap();
            assert_eq!(err.kind(), ErrorKind::NotFound);
            assert_eq!(err.to_string(), expected);
        }
        assert!(!file.exists());
    }

    #[tokio::test]
    async fn the_database_is_found_once_the_app_has_created_it() {
        let dir = temp_config_dir();
        let core = Core::new(dir.path().to_path_buf());
        assert!(core.tags().await.is_err());

        Db::open(&dir.path().join("data").join(DB_FILE))
            .await
            .unwrap();

        assert!(!core.tags().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn without_a_settings_file_the_data_folder_is_in_the_config_dir() {
        let seed = seeded().await;
        assert!(!seed.dir.path().join("settings.json").exists());

        let names: Vec<String> = seed
            .core
            .projects()
            .await
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();

        assert_eq!(names.len(), 2);
    }

    #[tokio::test]
    async fn the_data_folder_setting_points_to_the_database() {
        let config = temp_config_dir();
        let elsewhere = temp_config_dir();
        let db = Db::open(&elsewhere.path().join(DB_FILE)).await.unwrap();
        project(&db, "Elsewhere").await;
        write_settings(
            config.path(),
            json!({"dataDir": elsewhere.path().display().to_string()}),
        );
        let core = Core::new(config.path().to_path_buf());

        let names: Vec<String> = core
            .projects()
            .await
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();

        assert_eq!(names, ["Elsewhere"]);
    }
}
