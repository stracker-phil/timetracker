use std::path::Path;

use super::super::json_store::JsonStore;
use super::{AppSettingPatch, Settings};
use crate::error::AppResult;

const FILE_NAME: &str = "settings.json";

pub struct SettingsStore {
    store: JsonStore<Settings>,
}

impl SettingsStore {
    pub fn open(dir: &Path) -> Self {
        Self {
            store: JsonStore::open_with(dir.join(FILE_NAME), Settings::sanitized),
        }
    }

    pub fn get(&self) -> Settings {
        self.store.read(Settings::clone)
    }

    /// Returns `(previous, saved)`, both read under the same lock.
    pub async fn update(&self, settings: Settings) -> AppResult<(Settings, Settings)> {
        self.store
            .update_async(move |current| {
                let previous = std::mem::replace(current, settings.sanitized());
                (previous, current.clone())
            })
            .await
    }

    /// Only the data folder move changes `dataDir`. Returns `(previous, saved)`.
    pub async fn set_data_dir(&self, dir: Option<String>) -> AppResult<(Settings, Settings)> {
        self.store
            .update_async(move |current| {
                let next = Settings {
                    data_dir: dir,
                    ..current.clone()
                }
                .sanitized();
                let previous = std::mem::replace(current, next);
                (previous, current.clone())
            })
            .await
    }

    /// `Settings::with_app` on the current settings under the store's lock, so concurrent app
    /// changes never undo each other. Returns `(previous, saved)`.
    pub async fn update_app(
        &self,
        key: String,
        patch: AppSettingPatch,
    ) -> AppResult<(Settings, Settings)> {
        self.store
            .update_async(move |current| {
                let next = current.clone().with_app(&key, &patch)?.sanitized();
                let previous = std::mem::replace(current, next);
                Ok((previous, current.clone()))
            })
            .await?
    }
}

impl Settings {
    /// Reads the settings without the store's quarantine, so a short-lived second process
    /// never moves the running app's file aside.
    pub fn peek(dir: &Path) -> Settings {
        std::fs::read_to_string(dir.join(FILE_NAME))
            .ok()
            .and_then(|raw| serde_json::from_str::<Settings>(&raw).ok())
            .unwrap_or_default()
            .sanitized()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::tests::{patch, settings};
    use crate::store::settings::*;
    use crate::test_support::temp_config_dir;

    fn write_settings(dir: &Path, raw: &str) {
        std::fs::write(dir.join(FILE_NAME), raw).unwrap();
    }

    fn dir_entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[tokio::test]
    async fn a_part_1_appearance_loads_the_default_schemes_and_loses_its_keys_on_save() {
        let dir = temp_config_dir();
        write_settings(
            dir.path(),
            r#"{"theme": "dark", "lightScheme": "ayu-light", "darkScheme": "nord", "accent": "teal"}"#,
        );
        let store = SettingsStore::open(dir.path());

        let loaded = store.get();

        assert_eq!(loaded.light_scheme, Scheme::Default);
        assert_eq!(loaded.dark_scheme, Scheme::Dark);
        assert_eq!(loaded.accent, "teal");

        store.update(loaded).await.unwrap();
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap())
                .unwrap();
        assert!(written.get("theme").is_none());
        assert_eq!(written["lightScheme"], "default");
        assert_eq!(written["darkScheme"], "dark");
    }

    #[tokio::test]
    async fn a_step_07_appearance_moves_its_scheme_to_light_and_drops_mode_and_scheme_on_save() {
        let dir = temp_config_dir();
        write_settings(dir.path(), r#"{"mode": "dark", "scheme": "light"}"#);
        let store = SettingsStore::open(dir.path());

        let loaded = store.get();

        assert_eq!(loaded.light_scheme, Scheme::Light);
        assert_eq!(loaded.dark_scheme, Scheme::Dark);

        store.update(loaded).await.unwrap();
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap())
                .unwrap();
        assert!(written.get("mode").is_none());
        assert!(written.get("scheme").is_none());
        assert_eq!(written["lightScheme"], "light");
        let reopened = SettingsStore::open(dir.path()).get();
        assert_eq!(reopened.light_scheme, Scheme::Light);
        assert_eq!(reopened.dark_scheme, Scheme::Dark);
    }

