//! Every event the backend emits to the page, with its payload type.

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use ts_rs::TS;

/// No payload; the page then calls `take_launch_events`.
pub const LAUNCH_EVENT: &str = "launch";
/// Payload: a command id.
pub const COMMAND_EVENT: &str = "command";
/// Payload: a `RemoteEvent`.
pub const REMOTE_EVENT: &str = "remote";
/// Payload: a `DataChanged`; views that show the day reload.
pub const DATA_CHANGED_EVENT: &str = "data_changed";

/// A local calendar day, `YYYY-MM-DD`.
#[derive(Serialize, Debug, Clone, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Day(pub String);

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub enum DataKind {
    Activity,
    Recordings,
    Projects,
    Tags,
    Rules,
    Settings,
}

#[derive(Serialize, Debug, Clone, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct DataChanged {
    pub kind: DataKind,
    /// `None` when every day may have changed.
    pub day: Option<Day>,
}

pub fn data_changed(app: &AppHandle, kind: DataKind, day: Option<Day>) {
    if let Err(e) = app.emit(DATA_CHANGED_EVENT, DataChanged { kind, day }) {
        log::warn!("Couldn't notify the page of changed {kind:?}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_changed_serializes_kind_and_day() {
        let event = DataChanged {
            kind: DataKind::Recordings,
            day: Some(Day("2026-10-21".into())),
        };
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            serde_json::json!({ "kind": "recordings", "day": "2026-10-21" })
        );
    }

    #[test]
    fn data_changed_for_every_day_serializes_a_null_day() {
        let event = DataChanged {
            kind: DataKind::Activity,
            day: None,
        };
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            serde_json::json!({ "kind": "activity", "day": null })
        );
    }
}
