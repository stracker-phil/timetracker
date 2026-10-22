use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::serde_helpers::{lenient, present};

/// A look for one macOS mode: `Default` has a dark sidebar beside light content, `Light` is all light,
/// `Dark` is all dark.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum Scheme {
    #[default]
    Default,
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

/// Accent hues; every palette defines each one. Projects take their color from this list.
pub const ACCENTS: &[&str] = &[
    "orange", "gold", "red", "magenta", "purple", "indigo", "blue", "teal", "graphite",
];

pub fn is_accent(name: &str) -> bool {
    ACCENTS.contains(&name)
}

/// The app accent that follows the macOS accent color; projects never take it.
pub const SYSTEM_ACCENT: &str = "system";

/// Whether `name` can be the app accent: the system's or one of `ACCENTS`.
pub fn is_app_accent(name: &str) -> bool {
    name == SYSTEM_ACCENT || is_accent(name)
}

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

#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default, clap::ValueEnum, TS,
)]
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
            summary_minutes: 5,
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

/// Per-app overrides of the timeline settings, keyed by bundle id (the app's name without one).
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

/// The fields of one app's `AppOverride` to change; absent ones stay, `null` returns an override to the
/// timeline's own setting.
#[derive(Deserialize, Clone, Debug, Default, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct AppSettingPatch {
    #[serde(default, deserialize_with = "present")]
    #[ts(optional)]
    pub group_by_app: Option<Option<bool>>,
    #[serde(default, deserialize_with = "present")]
    #[ts(optional)]
    pub titles: Option<Option<Titles>>,
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

/// What `timetracker mcp` may read besides recordings, projects, tags and reports.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(default, rename_all = "camelCase")]
pub struct McpSettings {
    /// Window titles, URLs and documents of the captured activity.
    pub expose_activity: bool,
}