    #[test]
    fn the_legacy_scheme_wins_over_a_light_scheme_in_the_same_file() {
        let dir = temp_config_dir();
        write_settings(dir.path(), r#"{"scheme": "dark", "lightScheme": "light"}"#);

        assert_eq!(Settings::peek(dir.path()).light_scheme, Scheme::Dark);
    }

    #[test]
    fn an_invalid_legacy_scheme_leaves_the_light_scheme_at_its_default() {
        let dir = temp_config_dir();
        write_settings(dir.path(), r#"{"scheme": "solarized"}"#);

        assert_eq!(Settings::peek(dir.path()).light_scheme, Scheme::Default);
    }

    #[tokio::test]
    async fn saving_through_update_never_writes_the_legacy_scheme() {
        let dir = temp_config_dir();
        let store = SettingsStore::open(dir.path());
        let mut next = settings(|s| s.light_scheme = Scheme::Light);
        next.legacy_scheme = Some(Scheme::Dark);

        let (_, saved) = store.update(next).await.unwrap();

        assert_eq!(saved.light_scheme, Scheme::Dark);
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap())
                .unwrap();
        assert!(written.get("scheme").is_none());
        assert_eq!(written["lightScheme"], "dark");
    }

    #[test]
    fn a_missing_null_or_invalid_dark_scheme_loads_as_dark_and_keeps_the_light_scheme() {
        for raw in [
            r#"{"lightScheme": "light"}"#,
            r#"{"lightScheme": "light", "darkScheme": null}"#,
            r#"{"lightScheme": "light", "darkScheme": "nord"}"#,
            r#"{"lightScheme": "light", "darkScheme": 7}"#,
        ] {
            let dir = temp_config_dir();
            write_settings(dir.path(), raw);

            let loaded = Settings::peek(dir.path());

            assert_eq!(
                (loaded.light_scheme, loaded.dark_scheme),
                (Scheme::Light, Scheme::Dark),
                "{raw}"
            );
        }
    }

    #[test]
    fn an_unknown_scheme_alone_falls_back_without_failing_the_file() {
        for (raw, light, dark) in [
            (
                r#"{"lightScheme": "neon", "darkScheme": "light"}"#,
                Scheme::Default,
                Scheme::Light,
            ),
            (
                r#"{"lightScheme": "dark", "darkScheme": "nord"}"#,
                Scheme::Dark,
                Scheme::Dark,
            ),
            (
                r#"{"lightScheme": 3, "darkScheme": null, "scheme": "nope"}"#,
                Scheme::Default,
                Scheme::Dark,
            ),
        ] {
            let dir = temp_config_dir();
            write_settings(dir.path(), raw);

            let loaded = Settings::peek(dir.path());

            assert_eq!(
                (loaded.light_scheme, loaded.dark_scheme),
                (light, dark),
                "{raw}"
            );
        }
    }

    #[test]
    fn unknown_enum_values_fall_back_to_defaults_without_quarantining_the_file() {
        let dir = temp_config_dir();
        let raw = r#"{
            "lightScheme": "neon",
            "darkScheme": "ayu-mirage",
            "logLevel": "verbose",
            "uiFontSize": 16,
            "timeline": {"continuity": "forever", "noise": "loud", "titles": "all", "zoom": 2.0, "compact": true},
            "reports": {"rounding": "nearest7", "defaultGroup": "year"},
            "apps": {"com.example.app": {"titles": "nope", "hidden": true}}
        }"#;
        write_settings(dir.path(), raw);

        let loaded = SettingsStore::open(dir.path()).get();

        assert_eq!(
            loaded,
            settings(|s| {
                s.ui_font_size = 16;
                s.timeline.zoom = 2.0;
                s.timeline.compact = true;
                s.apps.insert(
                    "com.example.app".into(),
                    AppOverride {
                        hidden: true,
                        ..AppOverride::default()
                    },
                );
            })
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap(),
            raw
        );
        assert_eq!(dir_entries(dir.path()), vec![FILE_NAME.to_string()]);
    }

