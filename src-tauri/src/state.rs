use std::path::{Path, PathBuf};

use crate::db::{self, DB_FILE, DataStore};
use crate::launch::LaunchQueue;
use crate::recordings::undo::UndoStack;
use crate::store::keybindings::KeybindingsStore;
use crate::store::settings::{LogLevel, SettingsStore};
use crate::store::ui_state::UiStateStore;

pub struct AppState {
    pub settings: SettingsStore,
    pub keybindings: KeybindingsStore,
    pub ui_state: UiStateStore,
    pub launches: LaunchQueue,
    pub data: DataStore,
    /// Recording changes of this run; also serializes recording writes.
    pub undo: tokio::sync::Mutex<UndoStack>,
    pub config_dir: PathBuf,
    /// `--log-level` from the CLI; wins over the stored level until the app quits.
    pub log_override: Option<LogLevel>,
}

impl AppState {
    /// Blocks while the database opens, so it must not run inside async code.
    pub fn open(config_dir: &Path) -> Self {
        let settings = SettingsStore::open(config_dir);
        let db_file = db::data_dir(&settings.get(), config_dir).join(DB_FILE);
        Self {
            data: tauri::async_runtime::block_on(DataStore::open(db_file)),
            settings,
            keybindings: KeybindingsStore::open(config_dir),
            ui_state: UiStateStore::open(config_dir),
            launches: LaunchQueue::default(),
            undo: Default::default(),
            config_dir: config_dir.to_path_buf(),
            log_override: None,
        }
    }
}
