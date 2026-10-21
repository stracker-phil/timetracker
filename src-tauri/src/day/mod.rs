//! The Day view as a pure function of the day's rows and the timeline settings (ADR 0012): the page
//! positions blocks from `start`, `end`, `col` and `cols` and computes nothing.

pub mod apps;
pub mod commands;
mod lanes;
#[cfg(test)]
mod testing;
pub(crate) mod transform;

use std::collections::HashMap;

use serde::Serialize;
use ts_rs::TS;

use crate::db::activities::ActivityRow;
use crate::db::away::{AwayReason, AwayRow};
use crate::db::recordings::Recording;
use crate::db::time::{Day, Ms, clip};
use crate::rules::types::Suggestion;
use crate::store::settings::{AppOverride, Settings, TimelineSettings};

/// Time without activity or away shorter than this is no gap.
pub const GAP_MIN: Ms = 2 * 60_000;

const TRACKER_SOURCE: &str = "tracker";
const EXTERNAL_PREFIX: &str = "external:";
const PRIVATE_TITLE: &str = "Private";

#[derive(Debug, Clone, Default)]
pub struct ViewSettings {
    pub timeline: TimelineSettings,
    /// Keyed by `ActivityBlock::app_key`.
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
    /// Tag names by id.
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectRef {
    pub name: String,
    pub client_name: Option<String>,
    pub color: String,
    pub shade: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct DayView {
    pub day: Day,
    pub bounds: (Ms, Ms),
    /// Summary lane, activity lane, app lanes, source lanes; each only when present.
    pub lanes: Vec<Lane>,
    /// The recordings column, by start.
    pub recordings: Vec<RecordingBlock>,
    pub away: Vec<AwayBlock>,
    pub gaps: Vec<Gap>,
    pub totals: DayTotals,
    pub first_activity: Option<Ms>,
    /// Every source of the day's activity, sorted.
    pub sources: Vec<String>,
    /// The timeline's compact density.
    pub compact: bool,
    /// Every app the settings hide, by name.
    pub hidden_apps: Vec<HiddenApp>,
    /// What the rules suggest recording, by start; filled by `commands::load`.
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

/// A recording cut to the day.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct RecordingBlock {
    pub id: String,
    pub start: Ms,
    pub end: Ms,
    pub project_id: String,
    pub project_name: String,
    pub client_name: Option<String>,
    /// The project's accent and shade.
    pub color: String,
    pub shade: u8,
    pub note: String,
    /// Tag names, by name.
    pub tags: Vec<String>,
    /// The timer, still recording.
    pub running: bool,
    /// The recording starts on an earlier day or ends on a later one.
    pub clipped_start: bool,
    pub clipped_end: bool,
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
    /// The day's part of its recordings.
    pub recorded_ms: i64,
    /// Tracker activity, overlaps counted once, hidden apps and noise included; external sources are not
    /// tracked time.
    pub tracked_ms: i64,
    pub away_ms: i64,
}

pub fn view(input: &DayInput, settings: &ViewSettings) -> DayView {
    let rows = clipped_blocks(input);
    let recordings = recording_blocks(&input.recordings, &input.names, input.bounds, input.now);
    let away = away_blocks(input);

    // Totals and gaps describe what was captured, before any setting hides or merges it.
    let span = |b: &ActivityBlock| (b.start, b.end);
    let activity: Vec<(Ms, Ms)> = rows.iter().map(span).collect();
    let away_spans: Vec<(Ms, Ms)> = away.iter().map(|a| (a.start, a.end)).collect();
    let recorded: Vec<(Ms, Ms)> = recordings.iter().map(|r| (r.start, r.end)).collect();
    let totals = DayTotals {
        recorded_ms: recorded.iter().map(|(start, end)| end - start).sum(),
        tracked_ms: tracked_ms(&input.activities, input.bounds, input.now),
        away_ms: union_length(away_spans.clone()),
    };
    let gaps = gaps(&activity, &away_spans);
    let first_activity = activity.iter().map(|s| s.0).min();
    let mut sources: Vec<String> = rows.iter().map(|b| b.source.clone()).collect();
    sources.sort();
    sources.dedup();
    let hidden_apps = hidden_apps(&rows, settings);

    let (tracker, external): (Vec<_>, Vec<_>) =
        rows.into_iter().partition(|b| b.source == TRACKER_SOURCE);
    let pieces = transform::identified(transform::without_hidden(tracker, settings), settings);
    let barriers: Vec<(Ms, Ms)> = away
        .iter()
        .filter(|a| transform::breaks_continuity(a.reason))
        .map(|a| (a.start, a.end))
        .collect();
    let timeline = &settings.timeline;
    let mut lanes: Vec<Lane> = Vec::new();
    if timeline.summary_lane {
        let interval = Ms::from(timeline.summary_minutes) * 60_000;
        lanes.push(lanes::summary(&pieces, interval, input.bounds));
    }
    let tracker = transform::grouped(pieces, settings, &barriers);
    lanes.extend(lanes::lanes(tracker, external, settings));
    mark_covered(&mut lanes, &recorded);

    DayView {
        day: input.day,
        bounds: input.bounds,
        lanes,
        recordings,
        away,
        gaps,
        totals,
        first_activity,
        sources,
        compact: timeline.compact,
        hidden_apps,
        suggestions: Vec::new(),
    }
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

/// `recordings` cut to `bounds`, by start; a running one ends at `now`.
pub fn recording_blocks(
    recordings: &[Recording],
    names: &Names,
    bounds: (Ms, Ms),
    now: Ms,
) -> Vec<RecordingBlock> {
    let mut blocks: Vec<RecordingBlock> = recordings
        .iter()
        .filter_map(|r| {
            let (start, end) = clip(r.start, r.end, bounds, now)?;
            let project = names.projects.get(&r.project_id);
            let mut tags: Vec<String> = r
                .tag_ids
                .iter()
                .filter_map(|id| names.tags.get(id).cloned())
                .collect();
            tags.sort_by_key(|t| t.to_lowercase());
            Some(RecordingBlock {
                id: r.id.clone(),
                start,
                end,
                project_id: r.project_id.clone(),
                project_name: project.map_or_else(String::new, |p| p.name.clone()),
                client_name: project.and_then(|p| p.client_name.clone()),
                color: project.map_or_else(|| "graphite".into(), |p| p.color.clone()),
                shade: project.map_or(4, |p| p.shade),
                note: r.note.clone(),
                tags,
                running: r.end.is_none(),
                clipped_start: r.start < bounds.0,
                clipped_end: r.end.unwrap_or(now) > bounds.1,
            })
        })
        .collect();
    blocks.sort_by_key(|b| b.start);
    blocks
}

/// A block is covered when at least half of it lies inside recordings.
fn mark_covered(lanes: &mut [Lane], recorded: &[(Ms, Ms)]) {
    let recorded = union(recorded.to_vec());
    for block in lanes.iter_mut().flat_map(|l| l.blocks.iter_mut()) {
        let inside: Ms = recorded
            .iter()
            .map(|&(start, end)| (end.min(block.end) - start.max(block.start)).max(0))
            .sum();
        block.covered = 2 * inside >= block.end - block.start;
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
            let label = if title.is_empty() {
                row.app.clone()
            } else {
                title.clone()
            };
            Some(ActivityBlock {
                id: format!("a{}", row.id),
                start,
                end,
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

/// Tracker activity inside `bounds`, overlaps counted once; external sources are not tracked time.
pub fn tracked_ms(activities: &[ActivityRow], bounds: (Ms, Ms), now: Ms) -> Ms {
    union_length(
        activities
            .iter()
            .filter(|a| a.source == TRACKER_SOURCE)
            .filter_map(|a| clip(a.start, a.end, bounds, now))
            .collect(),
    )
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

    fn titled(id: i64, start_min: i64, end_min: i64, title: &str) -> ActivityRow {
        activity(id, mins(start_min), Some(mins(end_min)), "Code", title)
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

        assert_eq!(lane_ids(&view), ["activity"]);
        let lane = activity_lane(&view);
        assert_eq!(lane.kind, LaneKind::Activity);
        assert_eq!(lane.label, "Activity");
        assert_eq!(lane.app_key, None);
        assert!(lane.blocks.is_empty());
        assert!(view.gaps.is_empty());
        assert_eq!(view.totals, DayTotals::default());
        assert_eq!(view.first_activity, None);
    }

    #[test]
    fn lanes_are_activity_then_app_lanes_then_source_lanes_each_sorted() {
        let rows = vec![
            activity(1, mins(0), Some(mins(10)), "Mail", ""),
            activity(2, mins(10), Some(mins(20)), "Notes", ""),
            code(3, 20, 30),
            from_source(code(4, 30, 40), "external:planner"),
            from_source(code(5, 40, 50), "external:calendar"),
        ];
        let settings = own_lane(own_lane(settings(), &bundle_of("Mail")), &bundle_of("Code"));

        let view = view(&input(rows, vec![]), &settings);

        assert_eq!(
            lane_ids(&view),
            [
                "activity",
                "app:test.mail",
                "app:test.code",
                "source:calendar",
                "source:planner"
            ]
        );
        let labels: Vec<&str> = view.lanes.iter().map(|l| l.label.as_str()).collect();
        assert_eq!(labels, ["Activity", "Mail", "Code", "calendar", "planner"]);
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
            lane(&view, "app:test.code").app_key.as_deref(),
            Some("test.code")
        );
        assert_eq!(block_ids(activity_lane(&view)), ["a2"]);
        assert_eq!(block_ids(lane(&view, "app:test.code")), ["a3"]);
        assert_eq!(block_ids(lane(&view, "app:test.mail")), ["a1"]);
        assert_eq!(block_ids(lane(&view, "source:planner")), ["a4"]);
        assert_eq!(block_ids(lane(&view, "source:calendar")), ["a5"]);
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
        assert_eq!(columns(lane(&view, "app:test.mail")), [(0, 2), (1, 2)]);
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

        assert_eq!(lane_ids(&by_name), ["activity", "app:Code"]);
        assert_eq!(block_ids(lane(&by_name, "app:Code")), ["a1"]);
        assert_eq!(lane(&by_name, "app:Code").app_key.as_deref(), Some("Code"));
        assert_eq!(lane_ids(&by_other_key), ["activity"]);
        assert_eq!(block_ids(activity_lane(&by_other_key)), ["a1"]);
    }

    #[test]
    fn a_hidden_app_vanishes_from_the_lanes_but_still_counts_and_leaves_no_gap() {
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

        assert_eq!(lane_ids(&view), ["activity", "source:planner"]);
        assert_eq!(block_ids(activity_lane(&view)), ["a1", "a3"]);
        assert_eq!(block_ids(lane(&view, "source:planner")), ["a4"]);
        assert_eq!(view.totals.tracked_ms, mins(70));
        assert!(view.gaps.is_empty());
        assert_eq!(view.sources, ["external:planner", "tracker"]);
        assert_eq!(view.first_activity, Some(0));
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
                    "appKey": null,
                    "blocks": [{
                        "id": "a1",
                        "start": mins(10),
                        "end": mins(20),
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
                "away": [{"id": 3, "start": mins(30), "end": mins(40), "reason": "idle"}],
                "gaps": [],
                "totals": {"recordedMs": 0, "trackedMs": mins(10), "awayMs": mins(10)},
                "firstActivity": mins(10),
                "sources": ["tracker"],
                "compact": false,
                "hiddenApps": [],
                "suggestions": []
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
    fn coverage_is_judged_per_block() {
        let rows = vec![code(1, 0, 10), code(2, 20, 30)];

        let flags = covered_flags(rows, vec![recording("r", 0, Some(mins(10)))]);

        assert_eq!(flags, [true, false]);
    }

    #[test]
    fn recorded_time_sums_the_parts_of_recordings_inside_the_day() {
        let input = DayInput {
            bounds: (HOUR, 2 * HOUR),
            now: mins(115),
            ..input_with_recordings(vec![
                recording("a", mins(30), Some(mins(90))),
                recording("b", mins(100), Some(mins(200))),
                recording("c", mins(110), None),
                recording("d", 0, Some(HOUR)),
                recording("e", 3 * HOUR, Some(4 * HOUR)),
            ])
        };

        let view = view(&input, &settings());

        assert_eq!(view.totals.recorded_ms, mins(30) + mins(20) + mins(5));
        assert_eq!(view.recordings.len(), 3);
    }

    #[test]
    fn a_recording_across_midnight_shows_clipped_on_both_days() {
        let midnight = DAY_LENGTH;
        let rows = vec![recording("r", midnight - HOUR, Some(midnight + 2 * HOUR))];
        let first = DayInput {
            bounds: (0, midnight),
            ..input_with_recordings(rows.clone())
        };
        let second = DayInput {
            bounds: (midnight, 2 * midnight),
            ..input_with_recordings(rows)
        };

        let first = view(&first, &settings());
        let second = view(&second, &settings());

        let shape = |view: &DayView| {
            view.recordings
                .iter()
                .map(|b| (b.start, b.end, b.clipped_start, b.clipped_end))
                .collect::<Vec<_>>()
        };
        assert_eq!(shape(&first), [(midnight - HOUR, midnight, false, true)]);
        assert_eq!(
            shape(&second),
            [(midnight, midnight + 2 * HOUR, true, false)]
        );
        assert_eq!(first.totals.recorded_ms, HOUR);
        assert_eq!(second.totals.recorded_ms, 2 * HOUR);
    }

    #[test]
    fn a_recording_block_carries_the_project_and_the_sorted_tag_names() {
        let mut input = input_with_recordings(vec![
            Recording {
                note: "fix login".into(),
                tag_ids: vec!["t-zeta".into(), "t-alpha".into(), "t-gone".into()],
                ..recording("late", mins(120), Some(mins(150)))
            },
            recording("early", mins(60), Some(mins(90))),
        ]);
        input.names.projects.insert(
            "p1".into(),
            ProjectRef {
                client_name: Some("Acme".into()),
                color: "teal".into(),
                shade: 7,
                ..project_ref("Portal")
            },
        );
        input.names.tags.insert("t-zeta".into(), "zeta".into());
        input.names.tags.insert("t-alpha".into(), "Alpha".into());

        let view = view(&input, &settings());

        assert_eq!(
            view.recordings,
            [
                RecordingBlock {
                    id: "early".into(),
                    start: mins(60),
                    end: mins(90),
                    project_id: "p1".into(),
                    project_name: "Portal".into(),
                    client_name: Some("Acme".into()),
                    color: "teal".into(),
                    shade: 7,
                    note: String::new(),
                    tags: vec![],
                    running: false,
                    clipped_start: false,
                    clipped_end: false,
                },
                RecordingBlock {
                    id: "late".into(),
                    start: mins(120),
                    end: mins(150),
                    project_id: "p1".into(),
                    project_name: "Portal".into(),
                    client_name: Some("Acme".into()),
                    color: "teal".into(),
                    shade: 7,
                    note: "fix login".into(),
                    tags: vec!["Alpha".into(), "zeta".into()],
                    running: false,
                    clipped_start: false,
                    clipped_end: false,
                },
            ]
        );
    }

    #[test]
    fn a_running_recording_runs_to_now_and_is_clipped_at_the_end_of_the_day_when_now_is_later() {
        for (now, expected_end, clipped_end) in [
            (mins(90), mins(90), false),
            (3 * DAY_LENGTH, DAY_LENGTH, true),
        ] {
            let input = DayInput {
                now,
                ..input_with_recordings(vec![recording("r", HOUR, None)])
            };

            let view = view(&input, &settings());

            let block = &view.recordings[0];
            assert_eq!(
                (block.start, block.end, block.running, block.clipped_end),
                (HOUR, expected_end, true, clipped_end),
                "now {now}"
            );
        }
    }

    #[test]
    fn a_recording_of_an_unknown_project_falls_back_to_graphite() {
        let input = input_with_recordings(vec![recording("r", HOUR, Some(2 * HOUR))]);

        let view = view(&input, &settings());

        let block = &view.recordings[0];
        assert_eq!(
            (block.project_name.as_str(), block.client_name.clone()),
            ("", None)
        );
        assert_eq!((block.color.as_str(), block.shade), ("graphite", 4));
    }

    #[test]
    fn tracked_ms_counts_overlapping_tracker_activity_once() {
        let activities = rows(&[
            (0, 60, "Code", "a"),
            (30, 90, "Mail", "b"),
            (120, 150, "Code", "c"),
        ]);

        assert_eq!(
            tracked_ms(&activities, (0, DAY_LENGTH), NOW),
            mins(90) + mins(30)
        );
    }

    #[test]
    fn tracked_ms_excludes_external_sources() {
        let activities = vec![
            activity(1, 0, Some(mins(60)), "Code", "a"),
            from_source(
                activity(2, mins(30), Some(mins(120)), "Cal", "m"),
                "external:calendar",
            ),
        ];

        assert_eq!(tracked_ms(&activities, (0, DAY_LENGTH), NOW), mins(60));
    }

    #[test]
    fn tracked_ms_is_clipped_to_the_bounds() {
        let activities = rows(&[
            (0, 60, "Code", "a"),
            (100, 200, "Code", "b"),
            (300, 400, "Code", "c"),
        ]);

        assert_eq!(
            tracked_ms(&activities, (mins(30), mins(150)), NOW),
            mins(30) + mins(50)
        );
    }

    #[test]
    fn tracked_ms_of_an_open_span_runs_to_now_within_the_bounds() {
        let activities = vec![activity(1, mins(60), None, "Code", "a")];

        for (now, bounds_end, expected) in [
            (mins(90), DAY_LENGTH, mins(30)),
            (3 * DAY_LENGTH, DAY_LENGTH, DAY_LENGTH - mins(60)),
            (mins(30), DAY_LENGTH, 0),
        ] {
            assert_eq!(
                tracked_ms(&activities, (0, bounds_end), now),
                expected,
                "now {now}"
            );
        }
    }

    #[test]
    fn tracked_ms_of_nothing_is_zero() {
        assert_eq!(tracked_ms(&[], (0, DAY_LENGTH), NOW), 0);
    }

    #[test]
    fn a_recording_block_serializes_with_camel_case_keys() {
        let mut input = input_with_recordings(vec![Recording {
            note: "n".into(),
            tag_ids: vec!["t1".into()],
            ..recording("r1", HOUR, Some(2 * HOUR))
        }]);
        input
            .names
            .projects
            .insert("p1".into(), project_ref("Portal"));
        input.names.tags.insert("t1".into(), "admin".into());

        let view = view(&input, &settings());

        assert_eq!(
            serde_json::to_value(&view.recordings).unwrap(),
            json!([{
                "id": "r1",
                "start": HOUR,
                "end": 2 * HOUR,
                "projectId": "p1",
                "projectName": "Portal",
                "clientName": null,
                "color": "blue",
                "shade": 4,
                "note": "n",
                "tags": ["admin"],
                "running": false,
                "clippedStart": false,
                "clippedEnd": false
            }])
        );
    }
}
