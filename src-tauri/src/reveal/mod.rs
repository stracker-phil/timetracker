//! Showing one recording or suggestion selected in its day's Day view: the palette's `r:` rows
//! (`search`) and `timetracker select <id>` (`scripting::writes::locate`).

pub mod search;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::db::time::{Day, Ms};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum EntryKind {
    Recording,
    Suggestion,
}

/// A recording, or a suggestion's first piece, on `day`, by the id the Day view uses.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct EntryRef {
    pub day: Day,
    pub kind: EntryKind,
    pub id: String,
}

/// One palette row: where it is and what it shows.
#[derive(Serialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct FoundEntry {
    pub entry: EntryRef,
    /// `Wed, Nov 11`, the short day title.
    pub day_label: String,
    pub start: Ms,
    /// `None` while it runs.
    pub end: Option<Ms>,
    /// `None` for a suggestion waiting for a project (ADR 0030).
    pub project: Option<String>,
    pub client: Option<String>,
    /// The project's accent.
    pub color: String,
    pub shade: u8,
    pub note: String,
    pub tag: Option<String>,
    /// A suggestion's rules' names or the app that stored it.
    pub by: Option<String>,
}
