//! The Day view as a pure function of the day's rows and the timeline settings (ADR 0012): the page
//! positions blocks from `start`, `end`, `col` and `cols` and computes nothing.

pub mod apps;
pub mod commands;
mod lanes;
mod load;
mod recordings;
mod rule_blocks;
mod summary;
#[cfg(test)]
mod testing;
mod title;
mod totals;
pub(crate) mod transform;
mod types;

use std::collections::HashMap;

use crate::calendar::titles::day_title;
use crate::db::activities::ActivityRow;
use crate::db::away::AwayRow;
use crate::db::recordings::Recording;
use crate::db::time::{Day, Ms, clip};
use crate::recordings::ticks;
use crate::store::settings::{AppOverride, DayTitle, Settings, TimelineSettings};

pub use load::{load, names, totals_of, view_of};
pub use recordings::{recording_blocks, timesheet};
use totals::union;
pub use totals::{totals, tracked_ms};
pub use types::*;

const TRACKER_SOURCE: &str = "tracker";
const EXTERNAL_PREFIX: &str = "external:";
const PRIVATE_TITLE: &str = "Private";

#[derive(Debug, Clone, Default)]
pub struct ViewSettings {
    pub timeline: TimelineSettings,
    /// Keyed by `ActivityBlock::app_key`.
    pub apps: HashMap<String, AppOverride>,
    pub day_title: DayTitle,
}

impl From<&Settings> for ViewSettings {
    fn from(settings: &Settings) -> Self {
        Self {
            timeline: settings.timeline.clone(),
            apps: settings.apps.clone(),
            day_title: settings.day_title,
        }
    }
}

impl ViewSettings {
    fn app(&self, key: &str) -> Option<&AppOverride> {
        self.apps.get(key)
    }
}

/// What app overrides are keyed by: the bundle id, or the app's name without one.
pub fn app_key(bundle_id: Option<&str>, app: &str) -> String {
    bundle_id.unwrap_or(app).to_string()
}

/// Rows as stored: unclipped, open spans with `end: None`.
#[derive(Debug, Clone)]
pub struct DayInput {
    pub day: Day,
    pub bounds: (Ms, Ms),
    pub now: Ms,
    pub activities: Vec<ActivityRow>,
    pub away: Vec<AwayRow>,
    pub recordings: Vec<Recording>,
    pub names: Names,
}

