//! Moves the database to another folder while the app runs (Settings → Data). Until the new folder
//! is in the settings the old one stays authoritative: every failure before that reopens it. The old
//! files stay where they were; the user deletes them.

use std::fs::{DirBuilder, OpenOptions};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

use super::commands::{DataInfo, info};
use super::{DB_FILE, DataStore, Db};
use crate::error::{AppError, AppResult};
use crate::store::settings::SettingsStore;
use crate::tracker::{Suspended, Tracker};

/// What commands that need the database say while it moves.
const MOVING: &str = "The data folder is moving";
const WAL_SUFFIXES: [&str; 2] = ["-wal", "-shm"];
const WRITE_PROBE: &str = ".timetracker-write-check";

/// Moves the database into `target` and switches to it; on failure the old file stays active.
pub async fn move_data(
    data: &DataStore,
    settings: &SettingsStore,
    tracker: Option<&Tracker>,
    target: &Path,
) -> AppResult<DataInfo> {
    let _moving = data.moving.lock().await;
    let from = data.file();
    let to = prepare_target(&from, target)?.join(DB_FILE);
    // Refuse before suspending the tracker when there is no open store to move.
    data.db()?;
    let suspended = match tracker {
        Some(tracker) => Some(tracker.suspend().await),
        None => None,
    };
    let old = data.take(MOVING)?;
    match switch(&old, settings, &from, &to).await {
        Ok(db) => {
            data.put(to, Ok(db.clone()));
            if let Some(suspended) = suspended {
                suspended.resume(db);
            }
            log::info!("Moved the data folder to {}", data.file().display());
        }
        Err(error) => {
            roll_back(data, old, suspended, from, &to).await;
            log::warn!(
                "Couldn't move the data folder to {}: {error}",
                target.display()
            );
            return Err(error);
        }
    }
    Ok(info(data).await)
}

/// Drops the copies and makes the old file active again.
async fn roll_back(
    data: &DataStore,
    old: Db,
    suspended: Option<Suspended<'_>>,
    from: PathBuf,
    to: &Path,
) {
    old.close().await;
    remove_copies(to);
    let reopened = Db::open(&from).await;
    if let (Some(suspended), Ok(db)) = (suspended, &reopened) {
        suspended.resume(db.clone());
    }
    data.put(from, reopened.map_err(|e| e.to_string()));
}

/// Moves `db` into `to` and points the settings at it; the settings change back when it does not open.
async fn switch(db: &Db, settings: &SettingsStore, from: &Path, to: &Path) -> AppResult<Db> {
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(db.pool())
        .await?;
    db.close().await;
    copy_files(from, to).await?;
    verify(to).await?;
    let dir = to.parent().unwrap_or(to).display().to_string();
    let (previous, _) = settings.set_data_dir(Some(dir)).await?;
    match Db::open(to).await {
        Ok(db) => Ok(db),
        Err(e) => {
            if let Err(revert) = settings.set_data_dir(previous.data_dir).await {
                log::error!("Couldn't restore the data folder setting: {revert}");
            }
            Err(e)
        }
    }
}

/// `target` canonicalized and created (private) if missing, or why it can't take the data.
fn prepare_target(current_file: &Path, target: &Path) -> AppResult<PathBuf> {
    if !target.is_absolute() {
        return Err(AppError::InvalidInput(format!(
            "{} is not a full path",
            target.display()
        )));
    }
    let current_dir = current_file.parent().unwrap_or(current_file);
    let inside = || {
        AppError::InvalidInput(format!(
            "{} is inside the current data folder",
            target.display()
        ))
    };
    if target.starts_with(current_dir) {
        return Err(inside());
    }
    if !target.exists() {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(target)
            .map_err(|e| {
                AppError::InvalidInput(format!("Can't create {}: {e}", target.display()))
            })?;
    } else if !target.is_dir() {
        return Err(AppError::InvalidInput(format!(
            "{} is not a folder",
            target.display()
        )));
    }
    let target = target.canonicalize()?;
    if current_dir
        .canonicalize()
        .is_ok_and(|dir| target.starts_with(dir))
    {
        return Err(inside());
    }
    if target.join(DB_FILE).exists() {
        return Err(AppError::InvalidInput(format!(
            "{} already holds TimeTracker data",
            target.display()
        )));
    }
    let probe = target.join(WRITE_PROBE);
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|e| {
            AppError::InvalidInput(format!("{} is not writable: {e}", target.display()))
        })?;
    let _ = std::fs::remove_file(probe);
    Ok(target)
}

