//! Fixtures for the Day view tests.

use super::{DayInput, ViewSettings};
use crate::db::activities::ActivityRow;
use crate::db::away::{AwayReason, AwayRow};
use crate::db::time::{Day, Ms};

pub const MIN: Ms = 60_000;
pub const HOUR: Ms = 60 * MIN;
pub const DAY_LENGTH: Ms = 24 * HOUR;
/// After the end of the fixed day, so open spans reach the end of the day by default.
pub const NOW: Ms = 30 * HOUR;

pub fn mins(n: i64) -> Ms {
    n * MIN
}

pub fn day() -> Day {
    Day::parse("2026-01-05").unwrap()
}

/// The bundle id `activity` gives an app: `test.<lowercase name>`.
pub fn bundle_of(app: &str) -> String {
    format!("test.{}", app.to_lowercase())
}

/// A tracker row with the bundle id derived from the app.
pub fn activity(id: i64, start: Ms, end: Option<Ms>, app: &str, title: &str) -> ActivityRow {
    ActivityRow {
        id,
        source: "tracker".into(),
        start,
        end,
        app: app.into(),
        bundle_id: Some(bundle_of(app)),
        title: title.into(),
        url: None,
        path: None,
        private: false,
    }
}

pub fn from_source(row: ActivityRow, source: &str) -> ActivityRow {
    ActivityRow {
        source: source.into(),
        ..row
    }
}

pub fn private(row: ActivityRow) -> ActivityRow {
    ActivityRow {
        private: true,
        ..row
    }
}

pub fn with_url(row: ActivityRow, url: &str) -> ActivityRow {
    ActivityRow {
        url: Some(url.into()),
        ..row
    }
}

pub fn with_path(row: ActivityRow, path: &str) -> ActivityRow {
    ActivityRow {
        path: Some(path.into()),
        ..row
    }
}

pub fn with_bundle_id(row: ActivityRow, bundle_id: Option<&str>) -> ActivityRow {
    ActivityRow {
        bundle_id: bundle_id.map(str::to_string),
        ..row
    }
}

pub fn away(id: i64, start: Ms, end: Option<Ms>, reason: AwayReason) -> AwayRow {
    AwayRow {
        id,
        start,
        end,
        reason,
    }
}

/// Default timeline settings and no app overrides.
pub fn settings() -> ViewSettings {
    ViewSettings::default()
}

pub fn own_lane(mut settings: ViewSettings, bundle_id: &str) -> ViewSettings {
    settings.apps.entry(bundle_id.into()).or_default().own_lane = true;
    settings
}

pub fn hidden(mut settings: ViewSettings, bundle_id: &str) -> ViewSettings {
    settings.apps.entry(bundle_id.into()).or_default().hidden = true;
    settings
}

/// A day of `(0, 24 h)` seen at `NOW`.
pub fn input(activities: Vec<ActivityRow>, away: Vec<AwayRow>) -> DayInput {
    DayInput {
        day: day(),
        bounds: (0, DAY_LENGTH),
        now: NOW,
        activities,
        away,
    }
}