/// What recording blocks show of their projects and tags.
#[derive(Debug, Clone, Default)]
pub struct Names {
    /// By project id.
    pub projects: HashMap<String, ProjectRef>,
    /// By tag id.
    pub tags: HashMap<String, TagRef>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TagRef {
    pub name: String,
    pub emoji: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectRef {
    pub name: String,
    pub client_name: Option<String>,
    pub color: String,
    pub shade: u8,
}

pub fn view(input: &DayInput, settings: &ViewSettings) -> DayView {
    let rows = clipped_blocks(input);
    let recordings = recording_blocks(&input.recordings, &input.names, input.bounds, input.now);
    let barriers = barriers(input);
    let captured = captured(input, &rows);
    let hidden_apps = hidden_apps(&rows, settings);

    let (tracker, external): (Vec<_>, Vec<_>) =
        rows.into_iter().partition(|b| b.source == TRACKER_SOURCE);
    let tracker = transform::without_hidden(tracker, settings);
    let apps = apps::day_apps(&tracker, settings);
    let rule_blocks =
        rule_blocks::rule_blocks(&tracker, &external, &barriers, input.bounds, settings);
    let mut lanes = timeline_lanes(tracker, external, &barriers, input.bounds, settings);
    mark_covered(&mut lanes, &recordings);

    DayView {
        day: input.day,
        label: day_title(input.day, settings.day_title, Day::of(input.now)),
        bounds: input.bounds,
        ticks: ticks(
            input.bounds.0,
            input.bounds.1,
            u32::from(settings.timeline.snap_minutes),
        ),
        lanes,
        timesheet: timesheet(&recordings),
        recordings,
        barriers,
        rule_blocks,
        totals: captured.totals,
        first_activity: captured.first_activity,
        sources: captured.sources,
        compact: settings.timeline.compact,
        hidden_apps,
        apps,
        suggestions: Vec::new(),
    }
}

/// What was captured, before any setting hides or merges it.
struct Captured {
    totals: DayTotals,
    first_activity: Option<Ms>,
    sources: Vec<String>,
}

fn captured(input: &DayInput, rows: &[ActivityBlock]) -> Captured {
    let mut sources: Vec<String> = rows.iter().map(|b| b.source.clone()).collect();
    sources.sort();
    sources.dedup();
    Captured {
        totals: totals(input),
        first_activity: rows.iter().map(|b| b.start).min(),
        sources,
    }
}

/// The summary lane (when on) from the visible tracker rows, then the grouped activity, app and source lanes.
fn timeline_lanes(
    tracker: Vec<ActivityBlock>,
    external: Vec<ActivityBlock>,
    barriers: &[(Ms, Ms)],
    bounds: (Ms, Ms),
    settings: &ViewSettings,
) -> Vec<Lane> {
    let pieces = transform::identified(tracker, settings);
    let timeline = &settings.timeline;
    let mut lanes: Vec<Lane> = Vec::new();
    if timeline.summary_lane {
        let bucket = Ms::from(timeline.summary_minutes) * 60_000;
        lanes.push(summary::summary(&pieces, bucket, bounds));
    }
    let tracker = transform::grouped(pieces, settings, barriers);
    lanes.extend(lanes::lanes(tracker, external, settings));
    lanes
}

fn hidden_apps(rows: &[ActivityBlock], settings: &ViewSettings) -> Vec<HiddenApp> {
    let mut hidden: Vec<HiddenApp> = settings
        .apps
        .iter()
        .filter(|(_, app)| app.hidden)
        .map(|(key, _)| HiddenApp {
            app_key: key.clone(),
            app: rows
                .iter()
                .find(|b| &b.app_key == key)
                .map_or_else(|| key.clone(), |b| b.app.clone()),
        })
        .collect();
    hidden.sort_by(|a, b| {
        a.app
            .to_lowercase()
            .cmp(&b.app.to_lowercase())
            .then_with(|| a.app_key.cmp(&b.app_key))
    });
    hidden
}

/// A block is covered when at least half of it lies inside recordings.
fn mark_covered(lanes: &mut [Lane], recordings: &[RecordingBlock]) {
    let recorded = union(recordings.iter().map(|r| (r.start, r.end)).collect());
    for block in lanes.iter_mut().flat_map(|l| l.blocks.iter_mut()) {
        let inside: Ms = recorded
            .iter()
            .map(|&(start, end)| (end.min(block.end) - start.max(block.start)).max(0))
            .sum();
        block.covered = 2 * inside >= block.end - block.start;
    }
}

/// The away spans that break continuity (sleep, lock, pause), cut to the day.
fn barriers(input: &DayInput) -> Vec<(Ms, Ms)> {
    input
        .away
        .iter()
        .filter(|row| transform::breaks_continuity(row.reason))
        .filter_map(|row| clip(row.start, row.end, input.bounds, input.now))
        .collect()
}

/// Every activity cut to the day; private rows lose everything but the app.
fn clipped_blocks(input: &DayInput) -> Vec<ActivityBlock> {
    input
        .activities
        .iter()
        .filter_map(|row| {
            let (start, end) = clip(row.start, row.end, input.bounds, input.now)?;
            let (title, url, path) = if row.private {
                (PRIVATE_TITLE.to_string(), None, None)
            } else {
                (
                    title::clean_title(&row.title),
                    row.url.clone(),
                    row.path.clone(),
                )
            };
            let label = if title.is_empty() {
                row.app.clone()
            } else {
                title.clone()
            };
            Some(ActivityBlock {
                id: format!("a{}", row.id),
                start,
                end,
                active_ms: end - start,
                app: row.app.clone(),
                bundle_id: row.bundle_id.clone(),
                app_key: app_key(row.bundle_id.as_deref(), &row.app),
                title,
                label,
                url,
                path,
                source: row.source.clone(),
                covered: false,
                members: vec![row.id],
                col: 0,
                cols: 1,
                private: row.private,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use crate::db::away::AwayReason;
    use crate::store::settings::{Noise, Titles};
    use serde_json::json;

    #[test]
    fn spans_across_the_day_edges_appear_clipped_to_the_day() {
        let rows = vec![
            activity(1, mins(30), Some(mins(90)), "Code", ""),
            activity(2, mins(90), Some(mins(150)), "Mail", ""),
        ];
        let input = DayInput {
            bounds: (HOUR, 2 * HOUR),
            ..input(rows, vec![])
        };

        let view = view(&input, &settings());

        assert_eq!(
            spans(activity_lane(&view)),
            [(mins(60), mins(90)), (mins(90), mins(120))]
        );
        assert_eq!(view.bounds, (HOUR, 2 * HOUR));
        assert_eq!(view.totals.tracked_ms, HOUR);
    }

    #[test]
    fn spans_outside_the_day_or_touching_its_edge_are_dropped() {
        let rows = vec![
            activity(1, 0, Some(HOUR), "Code", ""),
            activity(2, 2 * HOUR, Some(3 * HOUR), "Code", ""),
            activity(3, 2 * HOUR, None, "Code", ""),
        ];
        let input = DayInput {
            bounds: (HOUR, 2 * HOUR),
            ..input(rows, vec![])
        };

        let view = view(&input, &settings());

        assert!(activity_lane(&view).blocks.is_empty());
        assert_eq!(view.first_activity, None);
        assert_eq!(view.sources, Vec::<String>::new());
    }

    #[test]
    fn an_open_span_ends_at_now_or_at_the_end_of_the_day() {
        for (now, expected_end) in [(mins(90), mins(90)), (3 * DAY_LENGTH, DAY_LENGTH)] {
            let input = DayInput {
                now,
                ..input(vec![activity(1, HOUR, None, "Code", "")], vec![])
            };

            let view = view(&input, &settings());

            assert_eq!(
                spans(activity_lane(&view)),
                [(HOUR, expected_end)],
                "now {now}"
            );
        }
    }

    #[test]
    fn an_open_span_that_starts_after_now_is_dropped() {
        let input = DayInput {
            now: HOUR,
            ..input(vec![activity(1, 2 * HOUR, None, "Code", "")], vec![])
        };

        assert!(activity_lane(&view(&input, &settings())).blocks.is_empty());
    }

    #[test]
    fn away_time_is_clipped_to_the_day_and_an_open_span_ends_at_now() {
        let away_rows = vec![
            away(1, mins(30), Some(mins(90)), AwayReason::Sleep),
            away(2, mins(110), None, AwayReason::Idle),
            away(3, 3 * HOUR, Some(4 * HOUR), AwayReason::Paused),
        ];
        let input = DayInput {
            bounds: (HOUR, 2 * HOUR),
            now: mins(115),
            ..input(vec![], away_rows)
        };

        let view = view(&input, &settings());

        assert_eq!(view.totals.away_ms, mins(35));
    }

    #[test]
    fn barriers_hold_the_clipped_sleep_lock_and_pause_spans_and_leave_idle_out() {
        let away_rows = vec![
            away(1, mins(30), Some(mins(90)), AwayReason::Sleep),
            away(2, mins(100), Some(mins(105)), AwayReason::Idle),
            away(3, mins(110), None, AwayReason::Lock),
            away(4, 0, Some(mins(40)), AwayReason::Paused),
            away(5, 3 * HOUR, Some(4 * HOUR), AwayReason::Sleep),
        ];
        let input = DayInput {
            bounds: (HOUR, 2 * HOUR),
            now: mins(115),
            ..input(vec![], away_rows)
        };

        assert_eq!(
            barriers(&input),
            [(mins(60), mins(90)), (mins(110), mins(115))]
        );
    }

    #[test]
    fn overlapping_away_spans_count_once_in_the_total() {
        let away_rows = vec![
            away(1, 0, Some(mins(30)), AwayReason::Idle),
            away(2, mins(20), Some(mins(50)), AwayReason::Lock),
        ];

        let view = view(&input(vec![], away_rows), &settings());

        assert_eq!(view.totals.away_ms, mins(50));
    }

    #[test]
    fn overlapping_blocks_share_the_width_of_their_cluster() {
        // (start, end) in minutes, ids by position.
        let cases: [(&str, Vec<(i64, i64)>, Vec<(u8, u8)>); 6] = [
            ("single", vec![(0, 60)], vec![(0, 1)]),
            (
                "two overlapping",
                vec![(0, 60), (30, 90)],
                vec![(0, 2), (1, 2)],
            ),
            (
                "third reuses the freed first column",
                vec![(0, 60), (30, 120), (70, 100)],
                vec![(0, 2), (1, 2), (0, 2)],
            ),
            (
                "three at once",
                vec![(0, 100), (10, 100), (20, 100)],
                vec![(0, 3), (1, 3), (2, 3)],
            ),
            (
                "separate clusters",
                vec![(0, 60), (30, 90), (200, 260)],
                vec![(0, 2), (1, 2), (0, 1)],
            ),
            (
                "touching blocks do not overlap",
                vec![(0, 60), (60, 120)],
                vec![(0, 1), (0, 1)],
            ),
        ];

        for (name, times, expected) in cases {
            let rows = times
                .iter()
                .enumerate()
                .map(|(i, &(s, e))| titled(i as i64 + 1, s, e, &format!("window {i}")))
                .collect();

            let view = view(&input(rows, vec![]), &settings());

            assert_eq!(columns(activity_lane(&view)), expected, "{name}");
        }
    }

    #[test]
    fn of_two_blocks_starting_together_the_longer_one_takes_column_zero() {
        let rows = vec![titled(1, 0, 30, "short"), titled(2, 0, 60, "long")];

        let view = view(&input(rows, vec![]), &settings());

        let lane = activity_lane(&view);
        assert_eq!(block_ids(lane), ["a2", "a1"]);
        assert_eq!(columns(lane), [(0, 2), (1, 2)]);
    }

    #[test]
    fn an_empty_day_still_has_the_activity_lane() {
        let view = view(&input(vec![], vec![]), &settings());

        assert_eq!(lane_id_list(&view), ["activity"]);
        let lane = activity_lane(&view);
        assert_eq!(lane.kind, LaneKind::Activity);
        assert_eq!(lane.label, "Activity");
        assert_eq!(lane.app_key, None);
        assert!(lane.blocks.is_empty());
        assert!(view.barriers.is_empty());
        assert_eq!(view.totals, DayTotals::default());
        assert_eq!(view.first_activity, None);
    }

    #[test]
    fn lanes_are_summary_then_app_lanes_then_activity_then_source_lanes_each_sorted() {
        let rows = vec![
            activity(1, mins(0), Some(mins(10)), "Mail", ""),
            activity(2, mins(10), Some(mins(20)), "Notes", ""),
            code(3, 20, 30),
            from_source(code(4, 30, 40), "external:planner"),
            from_source(code(5, 40, 50), "external:calendar"),
        ];
        let settings = with_timeline(
            own_lane(own_lane(settings(), &bundle_of("Mail")), &bundle_of("Code")),
            |t| t.summary_lane = true,
        );

        let view = view(&input(rows, vec![]), &settings);

        assert_eq!(
            lane_id_list(&view),
            [
                "summary",
                "app:test.mail",
                "app:test.code",
                "activity",
                "source:calendar",
                "source:planner"
            ]
        );
        let kinds: Vec<LaneKind> = view.lanes.iter().map(|l| l.kind).collect();
        assert_eq!(
            kinds,
            [
                LaneKind::Summary,
                LaneKind::App,
                LaneKind::App,
                LaneKind::Activity,
                LaneKind::Source,
                LaneKind::Source
            ]
        );
        assert_eq!(
            lane_by_id(&view, "app:test.code").app_key.as_deref(),
            Some("test.code")
        );
        assert_eq!(block_ids(activity_lane(&view)), ["a2"]);
        assert_eq!(block_ids(lane_by_id(&view, "app:test.code")), ["a3"]);
        assert_eq!(block_ids(lane_by_id(&view, "app:test.mail")), ["a1"]);
        assert_eq!(block_ids(lane_by_id(&view, "source:planner")), ["a4"]);
        assert_eq!(block_ids(lane_by_id(&view, "source:calendar")), ["a5"]);
    }

    #[test]
    fn blocks_of_an_app_lane_are_packed_separately_from_the_activity_lane() {
        let rows = vec![
            code(1, 0, 60),
            activity(2, mins(30), Some(mins(90)), "Mail", "inbox"),
            activity(3, mins(30), Some(mins(90)), "Mail", "drafts"),
        ];
        let settings = own_lane(settings(), &bundle_of("Mail"));

        let view = view(&input(rows, vec![]), &settings);

        assert_eq!(columns(activity_lane(&view)), [(0, 1)]);
        assert_eq!(
            columns(lane_by_id(&view, "app:test.mail")),
            [(0, 2), (1, 2)]
        );
    }

    #[test]
    fn an_app_without_a_bundle_id_is_keyed_by_its_name() {
        let row = with_bundle_id(code(1, 0, 10), None);

        let by_name = view(
            &input(vec![row.clone()], vec![]),
            &own_lane(settings(), "Code"),
        );
        let by_other_key = view(
            &input(vec![row], vec![]),
            &own_lane(hidden(settings(), &bundle_of("Code")), "other"),
        );

        assert_eq!(lane_id_list(&by_name), ["app:Code", "activity"]);
        assert_eq!(block_ids(lane_by_id(&by_name, "app:Code")), ["a1"]);
        assert_eq!(
            lane_by_id(&by_name, "app:Code").app_key.as_deref(),
            Some("Code")
        );
        assert_eq!(lane_id_list(&by_other_key), ["activity"]);
        assert_eq!(block_ids(activity_lane(&by_other_key)), ["a1"]);
    }

    #[test]
    fn a_hidden_app_vanishes_from_the_lanes_but_still_counts() {
        let rows = vec![
            code(1, 0, 10),
            activity(2, mins(10), Some(mins(60)), "Mail", ""),
            code(3, 60, 70),
            from_source(
                activity(4, mins(5), Some(mins(8)), "Mail", ""),
                "external:planner",
            ),
        ];
        let settings = own_lane(hidden(settings(), &bundle_of("Mail")), &bundle_of("Mail"));

        let view = view(&input(rows, vec![]), &settings);

        assert_eq!(lane_id_list(&view), ["activity", "source:planner"]);
        assert_eq!(block_ids(activity_lane(&view)), ["a1", "a3"]);
        assert_eq!(block_ids(lane_by_id(&view, "source:planner")), ["a4"]);
        assert_eq!(view.totals.tracked_ms, mins(70));
        assert_eq!(view.sources, ["external:planner", "tracker"]);
        assert_eq!(view.first_activity, Some(0));
    }

    #[test]
    fn first_activity_is_the_earliest_start_of_any_source() {
        let rows = vec![
            code(1, 30, 40),
            from_source(code(2, 10, 20), "external:planner"),
            code(3, 20, 25),
        ];

        let view = view(&input(rows, vec![]), &settings());

        assert_eq!(view.first_activity, Some(mins(10)));
    }

    #[test]
    fn sources_are_the_distinct_sources_of_the_day_sorted() {
        let rows = vec![
            code(1, 0, 10),
            from_source(code(2, 10, 20), "external:planner"),
            from_source(code(3, 20, 30), "external:calendar"),
            code(4, 30, 40),
            from_source(code(5, 40, 50), "external:planner"),
        ];

        let view = view(&input(rows, vec![]), &settings());

        assert_eq!(
            view.sources,
            ["external:calendar", "external:planner", "tracker"]
        );
    }

    #[test]
    fn a_block_carries_its_row_and_is_not_covered() {
        let row = with_path(
            with_url(
                activity(7, mins(10), Some(mins(20)), "Code", "main.rs"),
                "https://example.com",
            ),
            "/tmp/main.rs",
        );

        let view = view(&input(vec![row], vec![]), &settings());

        assert_eq!(
            activity_lane(&view).blocks,
            [ActivityBlock {
                id: "a7".into(),
                start: mins(10),
                end: mins(20),
                active_ms: mins(10),
                app: "Code".into(),
                bundle_id: Some("test.code".into()),
                app_key: "test.code".into(),
                title: "main.rs".into(),
                label: "main.rs".into(),
                url: Some("https://example.com".into()),
                path: Some("/tmp/main.rs".into()),
                source: "tracker".into(),
                covered: false,
                members: vec![7],
                col: 0,
                cols: 1,
                private: false,
            }]
        );
    }

    #[test]
    fn a_private_row_keeps_only_the_app() {
        let row = private(with_path(
            with_url(
                activity(3, mins(10), Some(mins(20)), "Safari", "Bank statement"),
                "https://bank.example",
            ),
            "/tmp/statement.pdf",
        ));

        let view = view(&input(vec![row], vec![]), &settings());

        let block = &activity_lane(&view).blocks[0];
        assert_eq!(block.title, "Private");
        assert_eq!(block.url, None);
        assert_eq!(block.path, None);
        assert!(block.private);
        assert_eq!(block.app, "Safari");
    }

    #[test]
    fn the_view_serializes_with_camel_case_keys_and_lowercase_kinds() {
        let rows = vec![activity(1, mins(10), Some(mins(20)), "Code", "main.rs")];
        let away_rows = vec![away(3, mins(30), Some(mins(40)), AwayReason::Sleep)];

        let view = view(&input(rows, away_rows), &settings());

        let value = serde_json::to_value(&view).unwrap();
        for key in ["away", "gaps", "barriers"] {
            assert!(value.get(key).is_none(), "{key}");
        }
        assert_eq!(
            value,
            json!({
                "day": "2026-01-05",
                "label": "Monday, January 5, 2026",
                "bounds": [0, DAY_LENGTH],
                "ticks": view.ticks,
                "lanes": [{
                    "id": "activity",
                    "kind": "activity",
                    "label": "Activity",
                    "appKey": null,
                    "bundleId": null,
                    "blocks": [{
                        "id": "a1",
                        "start": mins(10),
                        "end": mins(20),
                        "activeMs": mins(10),
                        "app": "Code",
                        "bundleId": "test.code",
                        "appKey": "test.code",
                        "title": "main.rs",
                        "label": "main.rs",
                        "url": null,
                        "path": null,
                        "source": "tracker",
                        "covered": false,
                        "members": [1],
                        "col": 0,
                        "cols": 1,
                        "private": false
                    }]
                }],
                "recordings": [],
                "timesheet": [],
                "totals": {"recordedMs": 0, "trackedMs": mins(10), "awayMs": mins(10)},
                "firstActivity": mins(10),
                "sources": ["tracker"],
                "compact": false,
                "hiddenApps": [],
                "apps": [{
                    "appKey": "test.code",
                    "app": "Code",
                    "bundleId": "test.code",
                    "activeMs": mins(10),
                    "ownLane": false
                }],
                "suggestions": []
            })
        );
    }

    #[test]
    fn the_view_lists_the_snap_marks_from_the_first_to_the_last_instant_of_the_day() {
        let bounds = day().bounds();
        let input = DayInput {
            bounds,
            ..input(vec![], vec![])
        };
        let settings = settings();
        let step = i64::from(settings.timeline.snap_minutes) * 60_000;

        let view = view(&input, &settings);

        assert_eq!(view.ticks.first(), Some(&bounds.0));
        assert_eq!(view.ticks.last(), Some(&bounds.1));
        assert!(view.ticks.windows(2).all(|w| w[1] - w[0] == step));
        let value = serde_json::to_value(&view).unwrap();
        assert_eq!(value["ticks"], json!(view.ticks));
    }

    #[test]
    fn the_view_is_labelled_by_the_day_title_setting() {
        let this_year = day().bounds().0 + HOUR;
        for (format, now, expected) in [
            (DayTitle::Long, this_year, "Monday, January 5, 2026"),
            (DayTitle::Short, this_year, "Mon, Jan 5"),
            (
                DayTitle::Short,
                400 * DAY_LENGTH + this_year,
                "Mon, Jan 5, 2026",
            ),
        ] {
            let input = DayInput {
                now,
                ..input(vec![], vec![])
            };
            let settings = ViewSettings {
                day_title: format,
                ..settings()
            };

            assert_eq!(view(&input, &settings).label, expected, "{format:?} {now}");
        }
    }

    #[test]
    fn apps_list_the_visible_tracker_apps_with_their_time_most_first() {
        let rows = vec![
            activity(1, 0, Some(mins(10)), "Mail", ""),
            code(2, 10, 40),
            activity(3, mins(40), Some(mins(60)), "Notes", ""),
            code(4, 60, 70),
            from_source(
                activity(5, mins(70), Some(mins(120)), "Planner", ""),
                "external:planner",
            ),
        ];
        let settings = own_lane(hidden(settings(), &bundle_of("Notes")), &bundle_of("Mail"));

        let view = view(&input(rows, vec![]), &settings);

        assert_eq!(
            view.apps,
            [
                DayApp {
                    app_key: "test.code".into(),
                    app: "Code".into(),
                    bundle_id: Some("test.code".into()),
                    active_ms: mins(40),
                    own_lane: false,
                },
                DayApp {
                    app_key: "test.mail".into(),
                    app: "Mail".into(),
                    bundle_id: Some("test.mail".into()),
                    active_ms: mins(10),
                    own_lane: true,
                },
            ]
        );
    }

    #[test]
    fn apps_with_equal_time_are_ordered_by_name_ignoring_case_and_keyed_by_name_without_a_bundle_id()
     {
        let rows = vec![
            activity(1, 0, Some(mins(5)), "beta", ""),
            with_bundle_id(activity(2, mins(5), Some(mins(10)), "Alpha", ""), None),
        ];

        let view = view(&input(rows, vec![]), &settings());

        let apps: Vec<(&str, Option<&str>)> = view
            .apps
            .iter()
            .map(|a| (a.app_key.as_str(), a.bundle_id.as_deref()))
            .collect();
        assert_eq!(apps, [("Alpha", None), ("test.beta", Some("test.beta"))]);
    }

    #[test]
    fn apps_count_the_time_before_bridging_and_noise() {
        let rows = vec![code(1, 0, 1), code(2, 10, 11)];
        let settings = with_timeline(settings(), |t| t.noise = Noise::High);

        let view = view(&input(rows, vec![]), &settings);

        assert!(activity_lane(&view).blocks.is_empty());
        assert_eq!(view.apps[0].active_ms, mins(2));
    }

    #[test]
    fn lane_kinds_serialize_as_lowercase_words() {
        for (kind, expected) in [
            (LaneKind::Summary, "summary"),
            (LaneKind::Activity, "activity"),
            (LaneKind::App, "app"),
            (LaneKind::Source, "source"),
        ] {
            assert_eq!(serde_json::to_value(kind).unwrap(), json!(expected));
        }
    }

    fn covered_flags(rows: Vec<ActivityRow>, recordings: Vec<Recording>) -> Vec<bool> {
        let input = DayInput {
            recordings,
            ..input(rows, vec![])
        };
        let view = view(&input, &settings());
        activity_lane(&view)
            .blocks
            .iter()
            .map(|b| b.covered)
            .collect()
    }

    #[test]
    fn a_block_is_covered_when_at_least_half_of_it_lies_inside_recordings() {
        let block = || vec![activity(1, 0, Some(100_000), "Code", "")];
        let cases: [(&str, Vec<Recording>, bool); 7] = [
            ("49 percent", vec![recording("r", 0, Some(49_000))], false),
            ("50 percent", vec![recording("r", 0, Some(50_000))], true),
            ("100 percent", vec![recording("r", 0, Some(100_000))], true),
            (
                "50 percent from the middle",
                vec![recording("r", 25_000, Some(75_000))],
                true,
            ),
            (
                "two recordings add up",
                vec![
                    recording("a", 0, Some(25_000)),
                    recording("b", 30_000, Some(55_000)),
                ],
                true,
            ),
            (
                "outside",
                vec![recording("r", 200_000, Some(300_000))],
                false,
            ),
            (
                "a running recording counts until now",
                vec![recording("r", 0, None)],
                true,
            ),
        ];

        for (name, recordings, expected) in cases {
            assert_eq!(covered_flags(block(), recordings), [expected], "{name}");
        }
    }

    #[test]
    fn without_recordings_no_block_is_covered() {
        let rows = vec![code(1, 0, 10), code(2, 20, 30)];

        assert_eq!(covered_flags(rows, vec![]), [false, false]);
    }

    #[test]
    fn titles_differing_only_by_a_leading_spinner_merge_into_one_block_with_one_label() {
        let rows = rows(&[
            (0, 10, "Code", "timetracker \u{2014} \u{25D0} Docs"),
            (10, 20, "Code", "timetracker \u{2014} \u{25D1} Docs"),
            (20, 30, "Code", "\u{2733} timetracker \u{2014} Docs"),
        ]);
        let settings = with_timeline(settings(), |t| t.titles = Titles::Document);

        let view = render(rows, &settings);

        let lane = activity_lane(&view);
        assert_eq!(shape(lane), [(0, 30, vec![1, 2, 3])]);
        assert_eq!(lane.blocks[0].label, "timetracker \u{2014} Docs");
        assert_eq!(lane.blocks[0].title, "timetracker \u{2014} Docs");
    }

    #[test]
    fn a_title_of_only_symbols_is_labelled_by_its_app() {
        let rows = rows(&[(0, 10, "Code", "\u{25D0}")]);
        let settings = with_timeline(settings(), |t| t.titles = Titles::Document);

        let view = render(rows, &settings);

        let block = &activity_lane(&view).blocks[0];
        assert_eq!((block.title.as_str(), block.label.as_str()), ("", "Code"));
    }

    #[test]
    fn coverage_is_judged_per_block() {
        let rows = vec![code(1, 0, 10), code(2, 20, 30)];

        let flags = covered_flags(rows, vec![recording("r", 0, Some(mins(10)))]);

        assert_eq!(flags, [true, false]);
    }
}
