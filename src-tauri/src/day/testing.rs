//! Fixtures for the Day view tests.

use super::{DayInput, DayView, Lane, Names, ProjectRef, ViewSettings, view};
use crate::db::activities::ActivityRow;
use crate::db::away::{AwayReason, AwayRow};
use crate::db::recordings::Recording;
use crate::db::time::{Day, Ms};
use crate::store::settings::{AppOverride, Continuity, Noise, TimelineSettings};

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

/// Tracker rows from `(start_min, end_min, app, title)` tuples; ids 1..n.
pub fn rows(spec: &[(i64, i64, &str, &str)]) -> Vec<ActivityRow> {
    spec.iter()
        .enumerate()
        .map(|(i, &(start, end, app, title))| {
            activity(i as i64 + 1, mins(start), Some(mins(end)), app, title)
        })
        .collect()
}

/// The view of a day holding only `activities`.
pub fn render(activities: Vec<ActivityRow>, settings: &ViewSettings) -> DayView {
    view(&input(activities, Vec::new()), settings)
}

pub fn lane_by_id<'a>(view: &'a DayView, id: &str) -> &'a Lane {
    view.lanes
        .iter()
        .find(|l| l.id == id)
        .unwrap_or_else(|| panic!("no lane {id}"))
}

pub fn lane_id_list(view: &DayView) -> Vec<&str> {
    view.lanes.iter().map(|l| l.id.as_str()).collect()
}

/// `(start_min, end_min, members)` of each block of a lane, in lane order.
pub fn shape(lane: &Lane) -> Vec<(i64, i64, Vec<i64>)> {
    lane.blocks
        .iter()
        .map(|b| (b.start / MIN, b.end / MIN, b.members.clone()))
        .collect()
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

/// Raw timeline settings: no summary lane, no grouping by app, no bridging, no noise filter; everything
/// else at its default, and no app overrides.
pub fn settings() -> ViewSettings {
    with_timeline(ViewSettings::default(), |t| {
        t.summary_lane = false;
        t.group_by_app = false;
        t.continuity = Continuity::Off;
        t.noise = Noise::Off;
    })
}

pub fn with_timeline(
    mut settings: ViewSettings,
    edit: impl FnOnce(&mut TimelineSettings),
) -> ViewSettings {
    edit(&mut settings.timeline);
    settings
}

/// Edits the override of `key`, creating it first.
pub fn with_override(
    mut settings: ViewSettings,
    key: &str,
    edit: impl FnOnce(&mut AppOverride),
) -> ViewSettings {
    edit(settings.apps.entry(key.into()).or_default());
    settings
}

pub fn own_lane(settings: ViewSettings, key: &str) -> ViewSettings {
    with_override(settings, key, |a| a.own_lane = true)
}

pub fn hidden(settings: ViewSettings, key: &str) -> ViewSettings {
    with_override(settings, key, |a| a.hidden = true)
}

/// A day of `(0, 24 h)` seen at `NOW`, without recordings.
pub fn input(activities: Vec<ActivityRow>, away: Vec<AwayRow>) -> DayInput {
    DayInput {
        day: day(),
        bounds: (0, DAY_LENGTH),
        now: NOW,
        activities,
        away,
        recordings: Vec::new(),
        names: Names::default(),
    }
}

/// A day with only recordings.
pub fn input_with_recordings(recordings: Vec<Recording>) -> DayInput {
    DayInput {
        recordings,
        ..input(Vec::new(), Vec::new())
    }
}

/// A recording of project `p1` without note or tags; `end: None` is the running timer.
pub fn recording(id: &str, start: Ms, end: Option<Ms>) -> Recording {
    Recording {
        id: id.into(),
        project_id: "p1".into(),
        start,
        end,
        note: String::new(),
        tag_ids: Vec::new(),
        created_at: 0,
        updated_at: 0,
    }
}

pub fn project_ref(name: &str) -> ProjectRef {
    ProjectRef {
        name: name.into(),
        client_name: None,
        color: "blue".into(),
        shade: 4,
    }
}