    #[test]
    fn unknown_title_formats_fall_back_to_their_defaults_without_quarantining_the_file() {
        let dir = temp_config_dir();
        let raw = r#"{"calendarTitle": "tiny", "dayTitle": "medium", "uiFontSize": 16}"#;
        write_settings(dir.path(), raw);

        let loaded = SettingsStore::open(dir.path()).get();

        assert_eq!(loaded, settings(|s| s.ui_font_size = 16));
        assert_eq!(
            (loaded.calendar_title, loaded.day_title),
            (CalendarTitle::Short, DayTitle::Long)
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap(),
            raw
        );
        assert_eq!(dir_entries(dir.path()), vec![FILE_NAME.to_string()]);
    }

    #[test]
    fn known_title_formats_are_read_from_the_file() {
        let dir = temp_config_dir();
        write_settings(
            dir.path(),
            r#"{"calendarTitle": "full", "dayTitle": "short"}"#,
        );

        let loaded = Settings::peek(dir.path());

        assert_eq!(
            (loaded.calendar_title, loaded.day_title),
            (CalendarTitle::Full, DayTitle::Short)
        );
    }

    #[test]
    fn wrongly_typed_nested_objects_fall_back_to_their_default_while_siblings_survive() {
        let dir = temp_config_dir();
        write_settings(
            dir.path(),
            r#"{"tracker": 5, "capture": "x", "reports": [], "timeline": {"zoom": 2.0}, "uiFontSize": 18}"#,
        );

        let loaded = SettingsStore::open(dir.path()).get();

        assert_eq!(
            loaded,
            settings(|s| {
                s.ui_font_size = 18;
                s.timeline.zoom = 2.0;
            })
        );
        assert_eq!(dir_entries(dir.path()), vec![FILE_NAME.to_string()]);
    }

    #[test]
    fn partial_file_fills_missing_fields_with_defaults() {
        let dir = temp_config_dir();
        write_settings(
            dir.path(),
            r#"{"accent": "teal", "tracker": {"idleMinutes": 9}}"#,
        );

        assert_eq!(
            SettingsStore::open(dir.path()).get(),
            settings(|s| {
                s.accent = "teal".into();
                s.tracker.idle_minutes = 9;
            })
        );
    }

