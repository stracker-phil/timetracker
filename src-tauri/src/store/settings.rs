use std::collections::HashMap;
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

use super::json_store::JsonStore;
use crate::capture::url::domain;
use crate::error::{AppError, AppResult};

/// Falls back to the default for a value the field doesn't know (a typo, a retired variant),
/// so one bad field doesn't quarantine the whole file and lose every other setting.
fn lenient<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned + Default,
{
    let raw = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(raw.clone()).unwrap_or_else(|_| {
        log::warn!("Ignoring unknown setting value {raw}; using the default");
        T::default()
    }))
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default, clap::ValueEnum, TS,
)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl From<LogLevel> for log::LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Trace => log::LevelFilter::Trace,
        }
    }
}

/// Accent hues; every color scheme defines each one. Projects take their color from this list.
pub const ACCENTS: &[&str] = &[
    "orange", "gold", "red", "magenta", "purple", "indigo", "blue", "teal", "graphite",
];

pub fn is_accent(name: &str) -> bool {
    ACCENTS.contains(&name)
}

/// Built-in color schemes (`src/styles/schemes/<id>.css`).
pub const SCHEMES: &[&str] = &[
    "ayu-mirage",
    "monokai",
    "nord",
    "solarized-dark",
    "ayu-light",
    "catppuccin-latte",
    "github-light",
    "solarized-light",
];

/// Lengths of the summary lane's buckets, in minutes.
pub const SUMMARY_MINUTES: &[u16] = &[5, 10, 15, 20, 30, 45, 60];
/// Grid that dragged recordings snap to, in minutes.
pub const SNAP_MINUTES: &[u8] = &[1, 5, 10, 15];

/// How long a gap may be before two activity blocks of the same kind stop merging.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum Continuity {
    Off,
    #[default]
    Short,
    Regular,
    Extended,
}

/// How short a block may be before the timeline hides it as noise.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum Noise {
    Off,
    #[default]
    Low,
    Medium,
    High,
}

