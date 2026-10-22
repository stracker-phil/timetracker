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
        .map(|card| CalendarEntry {
            height_units: height_units(card.duration_ms),
            card,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn a_negative_duration_has_the_minimum_height() {
        assert_eq!(height_units(-MIN), 3);
    }
}