    #[test]
    fn values_in_a_file_are_sanitized_on_open() {
        let dir = temp_config_dir();
        write_settings(dir.path(), r#"{"uiFontSize": 99, "accent": "nope"}"#);

        let loaded = SettingsStore::open(dir.path()).get();

        assert_eq!(loaded.ui_font_size, 24);
        assert_eq!(loaded.accent, "system");
    }

    #[test]
    fn peek_on_a_corrupt_file_returns_defaults_and_leaves_the_file_alone() {
        let dir = temp_config_dir();
        write_settings(dir.path(), "{ not json");

        assert_eq!(Settings::peek(dir.path()), Settings::default());
        assert_eq!(
            std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap(),
            "{ not json"
        );
        assert_eq!(dir_entries(dir.path()), vec![FILE_NAME.to_string()]);
    }

    #[test]
    fn peek_without_a_file_returns_defaults() {
        let dir = temp_config_dir();
        assert_eq!(Settings::peek(dir.path()), Settings::default());
    }

    #[test]
    fn peek_returns_the_stored_settings_sanitized() {
        let dir = temp_config_dir();
        write_settings(dir.path(), r#"{"logLevel": "debug", "uiFontSize": 99}"#);

        assert_eq!(
            Settings::peek(dir.path()),
            settings(|s| {
                s.log_level = LogLevel::Debug;
                s.ui_font_size = 24;
            })
        );
    }

    #[test]
    fn mcp_settings_are_read_in_camel_case() {
        let dir = temp_config_dir();
        write_settings(dir.path(), r#"{"mcp": {"exposeActivity": true}}"#);

        assert!(Settings::peek(dir.path()).mcp.expose_activity);
        assert_eq!(
            serde_json::to_value(Settings::peek(dir.path()).mcp).unwrap(),
            serde_json::json!({"exposeActivity": true})
        );
    }

    #[test]
    fn a_wrongly_typed_mcp_block_falls_back_to_hidden_activity() {
        let dir = temp_config_dir();
        write_settings(dir.path(), r#"{"mcp": {"exposeActivity": "yes"}}"#);

        assert!(!Settings::peek(dir.path()).mcp.expose_activity);
    }

    #[tokio::test]
    async fn update_returns_previous_and_sanitized_saved_and_persists_across_reopen() {
        let dir = temp_config_dir();
        let store = SettingsStore::open(dir.path());

        let (previous, saved) = store
            .update(settings(|s| {
                s.dark_scheme = Scheme::Light;
                s.ui_font_size = 99;
            }))
            .await
            .unwrap();

        assert_eq!(previous, Settings::default());
        assert_eq!(
            saved,
            settings(|s| {
                s.dark_scheme = Scheme::Light;
                s.ui_font_size = 24;
            })
        );
        assert_eq!(store.get(), saved);
        assert_eq!(SettingsStore::open(dir.path()).get(), saved);
    }

    #[tokio::test]
    async fn second_update_sees_the_first_as_previous() {
        let dir = temp_config_dir();
        let store = SettingsStore::open(dir.path());
        let (_, first) = store
            .update(settings(|s| s.accent = "blue".into()))
            .await
            .unwrap();

        let (previous, _) = store.update(Settings::default()).await.unwrap();

        assert_eq!(previous, first);
    }

    #[tokio::test]
    async fn concurrent_app_updates_for_different_apps_both_survive() {
        let dir = temp_config_dir();
        let store = SettingsStore::open(dir.path());

        let (hide, own) = tokio::join!(
            store.update_app("com.apple.Safari".into(), patch(None, Some(true))),
            store.update_app("com.tinyspeck.slackmacgap".into(), patch(Some(true), None)),
        );
        hide.unwrap();
        own.unwrap();

        let expected = HashMap::from([
            (
                "com.apple.Safari".to_string(),
                AppOverride {
                    hidden: true,
                    ..AppOverride::default()
                },
            ),
            (
                "com.tinyspeck.slackmacgap".to_string(),
                AppOverride {
                    own_lane: true,
                    ..AppOverride::default()
                },
            ),
        ]);
        assert_eq!(store.get().apps, expected);
        assert_eq!(SettingsStore::open(dir.path()).get().apps, expected);
    }

    #[tokio::test]
    async fn a_blank_key_in_update_app_is_invalid_input_and_changes_nothing() {
        let dir = temp_config_dir();
        let store = SettingsStore::open(dir.path());
        let (_, before) = store
            .update_app("com.apple.mail".into(), patch(Some(true), None))
            .await
            .unwrap();

        let err = store
            .update_app("  ".into(), patch(None, Some(true)))
            .await
            .unwrap_err();

        assert_eq!(err.kind(), crate::error::ErrorKind::InvalidInput);
        assert_eq!(store.get(), before);
        assert_eq!(SettingsStore::open(dir.path()).get(), before);
    }

    #[tokio::test]
    async fn update_app_returns_the_settings_before_and_after_the_change() {
        let dir = temp_config_dir();
        let store = SettingsStore::open(dir.path());

        let (previous, saved) = store
            .update_app("com.apple.mail".into(), patch(Some(true), None))
            .await
            .unwrap();

        assert_eq!(previous, Settings::default());
        assert_eq!(
            saved,
            settings(|s| {
                s.apps.insert(
                    "com.apple.mail".into(),
                    AppOverride {
                        own_lane: true,
                        ..AppOverride::default()
                    },
                );
            })
        );
        assert_eq!(store.get(), saved);
    }
}