/// What an activity block is labeled with.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum Titles {
    #[default]
    Document,
    App,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub enum Rounding {
    #[default]
    None,
    Nearest5,
    Nearest6,
    Nearest10,
    Nearest15,
    Nearest30,
    Up5,
    Up6,
    Up10,
    Up15,
    Up30,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum Group {
    #[default]
    Project,
    Tag,
    Day,
    Week,
    Month,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(default, rename_all = "camelCase")]
pub struct TrackerSettings {
    /// Minutes without input before the tracker counts the time as away.
    pub idle_minutes: u16,
    pub away_prompt_enabled: bool,
    /// Away spans at least this long trigger the away prompt.
    pub away_prompt_minutes: u16,
    pub poll_seconds: u8,
}

impl Default for TrackerSettings {
    fn default() -> Self {
        Self {
            idle_minutes: 5,
            away_prompt_enabled: true,
            away_prompt_minutes: 10,
            poll_seconds: 2,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(default, rename_all = "camelCase")]
pub struct CaptureSettings {
    /// Bundle ids the tracker never records.
    pub excluded_apps: Vec<String>,
    pub excluded_domains: Vec<String>,
    pub capture_urls: bool,
    pub capture_documents: bool,
}

impl Default for CaptureSettings {
    fn default() -> Self {
        Self {
            excluded_apps: Vec::new(),
            excluded_domains: Vec::new(),
            capture_urls: true,
            capture_documents: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(default, rename_all = "camelCase")]
pub struct TimelineSettings {
    pub summary_lane: bool,
    /// One of `SUMMARY_MINUTES`.
    pub summary_minutes: u16,
    pub show_all_activity: bool,
    pub compact: bool,
    pub group_by_app: bool,
    #[serde(deserialize_with = "lenient")]
    pub continuity: Continuity,
    #[serde(deserialize_with = "lenient")]
    pub noise: Noise,
    #[serde(deserialize_with = "lenient")]
    pub titles: Titles,
    /// One of `SNAP_MINUTES`.
    pub snap_minutes: u8,
    pub zoom: f32,
}

impl Default for TimelineSettings {
    fn default() -> Self {
        Self {
            summary_lane: true,
            summary_minutes: 15,
            show_all_activity: true,
            compact: false,
            group_by_app: true,
            continuity: Continuity::Short,
            noise: Noise::Low,
            titles: Titles::Document,
            snap_minutes: 5,
            zoom: 1.0,
        }
    }
}

/// Per-app overrides of the timeline settings, keyed by bundle id.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(default, rename_all = "camelCase")]
pub struct AppOverride {
    pub group_by_app: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    pub titles: Option<Titles>,
    pub hidden: bool,
    pub own_lane: bool,
}

/// The fields of one app's `AppOverride` to change; absent ones stay.
#[derive(Deserialize, Clone, Debug, Default, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct AppSettingPatch {
    #[ts(optional)]
    pub own_lane: Option<bool>,
    #[ts(optional)]
    pub hidden: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(default, rename_all = "camelCase")]
pub struct ReportSettings {
    #[serde(deserialize_with = "lenient")]
    pub rounding: Rounding,
    #[serde(deserialize_with = "lenient")]
    pub default_group: Group,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Which of the two schemes applies: light, dark, or the OS appearance.
    #[serde(deserialize_with = "lenient")]
    pub theme: Theme,
    #[serde(deserialize_with = "lenient")]
    pub light_scheme: String,
    #[serde(deserialize_with = "lenient")]
    pub dark_scheme: String,
    pub accent: String,
    /// Chrome: sidebar, labels, buttons (also scales rem).
    pub ui_font_size: u8,
    /// Data: times, durations, titles on the timeline.
    pub data_font_size: u8,
    /// An installed font family; `None` uses the bundled Geist.
    #[serde(deserialize_with = "lenient")]
    pub font_ui: Option<String>,
    /// An installed font family; `None` uses the bundled JetBrains Mono.
    #[serde(deserialize_with = "lenient")]
    pub font_mono: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub log_level: LogLevel,
    pub launch_at_login: bool,
    /// The data folder when moved away from the config dir.
    #[serde(deserialize_with = "lenient")]
    pub data_dir: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub tracker: TrackerSettings,
    #[serde(deserialize_with = "lenient")]
    pub capture: CaptureSettings,
    #[serde(deserialize_with = "lenient")]
    pub timeline: TimelineSettings,
    #[serde(deserialize_with = "lenient")]
    pub apps: HashMap<String, AppOverride>,
    #[serde(deserialize_with = "lenient")]
    pub reports: ReportSettings,
    pub timer_shortcut: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            light_scheme: "ayu-light".into(),
            dark_scheme: "ayu-mirage".into(),
            accent: "orange".into(),
            ui_font_size: 13,
            data_font_size: 12,
            font_ui: None,
            font_mono: None,
            log_level: LogLevel::Info,
            launch_at_login: false,
            data_dir: None,
            tracker: TrackerSettings::default(),
            capture: CaptureSettings::default(),
            timeline: TimelineSettings::default(),
            apps: HashMap::new(),
            reports: ReportSettings::default(),
            timer_shortcut: "Mod+Shift+T".into(),
        }
    }
}

impl Settings {
    /// Clamps every value into a usable range, so a hand-edited file can't break the app.
    pub fn sanitized(mut self) -> Self {
        let d = Settings::default();
        if !is_accent(&self.accent) {
            self.accent = d.accent;
        }
        if !SCHEMES.contains(&self.light_scheme.as_str()) {
            self.light_scheme = d.light_scheme;
        }
        if !SCHEMES.contains(&self.dark_scheme.as_str()) {
            self.dark_scheme = d.dark_scheme;
        }
        self.font_ui = non_blank(self.font_ui);
        self.font_mono = non_blank(self.font_mono);
        self.data_dir = non_blank(self.data_dir);
        self.ui_font_size = self.ui_font_size.clamp(10, 24);
        self.data_font_size = self.data_font_size.clamp(9, 28);

        let tracker = &mut self.tracker;
        tracker.idle_minutes = tracker.idle_minutes.clamp(1, 120);
        tracker.away_prompt_minutes = tracker.away_prompt_minutes.clamp(1, 240);
        tracker.poll_seconds = tracker.poll_seconds.clamp(1, 10);

        let capture = &mut self.capture;
        capture.excluded_apps = clean_list(std::mem::take(&mut capture.excluded_apps), Some);
        capture.excluded_domains =
            clean_list(std::mem::take(&mut capture.excluded_domains), domain_entry);

        let timeline = &mut self.timeline;
        timeline.summary_minutes = nearest(SUMMARY_MINUTES, timeline.summary_minutes);
        timeline.snap_minutes = nearest(SNAP_MINUTES, timeline.snap_minutes);
        timeline.zoom = if timeline.zoom.is_finite() {
            timeline.zoom.clamp(0.5, 4.0)
        } else {
            d.timeline.zoom
        };

        if self.timer_shortcut.trim().is_empty() {
            self.timer_shortcut = d.timer_shortcut;
        }
        self
    }

    /// `patch` applied to the app's override; an override that ends up empty is removed.
    pub fn with_app(mut self, bundle_id: &str, patch: &AppSettingPatch) -> AppResult<Settings> {
        let bundle_id = bundle_id.trim();
        if bundle_id.is_empty() {
            return Err(AppError::InvalidInput("An app needs a bundle id".into()));
        }
        let app = self.apps.entry(bundle_id.to_string()).or_default();
        if let Some(own_lane) = patch.own_lane {
            app.own_lane = own_lane;
        }
        if let Some(hidden) = patch.hidden {
            app.hidden = hidden;
        }
        if *app == AppOverride::default() {
            self.apps.remove(bundle_id);
        }
        Ok(self)
    }

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

/// Trimmed, lower case, without blanks and repeats; `shape` may rewrite or drop an entry.
fn clean_list(entries: Vec<String>, shape: impl Fn(String) -> Option<String>) -> Vec<String> {
    let mut clean: Vec<String> = Vec::with_capacity(entries.len());
    for entry in entries {
        let Some(entry) = shape(entry.trim().to_lowercase()).filter(|e| !e.is_empty()) else {
            continue;
        };
        if !clean.contains(&entry) {
            clean.push(entry);
        }
    }
    clean
}

/// A pasted address becomes its host; `www.` goes, as it does for the domains it is matched against.
fn domain_entry(entry: String) -> Option<String> {
    let address = if entry.contains("://") {
        entry
    } else {
        format!("http://{entry}")
    };
    domain(&address)
}

fn non_blank(value: Option<String>) -> Option<String> {
    let trimmed = value?.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// The allowed value closest to `value`; the smaller one on a tie.
fn nearest<T: Copy + Into<i64>>(allowed: &[T], value: T) -> T {
    let distance = |a: &T| ((*a).into() - value.into()).abs();
    *allowed
        .iter()
        .min_by_key(|a| distance(a))
        .expect("allowed values are not empty")
}

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_config_dir;
    use serde_json::json;

    fn settings(edit: impl FnOnce(&mut Settings)) -> Settings {
        let mut settings = Settings::default();
        edit(&mut settings);
        settings
    }

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

    fn patch(own_lane: Option<bool>, hidden: Option<bool>) -> AppSettingPatch {
        AppSettingPatch { own_lane, hidden }
    }

    #[test]
    fn with_app_sets_the_fields_of_the_patch() {
        let updated = Settings::default()
            .with_app("com.apple.mail", &patch(Some(true), Some(true)))
            .unwrap();

        assert_eq!(
            updated.apps,
            HashMap::from([(
                "com.apple.mail".to_string(),
                AppOverride {
                    own_lane: true,
                    hidden: true,
                    ..AppOverride::default()
                }
            )])
        );
    }

    #[test]
    fn with_app_keeps_the_fields_the_patch_leaves_out() {
        let start = settings(|s| {
            s.apps.insert(
                "com.apple.mail".into(),
                AppOverride {
                    own_lane: true,
                    group_by_app: Some(false),
                    ..AppOverride::default()
                },
            );
        });

        let updated = start
            .with_app("com.apple.mail", &patch(None, Some(true)))
            .unwrap();

        assert_eq!(
            updated.apps["com.apple.mail"],
            AppOverride {
                own_lane: true,
                hidden: true,
                group_by_app: Some(false),
                titles: None
            }
        );
    }

    #[test]
    fn with_app_trims_the_bundle_id() {
        let updated = Settings::default()
            .with_app("  com.apple.mail ", &patch(Some(true), None))
            .unwrap();

        assert_eq!(updated.apps.keys().collect::<Vec<_>>(), ["com.apple.mail"]);
    }

    #[test]
    fn with_app_rejects_a_blank_bundle_id() {
        for bundle_id in ["", "   "] {
            let err = Settings::default()
                .with_app(bundle_id, &patch(Some(true), None))
                .unwrap_err();
            assert_eq!(
                err.kind(),
                crate::error::ErrorKind::InvalidInput,
                "{bundle_id:?}"
            );
        }
    }

    #[test]
    fn with_app_removes_an_override_that_becomes_the_default() {
        let start = Settings::default()
            .with_app("com.apple.mail", &patch(Some(true), Some(true)))
            .unwrap();

        let half = start
            .clone()
            .with_app("com.apple.mail", &patch(Some(false), None))
            .unwrap();
        let none = half
            .clone()
            .with_app("com.apple.mail", &patch(None, Some(false)))
            .unwrap();

        assert!(half.apps.contains_key("com.apple.mail"));
        assert!(none.apps.is_empty());
    }

    #[test]
    fn with_app_does_not_create_an_entry_for_an_empty_patch() {
        let updated = Settings::default()
            .with_app("com.apple.mail", &patch(None, None))
            .unwrap();

        assert!(updated.apps.is_empty());
    }

    #[test]
    fn with_app_leaves_other_apps_and_settings_untouched() {
        let start = Settings::default()
            .with_app("com.apple.notes", &patch(None, Some(true)))
            .unwrap();

        let updated = start
            .clone()
            .with_app("com.apple.mail", &patch(Some(true), None))
            .unwrap();

        assert_eq!(
            updated.apps["com.apple.notes"],
            start.apps["com.apple.notes"]
        );
        assert_eq!(updated.apps.len(), 2);
        assert_eq!(
            Settings {
                apps: HashMap::new(),
                ..updated
            },
            Settings::default()
        );
    }

    #[test]
    fn excluded_apps_are_trimmed_lowercased_deduplicated_and_blanks_dropped() {
        let sanitized = settings(|s| {
            s.capture.excluded_apps =
                [" Com.Apple.Mail ", "", "   ", "com.apple.mail", "Test.Code"]
                    .map(String::from)
                    .to_vec();
        })
        .sanitized();

        assert_eq!(
            sanitized.capture.excluded_apps,
            ["com.apple.mail", "test.code"]
        );
    }

    #[test]
    fn excluded_domains_are_cleaned_and_keep_their_first_order() {
        let sanitized = settings(|s| {
            s.capture.excluded_domains = [
                " Bank.Example.com ",
                "",
                "a.com",
                "BANK.example.com",
                "www.a.com",
            ]
            .map(String::from)
            .to_vec();
        })
        .sanitized();

        assert_eq!(
            sanitized.capture.excluded_domains,
            ["bank.example.com", "a.com"]
        );
    }

    #[test]
    fn a_pasted_url_in_excluded_domains_becomes_its_host() {
        for (entry, expected) in [
            ("https://www.Bank.example.com/login", "bank.example.com"),
            ("http://bank.example.com:8080/x?y=1", "bank.example.com"),
            ("www.bank.example.com", "bank.example.com"),
        ] {
            let sanitized =
                settings(|s| s.capture.excluded_domains = vec![entry.into()]).sanitized();
            assert_eq!(sanitized.capture.excluded_domains, [expected], "{entry}");
        }
    }

    #[test]
    fn defaults_are_already_sanitized() {
        assert_eq!(Settings::default().sanitized(), Settings::default());
    }

    #[test]
    fn font_sizes_are_clamped_to_the_allowed_range() {
        for (input, ui, data) in [
            (0, 10, 9),
            (9, 10, 9),
            (10, 10, 10),
            (17, 17, 17),
            (24, 24, 24),
            (28, 24, 28),
            (29, 24, 28),
            (255, 24, 28),
        ] {
            let sanitized = settings(|s| {
                s.ui_font_size = input;
                s.data_font_size = input;
            })
            .sanitized();
            assert_eq!(sanitized.ui_font_size, ui, "ui {input}");
            assert_eq!(sanitized.data_font_size, data, "data {input}");
        }
    }

    #[test]
    fn tracker_values_are_clamped_to_the_allowed_range() {
        for (input, idle, away, poll) in [
            (0, 1, 1, 1),
            (1, 1, 1, 1),
            (5, 5, 5, 5),
            (10, 10, 10, 10),
            (11, 11, 11, 10),
            (120, 120, 120, 10),
            (121, 120, 121, 10),
            (240, 120, 240, 10),
            (241, 120, 240, 10),
            (60_000, 120, 240, 10),
        ] {
            let sanitized = settings(|s| {
                s.tracker.idle_minutes = input;
                s.tracker.away_prompt_minutes = input;
                s.tracker.poll_seconds = input.min(255) as u8;
            })
            .sanitized();
            assert_eq!(sanitized.tracker.idle_minutes, idle, "idle {input}");
            assert_eq!(sanitized.tracker.away_prompt_minutes, away, "away {input}");
            assert_eq!(sanitized.tracker.poll_seconds, poll, "poll {input}");
        }
    }

    #[test]
    fn summary_minutes_snap_to_the_nearest_allowed_value_and_ties_go_down() {
        for (input, expected) in [
            (0, 5),
            (5, 5),
            (7, 5),
            (8, 10),
            (14, 15),
            (25, 20),
            (26, 30),
            (37, 30),
            (40, 45),
            (53, 60),
            (60, 60),
            (1000, 60),
        ] {
            let sanitized = settings(|s| s.timeline.summary_minutes = input).sanitized();
            assert_eq!(sanitized.timeline.summary_minutes, expected, "{input}");
        }
    }

    #[test]
    fn snap_minutes_snap_to_the_nearest_allowed_value_and_ties_go_down() {
        for (input, expected) in [
            (0, 1),
            (1, 1),
            (3, 1),
            (4, 5),
            (7, 5),
            (8, 10),
            (12, 10),
            (13, 15),
            (255, 15),
        ] {
            let sanitized = settings(|s| s.timeline.snap_minutes = input).sanitized();
            assert_eq!(sanitized.timeline.snap_minutes, expected, "{input}");
        }
    }

    #[test]
    fn timeline_zoom_is_clamped_and_non_finite_values_reset_to_one() {
        for (input, expected) in [
            (0.1, 0.5),
            (0.5, 0.5),
            (2.0, 2.0),
            (4.0, 4.0),
            (9.0, 4.0),
            (f32::NAN, 1.0),
            (f32::INFINITY, 1.0),
            (f32::NEG_INFINITY, 1.0),
        ] {
            let sanitized = settings(|s| s.timeline.zoom = input).sanitized();
            assert_eq!(sanitized.timeline.zoom, expected, "{input}");
        }
    }

    #[test]
    fn unknown_accent_and_schemes_reset_to_the_defaults() {
        let sanitized = settings(|s| {
            s.accent = "chartreuse".into();
            s.light_scheme = "nope".into();
            s.dark_scheme = String::new();
        })
        .sanitized();
        assert_eq!(sanitized.accent, "orange");
        assert_eq!(sanitized.light_scheme, "ayu-light");
        assert_eq!(sanitized.dark_scheme, "ayu-mirage");
    }

    #[test]
    fn known_accent_and_schemes_are_kept() {
        let kept = settings(|s| {
            s.accent = "teal".into();
            s.light_scheme = "github-light".into();
            s.dark_scheme = "nord".into();
        });
        assert_eq!(kept.clone().sanitized(), kept);
    }

    #[test]
    fn blank_font_names_and_data_dir_become_none_and_others_are_trimmed() {
        for (input, expected) in [
            (None, None),
            (Some(""), None),
            (Some("   "), None),
            (Some(" Fira Code "), Some("Fira Code")),
            (Some("/Users/me/data"), Some("/Users/me/data")),
        ] {
            let value = input.map(String::from);
            let sanitized = settings(|s| {
                s.font_ui = value.clone();
                s.font_mono = value.clone();
                s.data_dir = value.clone();
            })
            .sanitized();
            let expected = expected.map(String::from);
            assert_eq!(sanitized.font_ui, expected, "{input:?}");
            assert_eq!(sanitized.font_mono, expected, "{input:?}");
            assert_eq!(sanitized.data_dir, expected, "{input:?}");
        }
    }

    #[test]
    fn blank_timer_shortcut_resets_to_the_default() {
        for (input, expected) in [
            ("", "Mod+Shift+T"),
            ("  ", "Mod+Shift+T"),
            ("Mod+Alt+R", "Mod+Alt+R"),
        ] {
            let sanitized = settings(|s| s.timer_shortcut = input.into()).sanitized();
            assert_eq!(sanitized.timer_shortcut, expected, "{input:?}");
        }
    }

    #[test]
    fn unknown_enum_values_fall_back_to_defaults_without_quarantining_the_file() {
        let dir = temp_config_dir();
        let raw = r#"{
            "theme": "neon",
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
        assert_eq!(loaded.accent, "orange");
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

    #[tokio::test]
    async fn update_returns_previous_and_sanitized_saved_and_persists_across_reopen() {
        let dir = temp_config_dir();
        let store = SettingsStore::open(dir.path());

        let (previous, saved) = store
            .update(settings(|s| {
                s.theme = Theme::Dark;
                s.ui_font_size = 99;
            }))
            .await
            .unwrap();

        assert_eq!(previous, Settings::default());
        assert_eq!(
            saved,
            settings(|s| {
                s.theme = Theme::Dark;
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

    #[test]
    fn default_settings_serialize_with_camel_case_keys() {
        assert_eq!(
            serde_json::to_value(Settings::default()).unwrap(),
            json!({
                "theme": "system",
                "lightScheme": "ayu-light",
                "darkScheme": "ayu-mirage",
                "accent": "orange",
                "uiFontSize": 13,
                "dataFontSize": 12,
                "fontUi": null,
                "fontMono": null,
                "logLevel": "info",
                "launchAtLogin": false,
                "dataDir": null,
                "tracker": {
                    "idleMinutes": 5,
                    "awayPromptEnabled": true,
                    "awayPromptMinutes": 10,
                    "pollSeconds": 2
                },
                "capture": {
                    "excludedApps": [],
                    "excludedDomains": [],
                    "captureUrls": true,
                    "captureDocuments": true
                },
                "timeline": {
                    "summaryLane": true,
                    "summaryMinutes": 15,
                    "showAllActivity": true,
                    "compact": false,
                    "groupByApp": true,
                    "continuity": "short",
                    "noise": "low",
                    "titles": "document",
                    "snapMinutes": 5,
                    "zoom": 1.0
                },
                "apps": {},
                "reports": { "rounding": "none", "defaultGroup": "project" },
                "timerShortcut": "Mod+Shift+T"
            })
        );
    }

    #[test]
    fn report_settings_serialize_rounding_in_camel_case() {
        let reports = ReportSettings {
            rounding: Rounding::Nearest15,
            default_group: Group::Week,
        };
        assert_eq!(
            serde_json::to_value(reports).unwrap(),
            json!({ "rounding": "nearest15", "defaultGroup": "week" })
        );
    }

    fn quoted_strings(text: &str) -> Vec<String> {
        text.split('"')
            .skip(1)
            .step_by(2)
            .map(String::from)
            .collect()
    }

    #[test]
    fn accents_and_schemes_match_the_frontend_lists() {
        let types_ts = include_str!("../../../src/api/types.ts");

        let accents_line = types_ts
            .lines()
            .find(|line| line.starts_with("export const ACCENTS = ["))
            .expect("src/api/types.ts should declare `export const ACCENTS = [...]` on one line");
        assert_eq!(quoted_strings(accents_line), ACCENTS, "ACCENTS in types.ts");

        let schemes_block = types_ts
            .split("export const SCHEMES = [")
            .nth(1)
            .and_then(|rest| rest.split("] as const").next())
            .expect("src/api/types.ts should declare `export const SCHEMES = [...] as const`");
        let scheme_ids: Vec<String> = schemes_block
            .split("id: \"")
            .skip(1)
            .map(|rest| rest.split('"').next().unwrap().to_string())
            .collect();
        assert_eq!(scheme_ids, SCHEMES, "SCHEMES ids in types.ts");
    }
}
