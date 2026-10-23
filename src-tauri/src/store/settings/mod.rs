mod serde_helpers;
mod store;
mod types;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::keybindings::is_global;
use crate::capture::url::domain;
use crate::error::{AppError, AppResult};
use serde_helpers::lenient;
pub use store::SettingsStore;
pub use types::*;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// The look while macOS is in light mode; part 1's scheme names (`ayu-light`, …) load as the default.
    #[serde(deserialize_with = "lenient")]
    pub light_scheme: Scheme,
    /// The look while macOS is in dark mode.
    #[serde(deserialize_with = "lenient_dark_scheme")]
    pub dark_scheme: Scheme,
    /// The single `scheme` of the appearance before a scheme per mode; read as `light_scheme`, never written.
    #[serde(rename = "scheme", skip_serializing, deserialize_with = "lenient")]
    #[ts(skip)]
    pub(crate) legacy_scheme: Option<Scheme>,
    /// The app accent: `system` (the macOS accent color) or one of `ACCENTS`.
    pub accent: String,
    #[serde(deserialize_with = "lenient")]
    pub text_size: TextSize,
    /// A text size in pixels (`uiFontSize`); read as its nearest `text_size` step, never written.
    #[serde(rename = "uiFontSize", skip_serializing, deserialize_with = "lenient")]
    #[ts(skip)]
    pub(crate) legacy_ui_font_size: Option<u8>,
    #[serde(deserialize_with = "lenient")]
    pub calendar_title: CalendarTitle,
    #[serde(deserialize_with = "lenient")]
    pub day_title: DayTitle,
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
    /// The system-wide shortcut for the timer picker; `None` turns it off.
    pub timer_shortcut: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub mcp: McpSettings,
}

/// ⌃⌥⌘T, which neither macOS nor common IDEs bind.
pub const DEFAULT_TIMER_SHORTCUT: &str = "Mod+Ctrl+Alt+T";

impl Default for Settings {
    fn default() -> Self {
        Self {
            light_scheme: Scheme::Default,
            dark_scheme: Scheme::Dark,
            legacy_scheme: None,
            accent: SYSTEM_ACCENT.into(),
            text_size: TextSize::Medium,
            legacy_ui_font_size: None,
            calendar_title: CalendarTitle::Short,
            day_title: DayTitle::Long,
            launch_at_login: false,
            data_dir: None,
            tracker: TrackerSettings::default(),
            capture: CaptureSettings::default(),
            timeline: TimelineSettings::default(),
            apps: HashMap::new(),
            reports: ReportSettings::default(),
            timer_shortcut: Some(DEFAULT_TIMER_SHORTCUT.into()),
            mcp: McpSettings::default(),
        }
    }
}

impl Settings {
    /// Clamps every value into a usable range, so a hand-edited file can't break the app. Unknown keys are
    /// ignored.
    pub fn sanitized(mut self) -> Self {
        let d = Settings::default();
        if let Some(scheme) = self.legacy_scheme.take() {
            self.light_scheme = scheme;
        }
        if let Some(px) = self.legacy_ui_font_size.take() {
            self.text_size = TextSize::nearest_to_px(px);
        }
        if !is_app_accent(&self.accent) {
            self.accent = d.accent;
        }
        self.data_dir = non_blank(self.data_dir);

        let tracker = &mut self.tracker;
        tracker.idle_minutes = tracker.idle_minutes.clamp(1, 120);
        tracker.away_prompt_minutes = tracker.away_prompt_minutes.clamp(1, 240);

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

        // Drop app entries that hold no override.
        self.apps.retain(|_, app| *app != AppOverride::default());
        if self
            .timer_shortcut
            .as_deref()
            .is_some_and(|b| !is_global(b))
        {
            self.timer_shortcut = d.timer_shortcut;
        }
        self
    }

    /// `patch` applied to the app's override; an override that ends up empty is removed. `key` is the
    /// app's bundle id, or its name when it has none.
    pub fn with_app(mut self, key: &str, patch: &AppSettingPatch) -> AppResult<Settings> {
        let key = key.trim();
        if key.is_empty() {
            return Err(AppError::InvalidInput(
                "An app needs a bundle id or a name".into(),
            ));
        }
        let app = self.apps.entry(key.to_string()).or_default();
        if let Some(own_lane) = patch.own_lane {
            app.own_lane = own_lane;
        }
        if let Some(hidden) = patch.hidden {
            app.hidden = hidden;
        }
        if *app == AppOverride::default() {
            self.apps.remove(key);
        }
        Ok(self)
    }
}

