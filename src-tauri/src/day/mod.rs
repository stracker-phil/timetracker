//! The Day view as a pure function of the day's rows and the timeline settings (ADR 0012): the page
//! positions blocks from `start`, `end`, `col` and `cols` and computes nothing.

pub mod commands;
#[cfg(test)]
mod testing;

use std::collections::HashMap;

use serde::Serialize;
use ts_rs::TS;

use crate::db::activities::ActivityRow;
use crate::db::away::{AwayReason, AwayRow};
use crate::db::time::{Day, Ms, clip};
use crate::store::settings::{AppOverride, Settings, TimelineSettings};

/// Time without activity or away shorter than this is no gap.
pub const GAP_MIN: Ms = 2 * 60_000;

const TRACKER_SOURCE: &str = "tracker";
const EXTERNAL_PREFIX: &str = "external:";
const PRIVATE_TITLE: &str = "Private";

#[derive(Debug, Clone, Default)]
pub struct ViewSettings {
    pub timeline: TimelineSettings,
    /// Keyed by bundle id.
    pub apps: HashMap<String, AppOverride>,
}

impl From<&Settings> for ViewSettings {
    fn from(settings: &Settings) -> Self {
        Self {
            timeline: settings.timeline.clone(),
            apps: settings.apps.clone(),
        }
    }
}

impl ViewSettings {
    fn app(&self, bundle_id: Option<&str>) -> Option<&AppOverride> {
        self.apps.get(bundle_id?)
    }
}

