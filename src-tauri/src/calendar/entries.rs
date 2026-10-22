//! A day's recordings as the Week and Month list them: one entry per recording, in start order.

use serde::Serialize;
use ts_rs::TS;

use crate::day::{Names, TimesheetCard, recording_blocks, timesheet};
use crate::db::recordings::Recording;
use crate::db::time::Ms;

/// The time one Week card height unit stands for.
const CARD_UNIT_MS: u64 = 15 * 60_000;
/// The shortest Week card: a line of title over the foot.
const CARD_MIN_UNITS: u64 = 3;
/// The tallest Week card: six hours and longer.
const CARD_MAX_UNITS: u64 = 24;
/// The shortest Week card that fits the note between the title and the foot.
const NOTE_MIN_UNITS: u32 = 4;

/// A recording on one day: its parts of the day added up, as the timesheet cards are.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct CalendarEntry {
    #[serde(flatten)]
    #[ts(flatten)]
    pub card: TimesheetCard,
    /// The Week card's height, `CARD_MIN_UNITS` to `CARD_MAX_UNITS`, one per started `CARD_UNIT_MS`.
    pub height_units: u32,
    /// Whether the Week card shows the note: it is at least `NOTE_MIN_UNITS` tall.
    pub show_note: bool,
}

/// The Week card height of a recording lasting `duration_ms` that day.
fn height_units(duration_ms: i64) -> u32 {
    let units = duration_ms.max(0).unsigned_abs().div_ceil(CARD_UNIT_MS);
    // At most `CARD_MAX_UNITS`, so it fits.
    units.clamp(CARD_MIN_UNITS, CARD_MAX_UNITS) as u32
}

/// The recordings within `bounds`, one entry each, by their first start that day.
pub fn entries(
    recordings: &[Recording],
    names: &Names,
    bounds: (Ms, Ms),
    now: Ms,
) -> Vec<CalendarEntry> {
    timesheet(&recording_blocks(recordings, names, bounds, now))
        .into_iter()
        .map(|card| {
            let height_units = height_units(card.duration_ms);
            CalendarEntry {
                height_units,
                show_note: height_units >= NOTE_MIN_UNITS,
                card,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::recordings::RecordingSpan;

    const MIN: i64 = 60_000;

    #[test]
    fn height_units_count_started_quarter_hours_between_the_minimum_and_the_cap() {
        for (minutes, expected) in [
            (0, 3),
            (10, 3),
            (45, 3),
            (46, 4),
            (60, 4),
            (360, 24),
            (480, 24),
        ] {
            assert_eq!(height_units(minutes * MIN), expected, "{minutes} min");
        }
    }

    fn recording_of(minutes: i64) -> Recording {
        Recording {
            id: "r".into(),
            project_id: "p1".into(),
            note: "note".into(),
            tag_id: None,
            created_at: 0,
            updated_at: 0,
            spans: vec![RecordingSpan::new(0, Some(minutes * MIN))],
        }
    }

    #[test]
    fn the_week_card_shows_the_note_from_four_height_units_up() {
        for (minutes, expected) in [
            (1, false),
            (30, false),
            (45, false),
            (46, true),
            (60, true),
            (480, true),
        ] {
            let list = entries(
                &[recording_of(minutes)],
                &Names::default(),
                (0, 24 * 60 * MIN),
                24 * 60 * MIN,
            );
            assert_eq!(list.len(), 1, "{minutes} min");
            assert_eq!(list[0].show_note, expected, "{minutes} min");
        }
    }

    #[test]
    fn a_negative_duration_has_the_minimum_height() {
        assert_eq!(height_units(-MIN), 3);
    }
}
