//! Window layout that survives a restart (`ui_state.json`): sidebar, pane sizes, the timeline zoom and
//! whether the welcome sheet was finished. Unknown keys, such as the dropped `dayMode` and `lastDay`, are
//! ignored.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::json_store::JsonStore;
use crate::error::AppResult;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(default, rename_all = "camelCase")]
pub struct UiState {
    pub sidebar_hidden: bool,
    /// In px.
    pub sidebar_width: u16,
    /// The Day view's scale in px per minute.
    pub timeline_zoom: f32,
    /// Splitter positions by pane id, in px.
    pub pane_sizes: HashMap<String, f32>,
    /// The welcome sheet was finished; it shows while this is false.
    pub onboarding_done: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            sidebar_hidden: false,
            sidebar_width: 200,
            timeline_zoom: 2.5,
            pane_sizes: HashMap::new(),
            onboarding_done: false,
        }
    }
}

pub struct UiStateStore {
    store: JsonStore<UiState>,
}

impl UiStateStore {
    pub fn open(dir: &Path) -> Self {
        Self {
            store: JsonStore::open(dir.join("ui_state.json")),
        }
    }

    pub fn get(&self) -> UiState {
        self.store.read(UiState::clone)
    }

    pub async fn set(&self, ui: UiState) -> AppResult<UiState> {
        self.store
            .update_async(move |current| {
                *current = ui.clone();
                ui
            })
            .await
    }

    pub async fn finish_onboarding(&self) -> AppResult<()> {
        self.store
            .update_async(|current| current.onboarding_done = true)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_config_dir;

    #[test]
    fn defaults_apply_without_a_file() {
        let dir = temp_config_dir();
        let ui = UiStateStore::open(dir.path()).get();
        assert_eq!(
            ui,
            UiState {
                sidebar_hidden: false,
                sidebar_width: 200,
                timeline_zoom: 2.5,
                pane_sizes: HashMap::new(),
                onboarding_done: false,
            }
        );
    }

    #[tokio::test]
    async fn saved_state_persists_across_reopen() {
        let dir = temp_config_dir();
        let ui = UiState {
            sidebar_hidden: true,
            sidebar_width: 260,
            timeline_zoom: 1.5,
            pane_sizes: HashMap::from([("timeline".to_string(), 320.0)]),
            onboarding_done: true,
        };
        let saved = UiStateStore::open(dir.path())
            .set(ui.clone())
            .await
            .unwrap();
        assert_eq!(saved, ui);
        assert_eq!(UiStateStore::open(dir.path()).get(), ui);
    }

    #[test]
    fn partial_file_fills_the_missing_fields_with_defaults() {
        let dir = temp_config_dir();
        std::fs::write(dir.path().join("ui_state.json"), r#"{"sidebarWidth": 320}"#).unwrap();
        assert_eq!(
            UiStateStore::open(dir.path()).get(),
            UiState {
                sidebar_width: 320,
                ..UiState::default()
            }
        );
    }

    #[test]
    fn keys_are_camel_case() {
        let ui = UiState::default();
        assert_eq!(
            serde_json::to_value(&ui).unwrap(),
            serde_json::json!({
                "sidebarHidden": false,
                "sidebarWidth": 200,
                "timelineZoom": 2.5,
                "paneSizes": {},
                "onboardingDone": false
            })
        );
    }

    #[test]
    fn a_file_without_onboarding_done_shows_the_welcome_sheet() {
        let dir = temp_config_dir();
        std::fs::write(dir.path().join("ui_state.json"), r#"{"sidebarWidth": 320}"#).unwrap();
        assert!(!UiStateStore::open(dir.path()).get().onboarding_done);
    }

    #[test]
    fn onboarding_done_is_read_from_its_camel_case_key() {
        let ui = serde_json::from_str::<UiState>(r#"{"onboardingDone": true}"#).unwrap();
        assert!(ui.onboarding_done);
    }

    #[tokio::test]
    async fn finishing_onboarding_persists_and_keeps_the_other_fields() {
        let dir = temp_config_dir();
        let store = UiStateStore::open(dir.path());
        let ui = UiState {
            sidebar_width: 260,
            ..UiState::default()
        };
        store.set(ui.clone()).await.unwrap();

        store.finish_onboarding().await.unwrap();

        let expected = UiState {
            onboarding_done: true,
            ..ui
        };
        assert_eq!(store.get(), expected);
        assert_eq!(UiStateStore::open(dir.path()).get(), expected);
    }

    #[tokio::test]
    async fn finishing_onboarding_twice_stays_done() {
        let dir = temp_config_dir();
        let store = UiStateStore::open(dir.path());
        store.finish_onboarding().await.unwrap();
        store.finish_onboarding().await.unwrap();
        assert!(UiStateStore::open(dir.path()).get().onboarding_done);
    }

    #[test]
    fn a_file_with_the_dropped_last_day_and_day_mode_keys_loads_without_quarantining_the_file() {
        let dir = temp_config_dir();
        let raw = r#"{"sidebarWidth": 320, "lastDay": "2026-10-21", "dayMode": "list", "onboardingDone": true}"#;
        std::fs::write(dir.path().join("ui_state.json"), raw).unwrap();

        let loaded = UiStateStore::open(dir.path()).get();

        assert_eq!(
            loaded,
            UiState {
                sidebar_width: 320,
                onboarding_done: true,
                ..UiState::default()
            }
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("ui_state.json")).unwrap(),
            raw
        );
        let mut names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, vec!["ui_state.json".to_string()]);
    }

    #[test]
    fn a_file_with_the_old_zoom_key_loads_with_the_default_timeline_zoom() {
        let dir = temp_config_dir();
        std::fs::write(
            dir.path().join("ui_state.json"),
            r#"{"sidebarWidth": 320, "zoom": 1.5}"#,
        )
        .unwrap();
        assert_eq!(
            UiStateStore::open(dir.path()).get(),
            UiState {
                sidebar_width: 320,
                timeline_zoom: 2.5,
                ..UiState::default()
            }
        );
    }
}