/// Rows as stored: unclipped, open spans with `end: None`.
#[derive(Debug, Clone)]
pub struct DayInput {
    pub day: Day,
    pub bounds: (Ms, Ms),
    pub now: Ms,
    pub activities: Vec<ActivityRow>,
    pub away: Vec<AwayRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct DayView {
    pub day: Day,
    pub bounds: (Ms, Ms),
    /// Activity lane first, then app lanes, then source lanes.
    pub lanes: Vec<Lane>,
    pub away: Vec<AwayBlock>,
    pub gaps: Vec<Gap>,
    pub totals: DayTotals,
    pub first_activity: Option<Ms>,
    /// Every source of the day's activity, sorted.
    pub sources: Vec<String>,
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
    /// `activity`, `app:<bundle id>` or `source:<name>`.
    pub id: String,
    pub kind: LaneKind,
    pub label: String,
    pub bundle_id: Option<String>,
    pub blocks: Vec<ActivityBlock>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ActivityBlock {
    /// `a<activity id>`.
    pub id: String,
    pub start: Ms,
    pub end: Ms,
    pub app: String,
    pub bundle_id: Option<String>,
    pub title: String,
    pub url: Option<String>,
    pub path: Option<String>,
    pub source: String,
    /// Under a recording.
    pub covered: bool,
    /// The activity ids the block stands for.
    pub members: Vec<i64>,
    /// Position among the overlapping blocks of its lane: column `col` of `cols` equal columns.
    pub col: u8,
    pub cols: u8,
    pub private: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct AwayBlock {
    pub id: i64,
    pub start: Ms,
    pub end: Ms,
    pub reason: AwayReason,
}

/// Time inside the day's activity with neither activity nor away.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Gap {
    pub start: Ms,
    pub end: Ms,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct DayTotals {
    pub recorded_ms: i64,
    /// Tracker activity, overlaps counted once; external sources are not tracked time.
    pub tracked_ms: i64,
    pub away_ms: i64,
}

pub fn view(input: &DayInput, settings: &ViewSettings) -> DayView {
    let blocks = clipped_blocks(input);
    let blocks = without_hidden_apps(blocks, settings);
    let blocks = group_by_app(blocks, settings);
    let blocks = bridge_gaps(blocks, settings);
    let blocks = drop_noise(blocks, settings);
    let blocks = choose_titles(blocks, settings);

    let away = away_blocks(input);
    let activity: Vec<(Ms, Ms)> = blocks.iter().map(|b| (b.start, b.end)).collect();
    let away_spans: Vec<(Ms, Ms)> = away.iter().map(|a| (a.start, a.end)).collect();
    let tracked: Vec<(Ms, Ms)> = blocks
        .iter()
        .filter(|b| b.source == TRACKER_SOURCE)
        .map(|b| (b.start, b.end))
        .collect();
    let totals = DayTotals {
        recorded_ms: 0,
        tracked_ms: union_length(tracked),
        away_ms: union_length(away_spans.clone()),
    };
    let gaps = gaps(&activity, &away_spans);
    let first_activity = activity.iter().map(|s| s.0).min();
    let mut sources: Vec<String> = blocks.iter().map(|b| b.source.clone()).collect();
    sources.sort();
    sources.dedup();

    let mut lanes: Vec<Lane> = summary_lane(&blocks, settings).into_iter().collect();
    lanes.extend(lanes_of(blocks, settings));
    DayView {
        day: input.day,
        bounds: input.bounds,
        lanes,
        away,
        gaps,
        totals,
        first_activity,
        sources,
    }
}

fn away_blocks(input: &DayInput) -> Vec<AwayBlock> {
    input
        .away
        .iter()
        .filter_map(|row| {
            let (start, end) = clip(row.start, row.end, input.bounds, input.now)?;
            Some(AwayBlock {
                id: row.id,
                start,
                end,
                reason: row.reason,
            })
        })
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
                (row.title.clone(), row.url.clone(), row.path.clone())
            };
            Some(ActivityBlock {
                id: format!("a{}", row.id),
                start,
                end,
                app: row.app.clone(),
                bundle_id: row.bundle_id.clone(),
                title,
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

fn without_hidden_apps(blocks: Vec<ActivityBlock>, settings: &ViewSettings) -> Vec<ActivityBlock> {
    blocks
        .into_iter()
        .filter(|b| {
            !settings
                .app(b.bundle_id.as_deref())
                .is_some_and(|a| a.hidden)
        })
        .collect()
}

/// Group by app: merges an app's neighboring blocks. Passes them through for now.
fn group_by_app(blocks: Vec<ActivityBlock>, _settings: &ViewSettings) -> Vec<ActivityBlock> {
    blocks
}

/// Continuity: closes short gaps between blocks. Passes them through for now.
fn bridge_gaps(blocks: Vec<ActivityBlock>, _settings: &ViewSettings) -> Vec<ActivityBlock> {
    blocks
}

/// Noise cancellation: hides short blocks. Passes them through for now.
fn drop_noise(blocks: Vec<ActivityBlock>, _settings: &ViewSettings) -> Vec<ActivityBlock> {
    blocks
}

/// Chooses which title a block shows. Passes them through for now.
fn choose_titles(blocks: Vec<ActivityBlock>, _settings: &ViewSettings) -> Vec<ActivityBlock> {
    blocks
}

/// The summary lane, one block per interval. None for now.
fn summary_lane(_blocks: &[ActivityBlock], _settings: &ViewSettings) -> Option<Lane> {
    None
}

/// The activity lane (always present), then app lanes and source lanes by label, each packed.
fn lanes_of(blocks: Vec<ActivityBlock>, settings: &ViewSettings) -> Vec<Lane> {
    let mut activity = Vec::new();
    let mut apps: HashMap<String, (String, Vec<ActivityBlock>)> = HashMap::new();
    let mut sources: HashMap<String, Vec<ActivityBlock>> = HashMap::new();
    for block in blocks {
        if let Some(name) = block.source.strip_prefix(EXTERNAL_PREFIX) {
            sources.entry(name.to_string()).or_default().push(block);
        } else if let Some(bundle_id) = block.bundle_id.as_deref()
            && settings.app(Some(bundle_id)).is_some_and(|a| a.own_lane)
        {
            let lane = apps
                .entry(bundle_id.to_string())
                .or_insert_with(|| (block.app.clone(), Vec::new()));
            lane.1.push(block);
        } else {
            activity.push(block);
        }
    }

    let mut app_lanes: Vec<Lane> = apps
        .into_iter()
        .map(|(bundle_id, (label, blocks))| Lane {
            id: format!("app:{bundle_id}"),
            kind: LaneKind::App,
            label,
            bundle_id: Some(bundle_id),
            blocks: packed(blocks),
        })
        .collect();
    app_lanes.sort_by(by_label);
    let mut source_lanes: Vec<Lane> = sources
        .into_iter()
        .map(|(name, blocks)| Lane {
            id: format!("source:{name}"),
            kind: LaneKind::Source,
            label: name,
            bundle_id: None,
            blocks: packed(blocks),
        })
        .collect();
    source_lanes.sort_by(by_label);

    let mut lanes = vec![Lane {
        id: "activity".into(),
        kind: LaneKind::Activity,
        label: "Activity".into(),
        bundle_id: None,
        blocks: packed(activity),
    }];
    lanes.extend(app_lanes);
    lanes.extend(source_lanes);
    lanes
}

fn by_label(a: &Lane, b: &Lane) -> std::cmp::Ordering {
    a.label
        .to_lowercase()
        .cmp(&b.label.to_lowercase())
        .then_with(|| a.id.cmp(&b.id))
}

/// Sorted by start, longer first. Each block takes the lowest column that is free at its start;
/// blocks connected by overlap form a cluster and share its width (`cols`) evenly.
fn packed(mut blocks: Vec<ActivityBlock>) -> Vec<ActivityBlock> {
    blocks.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then(b.end.cmp(&a.end))
            .then_with(|| a.id.cmp(&b.id))
    });
    let mut column_ends: Vec<Ms> = Vec::new();
    let mut cluster_start = 0;
    let mut cluster_end = Ms::MIN;
    for i in 0..blocks.len() {
        if blocks[i].start >= cluster_end {
            close_cluster(&mut blocks[cluster_start..i], column_ends.len());
            column_ends.clear();
            cluster_start = i;
        }
        let block = &mut blocks[i];
        let col = match column_ends.iter().position(|&end| end <= block.start) {
            Some(free) => {
                column_ends[free] = block.end;
                free
            }
            None => {
                column_ends.push(block.end);
                column_ends.len() - 1
            }
        };
        block.col = u8::try_from(col).unwrap_or(u8::MAX);
        cluster_end = cluster_end.max(block.end);
    }
    let len = blocks.len();
    close_cluster(&mut blocks[cluster_start..len], column_ends.len());
    blocks
}

fn close_cluster(cluster: &mut [ActivityBlock], columns: usize) {
    let cols = u8::try_from(columns.max(1)).unwrap_or(u8::MAX);
    for block in cluster {
        block.cols = cols;
    }
}

/// Sorted spans with overlapping and touching ones merged.
fn union(mut spans: Vec<(Ms, Ms)>) -> Vec<(Ms, Ms)> {
    spans.sort_unstable();
    let mut merged: Vec<(Ms, Ms)> = Vec::with_capacity(spans.len());
    for (start, end) in spans {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

fn union_length(spans: Vec<(Ms, Ms)>) -> Ms {
    union(spans).iter().map(|(start, end)| end - start).sum()
}

/// Holes of at least `GAP_MIN` between the first activity's start and the last activity's end.
fn gaps(activity: &[(Ms, Ms)], away: &[(Ms, Ms)]) -> Vec<Gap> {
    let (Some(first), Some(last)) = (
        activity.iter().map(|s| s.0).min(),
        activity.iter().map(|s| s.1).max(),
    ) else {
        return Vec::new();
    };
    let filled = union(activity.iter().chain(away).copied().collect());
    let mut gaps = Vec::new();
    let mut at = first;
    for (start, end) in filled {
        if start >= last {
            break;
        }
        if start - at >= GAP_MIN {
            gaps.push(Gap {
                start: at,
                end: start,
            });
        }
        at = at.max(end);
    }
    gaps
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use serde_json::json;

    fn lane<'a>(view: &'a DayView, id: &str) -> &'a Lane {
        view.lanes
            .iter()
            .find(|l| l.id == id)
            .unwrap_or_else(|| panic!("no lane {id}"))
    }

    fn lane_ids(view: &DayView) -> Vec<&str> {
        view.lanes.iter().map(|l| l.id.as_str()).collect()
    }

    fn block_ids(lane: &Lane) -> Vec<&str> {
        lane.blocks.iter().map(|b| b.id.as_str()).collect()
    }

    fn spans(lane: &Lane) -> Vec<(Ms, Ms)> {
        lane.blocks.iter().map(|b| (b.start, b.end)).collect()
    }

    fn columns(lane: &Lane) -> Vec<(u8, u8)> {
        lane.blocks.iter().map(|b| (b.col, b.cols)).collect()
    }

    fn gap_spans(view: &DayView) -> Vec<(Ms, Ms)> {
        view.gaps.iter().map(|g| (g.start, g.end)).collect()
    }

    fn code(id: i64, start_min: i64, end_min: i64) -> ActivityRow {
        activity(id, mins(start_min), Some(mins(end_min)), "Code", "")
    }

    fn activity_lane(view: &DayView) -> &Lane {
        lane(view, "activity")
    }

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
    fn away_spans_are_clipped_to_the_day_and_an_open_one_ends_at_now() {
        let away_rows = vec![
            away(1, mins(30), Some(mins(90)), AwayReason::Sleep),
            away(2, mins(110), None, AwayReason::Idle),
            away(3, 0, Some(mins(60)), AwayReason::Lock),
            away(4, 3 * HOUR, Some(4 * HOUR), AwayReason::Paused),
        ];
        let input = DayInput {
            bounds: (HOUR, 2 * HOUR),
            now: mins(115),
            ..input(vec![], away_rows)
        };

        let view = view(&input, &settings());

        assert_eq!(
            view.away,
            [
                AwayBlock {
                    id: 1,
                    start: mins(60),
                    end: mins(90),
                    reason: AwayReason::Sleep
                },
                AwayBlock {
                    id: 2,
                    start: mins(110),
                    end: mins(115),
                    reason: AwayReason::Idle
                },
            ]
        );
        assert_eq!(view.totals.away_ms, mins(35));
    }

    #[test]
    fn overlapping_away_spans_count_once_in_the_total() {
        let away_rows = vec![
            away(1, 0, Some(mins(30)), AwayReason::Idle),
            away(2, mins(20), Some(mins(50)), AwayReason::Lock),
        ];

        let view = view(&input(vec![], away_rows), &settings());

        assert_eq!(view.away.len(), 2);
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
                .map(|(i, &(s, e))| code(i as i64 + 1, s, e))
                .collect();

            let view = view(&input(rows, vec![]), &settings());

            assert_eq!(columns(activity_lane(&view)), expected, "{name}");
        }
    }

    #[test]
    fn of_two_blocks_starting_together_the_longer_one_takes_column_zero() {
        let rows = vec![code(1, 0, 30), code(2, 0, 60)];

        let view = view(&input(rows, vec![]), &settings());

        let lane = activity_lane(&view);
        assert_eq!(block_ids(lane), ["a2", "a1"]);
        assert_eq!(columns(lane), [(0, 2), (1, 2)]);
    }

    #[test]
    fn an_empty_day_still_has_the_activity_lane() {
        let view = view(&input(vec![], vec![]), &settings());

        assert_eq!(lane_ids(&view), ["activity"]);
        let lane = activity_lane(&view);
        assert_eq!(lane.kind, LaneKind::Activity);
        assert_eq!(lane.label, "Activity");
        assert_eq!(lane.bundle_id, None);
        assert!(lane.blocks.is_empty());
        assert!(view.gaps.is_empty());
        assert_eq!(view.totals, DayTotals::default());
        assert_eq!(view.first_activity, None);
    }

    #[test]
    fn lanes_are_activity_then_app_lanes_then_source_lanes_each_sorted() {
        let rows = vec![
            code(1, 0, 10),
            activity(2, mins(10), Some(mins(20)), "Notes", ""),
            activity(3, mins(20), Some(mins(30)), "Mail", ""),
            from_source(code(4, 30, 40), "external:planner"),
            from_source(code(5, 40, 50), "external:calendar"),
        ];
        let settings = own_lane(own_lane(settings(), &bundle_of("Mail")), &bundle_of("Code"));

        let view = view(&input(rows, vec![]), &settings);

        assert_eq!(
            lane_ids(&view),
            [
                "activity",
                "app:test.code",
                "app:test.mail",
                "source:calendar",
                "source:planner"
            ]
        );
        let labels: Vec<&str> = view.lanes.iter().map(|l| l.label.as_str()).collect();
        assert_eq!(labels, ["Activity", "Code", "Mail", "calendar", "planner"]);
        let kinds: Vec<LaneKind> = view.lanes.iter().map(|l| l.kind).collect();
        assert_eq!(
            kinds,
            [
                LaneKind::Activity,
                LaneKind::App,
                LaneKind::App,
                LaneKind::Source,
                LaneKind::Source
            ]
        );
        assert_eq!(
            lane(&view, "app:test.code").bundle_id.as_deref(),
            Some("test.code")
        );
        assert_eq!(block_ids(activity_lane(&view)), ["a2"]);
        assert_eq!(block_ids(lane(&view, "app:test.code")), ["a1"]);
        assert_eq!(block_ids(lane(&view, "app:test.mail")), ["a3"]);
        assert_eq!(block_ids(lane(&view, "source:planner")), ["a4"]);
        assert_eq!(block_ids(lane(&view, "source:calendar")), ["a5"]);
    }

    #[test]
    fn blocks_of_an_app_lane_are_packed_separately_from_the_activity_lane() {
        let rows = vec![
            code(1, 0, 60),
            activity(2, mins(30), Some(mins(90)), "Mail", ""),
            activity(3, mins(30), Some(mins(90)), "Mail", ""),
        ];
        let settings = own_lane(settings(), &bundle_of("Mail"));

        let view = view(&input(rows, vec![]), &settings);

        assert_eq!(columns(activity_lane(&view)), [(0, 1)]);
        assert_eq!(columns(lane(&view, "app:test.mail")), [(0, 2), (1, 2)]);
    }

    #[test]
    fn an_app_without_a_bundle_id_stays_in_the_activity_lane() {
        let row = with_bundle_id(code(1, 0, 10), None);
        let settings = own_lane(hidden(settings(), &bundle_of("Code")), "other");

        let view = view(&input(vec![row], vec![]), &settings);

        assert_eq!(lane_ids(&view), ["activity"]);
        assert_eq!(block_ids(activity_lane(&view)), ["a1"]);
    }

    #[test]
    fn a_hidden_app_vanishes_from_lanes_totals_gaps_and_sources() {
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

        assert_eq!(lane_ids(&view), ["activity"]);
        assert_eq!(block_ids(activity_lane(&view)), ["a1", "a3"]);
        assert_eq!(view.totals.tracked_ms, mins(20));
        assert_eq!(gap_spans(&view), [(mins(10), mins(60))]);
        assert_eq!(view.sources, ["tracker"]);
    }

    #[test]
    fn a_hole_of_under_two_minutes_is_no_gap_and_two_minutes_is() {
        for (hole, expected) in [(GAP_MIN - 1, vec![]), (GAP_MIN, vec![(mins(10), mins(12))])] {
            let rows = vec![
                code(1, 0, 10),
                activity(2, mins(10) + hole, Some(mins(20) + hole), "Code", ""),
            ];

            let view = view(&input(rows, vec![]), &settings());

            assert_eq!(gap_spans(&view), expected, "hole {hole}");
        }
    }

    #[test]
    fn gaps_lie_only_between_the_first_and_the_last_activity() {
        let rows = vec![code(1, 300, 310), code(2, 330, 340)];

        let view = view(&input(rows, vec![]), &settings());

        assert_eq!(gap_spans(&view), [(mins(310), mins(330))]);
    }

    #[test]
    fn a_day_with_one_activity_has_no_gaps() {
        let view = view(&input(vec![code(1, 0, 10)], vec![]), &settings());

        assert!(view.gaps.is_empty());
    }

    #[test]
    fn external_activity_fills_gaps_between_tracker_spans() {
        let rows = vec![
            code(1, 0, 10),
            from_source(code(2, 10, 40), "external:planner"),
            code(3, 40, 50),
        ];

        let view = view(&input(rows, vec![]), &settings());

        assert!(view.gaps.is_empty());
    }

    #[test]
    fn away_spans_fill_the_holes_they_cover() {
        let rows = vec![code(1, 0, 10), code(2, 40, 50)];

        let covered = view(
            &input(
                rows.clone(),
                vec![away(1, mins(10), Some(mins(40)), AwayReason::Idle)],
            ),
            &settings(),
        );
        let partly = view(
            &input(
                rows,
                vec![away(1, mins(15), Some(mins(30)), AwayReason::Idle)],
            ),
            &settings(),
        );

        assert!(covered.gaps.is_empty());
        assert_eq!(
            gap_spans(&partly),
            [(mins(10), mins(15)), (mins(30), mins(40))]
        );
    }

    #[test]
    fn overlapping_tracker_spans_count_once_and_external_ones_not_at_all() {
        let rows = vec![
            code(1, 0, 60),
            code(2, 30, 90),
            from_source(code(3, 0, 120), "external:planner"),
        ];

        let view = view(&input(rows, vec![]), &settings());

        assert_eq!(
            view.totals,
            DayTotals {
                recorded_ms: 0,
                tracked_ms: mins(90),
                away_ms: 0
            }
        );
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
                app: "Code".into(),
                bundle_id: Some("test.code".into()),
                title: "main.rs".into(),
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
        let away_rows = vec![away(3, mins(30), Some(mins(40)), AwayReason::Idle)];

        let view = view(&input(rows, away_rows), &settings());

        assert_eq!(
            serde_json::to_value(&view).unwrap(),
            json!({
                "day": "2026-01-05",
                "bounds": [0, DAY_LENGTH],
                "lanes": [{
                    "id": "activity",
                    "kind": "activity",
                    "label": "Activity",
                    "bundleId": null,
                    "blocks": [{
                        "id": "a1",
                        "start": mins(10),
                        "end": mins(20),
                        "app": "Code",
                        "bundleId": "test.code",
                        "title": "main.rs",
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
                "away": [{"id": 3, "start": mins(30), "end": mins(40), "reason": "idle"}],
                "gaps": [],
                "totals": {"recordedMs": 0, "trackedMs": mins(10), "awayMs": mins(10)},
                "firstActivity": mins(10),
                "sources": ["tracker"]
            })
        );
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

    #[test]
    fn the_summary_lane_is_not_built_yet() {
        let view = view(&input(vec![code(1, 0, 10)], vec![]), &settings());

        assert!(view.lanes.iter().all(|l| l.kind != LaneKind::Summary));
    }
}
