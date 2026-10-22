use std::path::{Path, PathBuf};

use crate::activity::delete::DeleteBuffer;
use crate::db::{self, DB_FILE, DataStore};
use crate::ipc::IpcMessage;
use crate::launch::LaunchQueue;
use crate::recordings::undo::UndoStack;
use crate::store::keybindings::KeybindingsStore;
use crate::store::settings::SettingsStore;
use crate::store::ui_state::UiStateStore;

pub struct AppState {
    pub settings: SettingsStore,
    pub keybindings: KeybindingsStore,
    pub ui_state: UiStateStore,
    pub launches: LaunchQueue,
    pub data: DataStore,
    /// Recording changes of this run; also serializes recording writes.
    pub undo: tokio::sync::Mutex<UndoStack>,
    /// The last activity delete, for its undo window.
    pub deleted_activity: DeleteBuffer,
    pub config_dir: PathBuf,
}

impl AppState {
    /// Blocks while the database opens, so it must not run inside async code.
    pub fn open(config_dir: &Path) -> Self {
        let settings = SettingsStore::open(config_dir);
        let launches = LaunchQueue::default();
        let db_file = data_dir_at_startup(&settings, config_dir, &launches).join(DB_FILE);
        let data = tauri::async_runtime::block_on(DataStore::open(db_file));
        if let Some(notice) = data.migration_notice() {
            launches.notice(notice.to_string());
        }
        Self {
            data,
            settings,
            keybindings: KeybindingsStore::open(config_dir),
            ui_state: UiStateStore::open(config_dir),
            launches,
            undo: Default::default(),
            deleted_activity: DeleteBuffer::default(),
            config_dir: config_dir.to_path_buf(),
        }
    }
}

/// A chosen data folder that is gone (an unmounted volume) leaves the setting alone; this run uses
/// the default folder and the page says so once it loads.
fn data_dir_at_startup(
    settings: &SettingsStore,
    config_dir: &Path,
    launches: &LaunchQueue,
) -> PathBuf {
    let dir = db::data_dir(&settings.get(), config_dir);
    let default = db::default_data_dir(config_dir);
    if dir == default || dir.is_dir() {
        return dir;
    }
    let message = format!("Data folder {} not found; using the default", dir.display());
    log::error!("{message}");
    launches.push(IpcMessage::Error { message });
    default
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::LaunchEvent;
    use crate::test_support::temp_config_dir;

    fn store_data_dir(config_dir: &Path, data_dir: &Path) {
        let raw = serde_json::json!({ "dataDir": data_dir.display().to_string() });
        std::fs::write(config_dir.join("settings.json"), raw.to_string()).unwrap();
    }

    #[test]
    fn startup_opens_the_default_data_folder_without_a_chosen_one() {
        let config = temp_config_dir();

        let state = AppState::open(config.path());

        assert_eq!(state.data.file(), config.path().join("data").join(DB_FILE));
        assert!(state.data.db().is_ok());
        assert_eq!(state.launches.take_events(), vec![]);
    }

    #[test]
    fn startup_queues_a_notice_when_the_migration_dropped_tags() {
        use crate::db::one_tag::{TagsDropped, legacy};

        let config = temp_config_dir();
        let data = config.path().join("data");
        std::fs::create_dir_all(&data).unwrap();
        tauri::async_runtime::block_on(legacy::create(
            &data.join(DB_FILE),
            legacy::RECORDING_WITH_THREE_TAGS,
        ));

        let state = AppState::open(config.path());

        let expected = TagsDropped {
            recordings: 1,
            rules: 0,
            backup: data.join(format!("pre-0006-{}.json", db::time::Day::today())),
        }
        .message();
        assert_eq!(
            state.launches.take_events(),
            vec![LaunchEvent::Notice { message: expected }]
        );
    }

    #[test]
    fn startup_opens_the_chosen_data_folder_when_it_exists() {
        let config = temp_config_dir();
        let chosen = temp_config_dir();
        store_data_dir(config.path(), chosen.path());

        let state = AppState::open(config.path());

        assert_eq!(state.data.file(), chosen.path().join(DB_FILE));
        assert!(state.data.db().is_ok());
        assert_eq!(state.launches.take_events(), vec![]);
    }

    #[test]
    fn startup_falls_back_to_the_default_folder_when_the_chosen_one_is_gone() {
        let config = temp_config_dir();
        let gone = config.path().join("unmounted-volume");
        store_data_dir(config.path(), &gone);

        let state = AppState::open(config.path());

        assert_eq!(state.data.file(), config.path().join("data").join(DB_FILE));
        assert!(state.data.db().is_ok());
        assert_eq!(
            state.settings.get().data_dir,
            Some(gone.display().to_string())
        );
        assert_eq!(
            state.launches.take_events(),
            vec![LaunchEvent::Error {
                message: format!(
                    "Data folder {} not found; using the default",
                    gone.display()
                )
            }]
        );
    }
}