fn with_suffix(file: &Path, suffix: &str) -> PathBuf {
    let mut name = file.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Copies the database and whichever WAL files exist.
async fn copy_files(from: &Path, to: &Path) -> AppResult<()> {
    copy_file(from, to).await?;
    for suffix in WAL_SUFFIXES {
        let source = with_suffix(from, suffix);
        if source.is_file() {
            copy_file(&source, &with_suffix(to, suffix)).await?;
        }
    }
    Ok(())
}

async fn copy_file(source: &Path, destination: &Path) -> AppResult<()> {
    let written = tokio::fs::copy(source, destination).await?;
    if written != tokio::fs::metadata(source).await?.len() {
        return Err(AppError::Io(format!(
            "The copy of {} is incomplete",
            source.display()
        )));
    }
    Ok(())
}

async fn verify(file: &Path) -> AppResult<()> {
    let copy = Db::open_read_only(file).await?;
    let result: Result<String, sqlx::Error> = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(copy.pool())
        .await;
    copy.close().await;
    let problem = result?;
    if problem == "ok" {
        return Ok(());
    }
    Err(AppError::Db {
        message: format!("The copy in {} is damaged: {problem}", file.display()),
        code: None,
    })
}

fn remove_copies(to: &Path) {
    let _ = std::fs::remove_file(to);
    for suffix in WAL_SUFFIXES {
        let _ = std::fs::remove_file(with_suffix(to, suffix));
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;
    use crate::capture::fake::FakeSource;
    use crate::db::activities;
    use crate::db::time::Ms;
    use crate::error::ErrorKind;
    use crate::store::settings::TrackerSettings;
    use crate::test_support::{project, temp_config_dir};

    const T: Ms = 1_800_000_000_000;
    const SECOND: Ms = 1_000;

    struct Fixture {
        root: tempfile::TempDir,
        data: DataStore,
        settings: SettingsStore,
    }

    impl Fixture {
        async fn new() -> Fixture {
            let root = temp_config_dir();
            let settings = SettingsStore::open(root.path());
            let data = DataStore::open(root.path().join("data").join(DB_FILE)).await;
            project(&data.db().unwrap(), "Alpha").await;
            Fixture {
                root,
                data,
                settings,
            }
        }

        fn path(&self, name: &str) -> PathBuf {
            self.root.path().join(name)
        }

        async fn project_names(&self) -> Vec<String> {
            let mut names: Vec<String> = sqlx::query_scalar("SELECT name FROM projects")
                .fetch_all(self.data.db().unwrap().pool())
                .await
                .unwrap();
            names.sort();
            names
        }
    }

    #[tokio::test]
    async fn a_move_copies_the_data_updates_the_setting_and_reopens_the_store() {
        let f = Fixture::new().await;
        let old_file = f.data.file();
        let target = f.path("elsewhere");

        let info = move_data(&f.data, &f.settings, None, &target)
            .await
            .unwrap();

        let dir = target.canonicalize().unwrap();
        assert_eq!(info.dir, dir.display().to_string());
        assert_eq!(info.file, dir.join(DB_FILE).display().to_string());
        assert_eq!(info.projects, 1);
        assert!(info.available);
        assert_eq!(info.error, None);
        assert_eq!(f.data.file(), dir.join(DB_FILE));
        assert_eq!(f.settings.get().data_dir, Some(dir.display().to_string()));
        assert_eq!(f.project_names().await, ["Alpha"]);
        assert_eq!(
            std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert!(old_file.is_file());
    }

    #[tokio::test]
    async fn after_a_move_writes_go_to_the_new_file_and_the_old_one_keeps_its_rows() {
        let f = Fixture::new().await;
        let old_file = f.data.file();

        move_data(&f.data, &f.settings, None, &f.path("elsewhere"))
            .await
            .unwrap();
        project(&f.data.db().unwrap(), "Beta").await;

        assert_eq!(f.project_names().await, ["Alpha", "Beta"]);
        let old = Db::open_read_only(&old_file).await.unwrap();
        let old_names: Vec<String> = sqlx::query_scalar("SELECT name FROM projects")
            .fetch_all(old.pool())
            .await
            .unwrap();
        assert_eq!(old_names, ["Alpha"]);
    }

    #[tokio::test]
    async fn a_move_into_an_existing_empty_folder_works() {
        let f = Fixture::new().await;
        let target = f.path("existing");
        std::fs::create_dir(&target).unwrap();

        move_data(&f.data, &f.settings, None, &target)
            .await
            .unwrap();

        assert_eq!(f.data.file(), target.canonicalize().unwrap().join(DB_FILE));
        assert_eq!(f.project_names().await, ["Alpha"]);
    }

    #[tokio::test]
    async fn refused_targets_leave_the_old_store_active_and_the_setting_unchanged() {
        let f = Fixture::new().await;
        let old_file = f.data.file();
        let data_dir = old_file.parent().unwrap().to_path_buf();

        let holding_data = f.path("holding-data");
        std::fs::create_dir(&holding_data).unwrap();
        std::fs::write(holding_data.join(DB_FILE), b"").unwrap();

        let read_only = f.path("read-only");
        std::fs::create_dir(&read_only).unwrap();
        std::fs::set_permissions(&read_only, std::fs::Permissions::from_mode(0o500)).unwrap();

        let a_file = f.path("a-file");
        std::fs::write(&a_file, b"").unwrap();

        let link = f.path("link-into-data");
        std::os::unix::fs::symlink(&data_dir, &link).unwrap();

        let cases = [
            ("inside the data folder", data_dir.join("sub")),
            ("the data folder itself", data_dir.clone()),
            ("relative", PathBuf::from("relative/dir")),
            ("holding a database", holding_data),
            ("read only", read_only.clone()),
            ("a file", a_file),
            ("a link into the data folder", link),
        ];
        for (name, target) in cases {
            let err = move_data(&f.data, &f.settings, None, &target)
                .await
                .unwrap_err();

            assert_eq!(err.kind(), ErrorKind::InvalidInput, "{name}: {err}");
            assert_eq!(f.settings.get().data_dir, None, "{name}");
            assert_eq!(f.data.file(), old_file, "{name}");
            assert_eq!(f.project_names().await, ["Alpha"], "{name}");
        }
        assert!(!data_dir.join("sub").exists());
        std::fs::set_permissions(&read_only, std::fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[tokio::test]
    async fn the_tracker_keeps_recording_into_the_new_file_after_a_move() {
        let f = Fixture::new().await;
        let old_file = f.data.file();
        let source = FakeSource::new();
        let tracker = Tracker::new(
            f.data.db().unwrap(),
            source.clone(),
            TrackerSettings::default(),
            T,
        );
        source.show("Code", "a.rs");
        tracker.tick(T).await.unwrap();

        move_data(&f.data, &f.settings, Some(&tracker), &f.path("elsewhere"))
            .await
            .unwrap();
        source.show("Code", "b.rs");
        tracker.tick(T + 5 * SECOND).await.unwrap();

        let spans = |rows: Vec<activities::ActivityRow>| -> Vec<(Ms, Option<Ms>, String)> {
            rows.into_iter()
                .map(|r| (r.start, r.end, r.title))
                .collect()
        };
        let moved = activities::list_for_range(&f.data.db().unwrap(), 0, Ms::MAX)
            .await
            .unwrap();
        assert_eq!(
            spans(moved),
            [
                (T, Some(T + 5 * SECOND), "a.rs".to_string()),
                (T + 5 * SECOND, None, "b.rs".to_string()),
            ]
        );
        let old = Db::open_read_only(&old_file).await.unwrap();
        let left_behind = activities::list_for_range(&old, 0, Ms::MAX).await.unwrap();
        assert_eq!(spans(left_behind), [(T, None, "a.rs".to_string())]);
    }

    #[tokio::test]
    async fn a_refused_move_leaves_the_tracker_recording_into_the_old_file() {
        let f = Fixture::new().await;
        let source = FakeSource::new();
        let tracker = Tracker::new(
            f.data.db().unwrap(),
            source.clone(),
            TrackerSettings::default(),
            T,
        );
        source.show("Code", "a.rs");
        tracker.tick(T).await.unwrap();

        let err = move_data(&f.data, &f.settings, Some(&tracker), Path::new("relative"))
            .await
            .unwrap_err();
        source.show("Code", "b.rs");
        tracker.tick(T + 5 * SECOND).await.unwrap();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        let titles: Vec<String> = activities::list_for_range(&f.data.db().unwrap(), 0, Ms::MAX)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.title)
            .collect();
        assert_eq!(titles, ["a.rs", "b.rs"]);
    }
}
