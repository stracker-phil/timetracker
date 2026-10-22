//! What the Day view sends to the page.

use serde::Serialize;
use ts_rs::TS;

use crate::db::time::{Day, Ms};
use crate::rules::types::Suggestion;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct DayView {
    pub day: Day,
    pub bounds: (Ms, Ms),
    /// The summary, activity, app and source lanes, each when shown.
    pub lanes: Vec<Lane>,
    /// The recordings column, by start.
    pub recordings: Vec<RecordingBlock>,
    /// The timesheet: one card per recording on the day, by its first span on the day.
    pub timesheet: Vec<TimesheetCard>,
    /// Sleep, lock and pause spans cut to the day: the continuity barriers for the rules.
    #[serde(skip)]
    #[ts(skip)]
    pub barriers: Vec<(Ms, Ms)>,
    pub totals: DayTotals,
    pub first_activity: Option<Ms>,
    /// Every source of the day's activity, sorted.
    pub sources: Vec<String>,
    /// The timeline's compact density.
    pub compact: bool,
    /// Every app the settings hide, by name.
    pub hidden_apps: Vec<HiddenApp>,
    /// The day's tracked apps that are not hidden, most time first: what can get a lane of its own.
    pub apps: Vec<DayApp>,
    /// What the rules suggest recording, by start; filled by `load`.
    pub suggestions: Vec<Suggestion>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct HiddenApp {
    pub app_key: String,
    /// Its name on this day, else the key.
    pub app: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum LaneKind {
    Summary,
    Activity,
    App,
    Source,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Lane {
    /// `summary`, `activity`, `app:<app key>` or `source:<name>`.
    pub id: String,
    pub kind: LaneKind,
    pub label: String,
    /// The app of an app lane.
    pub app_key: Option<String>,
    /// The bundle id of an app lane's app, for its icon.
    pub bundle_id: Option<String>,
    pub blocks: Vec<ActivityBlock>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ActivityBlock {
    /// `a<first member id>`; `s<start>` in the summary lane.
    pub id: String,
    pub start: Ms,
    pub end: Ms,
    /// The time its activity fills inside it: less than `end - start` when it bridges gaps or, in the
    /// summary lane, when its buckets are not full.
    pub active_ms: i64,
    pub app: String,
    pub bundle_id: Option<String>,
    pub app_key: String,
    /// The window title of its longest member.
    pub title: String,
    /// What the block shows: the document name or the app, by the title setting.
    pub label: String,
    pub url: Option<String>,
    pub path: Option<String>,
    pub source: String,
    /// At least half of it lies inside recordings.
    pub covered: bool,
    /// The activity ids the block stands for.
    pub members: Vec<i64>,
    /// Position among the overlapping blocks of its lane: column `col` of `cols` equal columns.
    pub col: u8,
    pub cols: u8,
    pub private: bool,
}

/// A span of a recording cut to the day; a recording with several spans on the day has several blocks.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct RecordingBlock {
    /// The recording's id.
    pub id: String,
    pub span_id: String,
    /// How many spans the recording has, on any day.
    pub span_count: u32,
    pub start: Ms,
    pub end: Ms,
    pub project_id: String,
    pub project_name: String,
    pub client_name: Option<String>,
    /// The project's accent and shade.
    pub color: String,
    pub shade: u8,
    pub note: String,
    pub tag: Option<String>,
    /// The timer's open span, still recording.
    pub running: bool,
    /// The span starts on an earlier day or ends on a later one.
    pub clipped_start: bool,
    pub clipped_end: bool,
}

/// A recording as the timesheet shows it: the parts of its spans on the day, added up.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct TimesheetCard {
    /// The recording's id.
    pub id: String,
    pub project_id: String,
    pub project_name: String,
    pub client_name: Option<String>,
    pub color: String,
    pub shade: u8,
    pub note: String,
    pub tag: Option<String>,
    /// The day's part of its spans.
    pub duration_ms: i64,
    /// Its first span's start and its last span's end on the day.
    pub start: Ms,
    pub end: Ms,
    /// Its spans on the day, by start.
    pub span_ids: Vec<String>,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct DayApp {
    pub app_key: String,
    pub app: String,
    pub bundle_id: Option<String>,
    /// Its tracked time on the day.
    pub active_ms: i64,
    pub own_lane: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct DayTotals {
    /// The day's part of its recordings' spans.
    pub recorded_ms: i64,
    /// Tracker activity, overlaps counted once, hidden apps and noise included; external sources are not
    /// tracked time.
    pub tracked_ms: i64,
    pub away_ms: i64,
}
