//! Window layout that survives a restart (`ui_state.json`): sidebar, pane sizes, the last viewed day
//! and the timeline zoom.

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
    /// `YYYY-MM-DD`.
    pub last_day: Option<String>,
    /// The Day view's scale in px per minute.
    pub timeline_zoom: f32,
    /// Splitter positions by pane id, in px.
    pub pane_sizes: HashMap<String, f32>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            sidebar_hidden: false,
            sidebar_width: 200,
            last_day: None,
            timeline_zoom: 2.5,
            pane_sizes: HashMap::new(),
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
                last_day: None,
                timeline_zoom: 2.5,
                pane_sizes: HashMap::new(),
            }
        );
    }

    #[tokio::test]
    async fn saved_state_persists_across_reopen() {
        let dir = temp_config_dir();
        let ui = UiState {
            sidebar_hidden: true,
            sidebar_width: 260,
            last_day: Some("2026-10-21".into()),
            timeline_zoom: 1.5,
            pane_sizes: HashMap::from([("timeline".to_string(), 320.0)]),
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
        let ui = UiState {
            last_day: Some("2026-10-21".into()),
            ..UiState::default()
        };
        assert_eq!(
            serde_json::to_value(&ui).unwrap(),
            serde_json::json!({
                "sidebarHidden": false,
                "sidebarWidth": 200,
                "lastDay": "2026-10-21",
                "timelineZoom": 2.5,
                "paneSizes": {}
            })
        );
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