/// `lenient`, but an unknown value falls back to the dark-mode default, `Dark`.
fn lenient_dark_scheme<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Scheme, D::Error> {
    lenient::<D, Option<Scheme>>(deserializer).map(|scheme| scheme.unwrap_or(Scheme::Dark))
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    pub(super) fn settings(edit: impl FnOnce(&mut Settings)) -> Settings {
        let mut settings = Settings::default();
        edit(&mut settings);
        settings
    }

    pub(super) fn patch(own_lane: Option<bool>, hidden: Option<bool>) -> AppSettingPatch {
        AppSettingPatch {
            own_lane,
            hidden,
            ..Default::default()
        }
    }

    fn quoted_strings(text: &str) -> Vec<String> {
        text.split('"')
            .skip(1)
            .step_by(2)
            .map(String::from)
            .collect()
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
    fn tracker_values_are_clamped_to_the_allowed_range() {
        for (input, idle, away) in [
            (0, 1, 1),
            (1, 1, 1),
            (5, 5, 5),
            (120, 120, 120),
            (121, 120, 121),
            (240, 120, 240),
            (241, 120, 240),
            (60_000, 120, 240),
        ] {
            let sanitized = settings(|s| {
                s.tracker.idle_minutes = input;
                s.tracker.away_prompt_minutes = input;
            })
            .sanitized();
            assert_eq!(sanitized.tracker.idle_minutes, idle, "idle {input}");
            assert_eq!(sanitized.tracker.away_prompt_minutes, away, "away {input}");
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
    fn the_default_summary_interval_is_five_minutes() {
        assert_eq!(Settings::default().timeline.summary_minutes, 5);
        assert_eq!(TimelineSettings::default().summary_minutes, 5);
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
    fn an_unknown_accent_resets_to_the_default() {
        let sanitized = settings(|s| s.accent = "chartreuse".into()).sanitized();
        assert_eq!(sanitized.accent, "system");
    }

    #[test]
    fn the_default_accent_follows_the_system() {
        assert_eq!(Settings::default().accent, "system");
    }

    #[test]
    fn accents_survive_sanitizing() {
        for accent in ["system", "orange", "teal"] {
            let sanitized = settings(|s| s.accent = accent.into()).sanitized();
            assert_eq!(sanitized.accent, accent, "{accent}");
        }
    }

    #[test]
    fn known_accent_and_appearance_are_kept() {
        let kept = settings(|s| {
            s.accent = "teal".into();
            s.light_scheme = Scheme::Light;
            s.dark_scheme = Scheme::Default;
        });
        assert_eq!(kept.clone().sanitized(), kept);
    }

    #[test]
    fn a_blank_data_dir_becomes_none_and_others_are_trimmed() {
        for (input, expected) in [
            (None, None),
            (Some(""), None),
            (Some("   "), None),
            (Some(" /Volumes/Data "), Some("/Volumes/Data")),
            (Some("/Users/me/data"), Some("/Users/me/data")),
        ] {
            let value = input.map(String::from);
            let sanitized = settings(|s| s.data_dir = value).sanitized();
            assert_eq!(sanitized.data_dir, expected.map(String::from), "{input:?}");
        }
    }

    #[test]
    fn a_timer_shortcut_that_cannot_be_global_resets_to_the_default() {
        for (input, expected) in [
            ("", DEFAULT_TIMER_SHORTCUT),
            ("  ", DEFAULT_TIMER_SHORTCUT),
            ("T", DEFAULT_TIMER_SHORTCUT),
            ("Shift+T", DEFAULT_TIMER_SHORTCUT),
            ("Alt+R", DEFAULT_TIMER_SHORTCUT),
            ("Mod+", DEFAULT_TIMER_SHORTCUT),
            ("Mod+Alt+R", "Mod+Alt+R"),
            ("Ctrl+Alt+K", "Ctrl+Alt+K"),
        ] {
            let sanitized = settings(|s| s.timer_shortcut = Some(input.into())).sanitized();
            assert_eq!(
                sanitized.timer_shortcut.as_deref(),
                Some(expected),
                "{input:?}"
            );
        }
    }

    #[test]
    fn the_timer_shortcut_defaults_to_control_option_command_t() {
        assert_eq!(DEFAULT_TIMER_SHORTCUT, "Mod+Ctrl+Alt+T");
        assert_eq!(
            Settings::default().timer_shortcut.as_deref(),
            Some(DEFAULT_TIMER_SHORTCUT)
        );
        assert_eq!(
            Settings::default().sanitized().timer_shortcut.as_deref(),
            Some(DEFAULT_TIMER_SHORTCUT)
        );
    }

    #[test]
    fn a_null_timer_shortcut_loads_as_off_and_survives_sanitizing() {
        let loaded: Settings = serde_json::from_value(json!({ "timerShortcut": null })).unwrap();

        assert_eq!(loaded.timer_shortcut, None);
        assert_eq!(loaded.sanitized().timer_shortcut, None);
    }

    #[test]
    fn a_missing_timer_shortcut_gets_the_default() {
        let loaded: Settings = serde_json::from_value(json!({ "accent": "teal" })).unwrap();

        assert_eq!(
            loaded.timer_shortcut.as_deref(),
            Some(DEFAULT_TIMER_SHORTCUT)
        );
    }

    #[test]
    fn a_turned_off_timer_shortcut_round_trips_as_null() {
        let original = settings(|s| s.timer_shortcut = None);

        let value = serde_json::to_value(&original).unwrap();

        assert_eq!(value["timerShortcut"], json!(null));
        let back: Settings = serde_json::from_value(value).unwrap();
        assert_eq!(back, original);
    }

    #[test]
    fn schemes_round_trip_as_lowercase_camel_case_json() {
        for (light, dark, light_json, dark_json) in [
            (Scheme::Default, Scheme::Light, "default", "light"),
            (Scheme::Light, Scheme::Dark, "light", "dark"),
            (Scheme::Dark, Scheme::Default, "dark", "default"),
        ] {
            let original = settings(|s| {
                s.light_scheme = light;
                s.dark_scheme = dark;
            });

            let value = serde_json::to_value(&original).unwrap();

            assert_eq!(value["lightScheme"], json!(light_json));
            assert_eq!(value["darkScheme"], json!(dark_json));
            let back: Settings = serde_json::from_value(value).unwrap();
            assert_eq!(back, original, "{light_json}/{dark_json}");
        }
    }

    #[test]
    fn titles_default_to_a_short_calendar_month_and_a_long_day() {
        let defaults = Settings::default();

        assert_eq!(
            (defaults.calendar_title, defaults.day_title),
            (CalendarTitle::Short, DayTitle::Long)
        );
    }

    #[test]
    fn titles_round_trip_as_lowercase_json() {
        for (calendar, day, calendar_json, day_json) in [
            (CalendarTitle::Short, DayTitle::Long, "short", "long"),
            (CalendarTitle::Full, DayTitle::Short, "full", "short"),
        ] {
            let original = settings(|s| {
                s.calendar_title = calendar;
                s.day_title = day;
            });

            let value = serde_json::to_value(&original).unwrap();

            assert_eq!(value["calendarTitle"], json!(calendar_json));
            assert_eq!(value["dayTitle"], json!(day_json));
            let back: Settings = serde_json::from_value(value).unwrap();
            assert_eq!(back, original, "{calendar_json}/{day_json}");
        }
    }

    #[test]
    fn text_size_defaults_to_medium_and_round_trips_as_lowercase_json() {
        assert_eq!(Settings::default().text_size, TextSize::Medium);
        for (size, json_value) in [
            (TextSize::Small, "small"),
            (TextSize::Medium, "medium"),
            (TextSize::Large, "large"),
        ] {
            let original = settings(|s| s.text_size = size);

            let value = serde_json::to_value(&original).unwrap();

            assert_eq!(value["textSize"], json!(json_value));
            let back: Settings = serde_json::from_value(value).unwrap();
            assert_eq!(back, original, "{json_value}");
        }
    }

    #[test]
    fn the_legacy_font_size_is_never_serialized() {
        let value = serde_json::to_value(settings(|s| s.legacy_ui_font_size = Some(16))).unwrap();

        assert!(value.get("uiFontSize").is_none());
    }

    #[test]
    fn mcp_does_not_expose_activity_by_default() {
        assert!(!Settings::default().mcp.expose_activity);
        assert!(!McpSettings::default().expose_activity);
    }

    #[test]
    fn default_settings_serialize_with_camel_case_keys() {
        assert_eq!(
            serde_json::to_value(Settings::default()).unwrap(),
            json!({
                "lightScheme": "default",
                "darkScheme": "dark",
                "accent": "system",
                "textSize": "medium",
                "calendarTitle": "short",
                "dayTitle": "long",
                "launchAtLogin": false,
                "dataDir": null,
                "tracker": {
                    "idleMinutes": 5,
                    "awayPromptEnabled": true,
                    "awayPromptMinutes": 10
                },
                "capture": {
                    "excludedApps": [],
                    "excludedDomains": [],
                    "captureUrls": true,
                    "captureDocuments": true
                },
                "timeline": {
                    "summaryLane": true,
                    "summaryMinutes": 5,
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
                "timerShortcut": "Mod+Ctrl+Alt+T",
                "mcp": { "exposeActivity": false }
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

    #[test]
    fn accents_match_the_frontend_list() {
        let types_ts = include_str!("../../../../src/api/types.ts");

        let accents_line = types_ts
            .lines()
            .find(|line| line.starts_with("export const ACCENTS = ["))
            .expect("src/api/types.ts should declare `export const ACCENTS = [...]` on one line");
        assert_eq!(quoted_strings(accents_line), ACCENTS, "ACCENTS in types.ts");
    }
}
