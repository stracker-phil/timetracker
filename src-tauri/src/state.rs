use std::path::{Path, PathBuf};

use crate::launch::LaunchQueue;
use crate::store::keybindings::KeybindingsStore;
use crate::store::settings::{LogLevel, SettingsStore};
use crate::store::ui_state::UiStateStore;

pub struct AppState {
    pub settings: SettingsStore,
    pub keybindings: KeybindingsStore,
    pub ui_state: UiStateStore,
    pub launches: LaunchQueue,
    pub config_dir: PathBuf,
    /// `--log-level` from the CLI; wins over the stored level until the app quits.
    pub log_override: Option<LogLevel>,
}

impl AppState {
    pub fn open(config_dir: &Path) -> Self {
        Self {
            settings: SettingsStore::open(config_dir),
            keybindings: KeybindingsStore::open(config_dir),
            ui_state: UiStateStore::open(config_dir),
            launches: LaunchQueue::default(),
            config_dir: config_dir.to_path_buf(),
            log_override: None,
        }
    }
}
