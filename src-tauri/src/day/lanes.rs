//! Which lane a block goes to, its column inside the lane, and the summary lane.

use std::collections::HashMap;

use super::transform::{Piece, joined};
use super::{ActivityBlock, EXTERNAL_PREFIX, Lane, LaneKind, ViewSettings};
use crate::db::time::Ms;

/// The activity lane (unless the settings leave it out), then app lanes by first appearance, then source
/// lanes by name; each packed.
pub(super) fn lanes(
    mut tracker: Vec<ActivityBlock>,
    external: Vec<ActivityBlock>,
    settings: &ViewSettings,
) -> Vec<Lane> {
    let mut activity = Vec::new();
    let mut app_lanes: Vec<Lane> = Vec::new();
    tracker.sort_by_key(|b| b.start);
    for block in tracker {
        if settings.app(&block.app_key).is_some_and(|a| a.own_lane) {
            match app_lanes
                .iter_mut()
                .find(|l| l.app_key.as_ref() == Some(&block.app_key))
            {
                Some(lane) => lane.blocks.push(block),
                None => app_lanes.push(Lane {
                    id: format!("app:{}", block.app_key),
                    kind: LaneKind::App,
                    label: block.app.clone(),
                    app_key: Some(block.app_key.clone()),
                    blocks: vec![block],
                }),
            }
        } else {
            activity.push(block);
        }
    }
    let mut sources: HashMap<String, Vec<ActivityBlock>> = HashMap::new();
    for block in external {
        let name = block
            .source
            .strip_prefix(EXTERNAL_PREFIX)
            .unwrap_or(&block.source);
        sources.entry(name.to_string()).or_default().push(block);
    }
    let mut source_lanes: Vec<Lane> = sources
        .into_iter()
        .map(|(name, blocks)| Lane {
            id: format!("source:{name}"),
            kind: LaneKind::Source,
            label: name,
            app_key: None,
            blocks: packed(blocks),
        })
        .collect();
    source_lanes.sort_by(|a, b| {
        a.label
            .to_lowercase()
            .cmp(&b.label.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut lanes = Vec::new();
    if settings.timeline.show_all_activity {
        lanes.push(Lane {
            id: "activity".into(),
            kind: LaneKind::Activity,
            label: "Activity".into(),
            app_key: None,
            blocks: packed(activity),
        });
    }
    lanes.extend(app_lanes.into_iter().map(|lane| Lane {
        blocks: packed(lane.blocks),
        ..lane
    }));
    lanes.extend(source_lanes);
    lanes
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

/// A key's part of one summary interval.
#[derive(Clone, Copy)]
struct Share {
    time: Ms,
    first_start: Ms,
}

/// The day cut into intervals of `interval` from its start; each interval goes to the key with the most
/// time in it (on a tie, the one that started first). A run of intervals with one winner is one block,
/// from the winner's first start to its last end inside the run.
pub(super) fn summary(pieces: &[Piece], interval: Ms, bounds: (Ms, Ms)) -> Lane {
    let count = usize::try_from((bounds.1 - bounds.0 + interval - 1) / interval).unwrap_or(0);
    let interval_at = |i: usize| {
        let start = bounds.0 + interval * i as Ms;
        (start, (start + interval).min(bounds.1))
    };
    let index_of = |ms: Ms| usize::try_from((ms - bounds.0) / interval).unwrap_or(0);

    let mut shares: Vec<HashMap<&str, Share>> = vec![HashMap::new(); count];
    for piece in pieces {
        let block = &piece.block;
        for (i, share) in shares
            .iter_mut()
            .enumerate()
            .take(index_of(block.end - 1) + 1)
            .skip(index_of(block.start))
        {
            let (from, to) = interval_at(i);
            let entry = share.entry(&piece.key).or_insert(Share {
                time: 0,
                first_start: Ms::MAX,
            });
            entry.time += block.end.min(to) - block.start.max(from);
            entry.first_start = entry.first_start.min(block.start);
        }
    }
    let winners: Vec<Option<&str>> = shares
        .iter()
        .map(|share| {
            share
                .iter()
                .max_by(|(a_key, a), (b_key, b)| {
                    a.time
                        .cmp(&b.time)
                        .then(b.first_start.cmp(&a.first_start))
                        .then(b_key.cmp(a_key))
                })
                .map(|(key, _)| *key)
        })
        .collect();

    let mut blocks = Vec::new();
    let mut i = 0;
    while i < count {
        let Some(key) = winners[i] else {
            i += 1;
            continue;
        };
        let first = i;
        while i + 1 < count && winners[i + 1] == Some(key) {
            i += 1;
        }
        let (from, to) = (interval_at(first).0, interval_at(i).1);
        let members: Vec<Piece> = pieces
            .iter()
            .filter(|p| p.key == key && p.block.start < to && p.block.end > from)
            .cloned()
            .collect();
        let start = members
            .iter()
            .map(|p| p.block.start)
            .min()
            .unwrap_or(from)
            .max(from);
        let end = members
            .iter()
            .map(|p| p.block.end)
            .max()
            .unwrap_or(to)
            .min(to);
        blocks.push(ActivityBlock {
            id: format!("s{start}"),
            col: 0,
            cols: 1,
            ..joined(members, (start, end))
        });
        i += 1;
    }
    Lane {
        id: "summary".into(),
        kind: LaneKind::Summary,
        label: "Summary".into(),
        app_key: None,
        blocks,
    }
}

#[cfg(test)]
mod tests {
    use crate::day::LaneKind;
    use crate::day::testing::*;
    use crate::store::settings::Noise;

    fn summarizing(minutes: u16) -> crate::day::ViewSettings {
        with_timeline(settings(), |t| {
            t.summary_lane = true;
            t.summary_minutes = minutes;
        })
    }

    #[test]
    fn the_summary_lane_comes_first_with_single_column_blocks_named_by_their_start() {
        let view = render(rows(&[(5, 10, "Code", "")]), &summarizing(15));

        assert_eq!(lane_id_list(&view), ["summary", "activity"]);
        let lane = lane_by_id(&view, "summary");
        assert_eq!(lane.kind, LaneKind::Summary);
        assert_eq!(lane.label, "Summary");
        assert_eq!(lane.app_key, None);
        let block = &lane.blocks[0];
        assert_eq!(
            (block.id.as_str(), block.col, block.cols),
            (format!("s{}", mins(5)).as_str(), 0, 1)
        );
        assert_eq!((block.start, block.end), (mins(5), mins(10)));
    }

    #[test]
    fn without_the_summary_setting_there_is_no_summary_lane() {
        let view = render(rows(&[(0, 10, "Code", "")]), &settings());

        assert_eq!(lane_id_list(&view), ["activity"]);
    }

    #[test]
    fn an_interval_goes_to_the_key_with_the_most_time_and_the_block_is_clipped_to_the_winner() {
        let rows = rows(&[(0, 8, "Code", ""), (8, 30, "Mail", "")]);

        let view = render(rows, &summarizing(15));

        assert_eq!(
            shape(lane_by_id(&view, "summary")),
            [(0, 8, vec![1]), (15, 30, vec![2])]
        );
    }

    #[test]
    fn on_a_tie_the_key_that_started_first_wins() {
        let rows = rows(&[(5, 10, "Mail", ""), (0, 5, "Code", "")]);

        let view = render(rows, &summarizing(15));

        assert_eq!(shape(lane_by_id(&view, "summary")), [(0, 5, vec![2])]);
    }

    #[test]
    fn adjacent_intervals_with_one_winner_merge_into_one_block_with_all_its_rows() {
        let rows = rows(&[(0, 20, "Code", ""), (25, 40, "Code", "")]);

        let view = render(rows, &summarizing(15));

        assert_eq!(shape(lane_by_id(&view, "summary")), [(0, 40, vec![1, 2])]);
    }

    #[test]
    fn different_winners_stay_apart_and_empty_intervals_have_no_block() {
        let rows = rows(&[(0, 10, "Code", ""), (50, 60, "Mail", "")]);

        let view = render(rows, &summarizing(15));

        assert_eq!(
            shape(lane_by_id(&view, "summary")),
            [(0, 10, vec![1]), (50, 60, vec![2])]
        );
        assert!(
            lane_by_id(&render(vec![], &summarizing(15)), "summary")
                .blocks
                .is_empty()
        );
    }

    #[test]
    fn the_interval_length_decides_where_the_blocks_split() {
        let cases = [
            (5, vec![(0, 10, vec![1]), (10, 20, vec![2])]),
            (15, vec![(0, 10, vec![1]), (15, 20, vec![2])]),
            (30, vec![(0, 10, vec![1])]),
        ];

        for (minutes, expected) in cases {
            let view = render(
                rows(&[(0, 10, "Code", ""), (10, 20, "Mail", "")]),
                &summarizing(minutes),
            );

            assert_eq!(
                shape(lane_by_id(&view, "summary")),
                expected,
                "{minutes} minutes"
            );
        }
    }

    #[test]
    fn grouping_by_app_decides_whether_titles_compete_or_add_up() {
        let cases = [
            (true, vec![(0, 10, vec![1, 2])]),
            (false, vec![(0, 5, vec![1])]),
        ];

        for (group_by_app, expected) in cases {
            let rows = rows(&[
                (0, 5, "Code", "a.rs"),
                (5, 10, "Code", "b.rs"),
                (10, 14, "Mail", ""),
            ]);
            let settings = with_timeline(summarizing(15), |t| t.group_by_app = group_by_app);

            let view = render(rows, &settings);

            assert_eq!(
                shape(lane_by_id(&view, "summary")),
                expected,
                "group by app {group_by_app}"
            );
        }
    }

    #[test]
    fn the_summary_counts_own_lane_apps_and_noise_but_not_hidden_apps() {
        let rows = rows(&[
            (0, 10, "Mail", ""),
            (20, 30, "Notes", ""),
            (40, 41, "Code", ""),
        ]);
        let settings = with_timeline(
            own_lane(
                hidden(summarizing(15), &bundle_of("Notes")),
                &bundle_of("Mail"),
            ),
            |t| t.noise = Noise::Low,
        );

        let view = render(rows, &settings);

        assert_eq!(
            shape(lane_by_id(&view, "summary")),
            [(0, 10, vec![1]), (40, 41, vec![3])]
        );
        assert!(lane_by_id(&view, "activity").blocks.is_empty());
        assert_eq!(
            shape(lane_by_id(&view, "app:test.mail")),
            [(0, 10, vec![1])]
        );
    }

    #[test]
    fn leaving_out_the_activity_lane_keeps_the_summary_app_and_source_lanes() {
        let rows = vec![
            activity(1, 0, Some(mins(10)), "Code", ""),
            activity(2, mins(10), Some(mins(20)), "Mail", ""),
            from_source(
                activity(3, mins(20), Some(mins(30)), "Code", ""),
                "external:planner",
            ),
        ];
        let settings = with_timeline(own_lane(summarizing(15), &bundle_of("Mail")), |t| {
            t.show_all_activity = false
        });

        let view = render(rows.clone(), &settings);
        let without_own_lanes = render(
            rows,
            &with_timeline(summarizing(15), |t| t.show_all_activity = false),
        );

        assert_eq!(
            lane_id_list(&view),
            ["summary", "app:test.mail", "source:planner"]
        );
        assert_eq!(
            lane_id_list(&without_own_lanes),
            ["summary", "source:planner"]
        );
    }

    #[test]
    fn app_lanes_follow_the_first_appearance_of_their_app_not_its_name() {
        let rows = rows(&[(0, 5, "Zed", ""), (5, 10, "Alpha", ""), (20, 25, "Zed", "")]);
        let settings = own_lane(own_lane(settings(), &bundle_of("Alpha")), &bundle_of("Zed"));

        let view = render(rows, &settings);

        assert_eq!(
            lane_id_list(&view),
            ["activity", "app:test.zed", "app:test.alpha"]
        );
        assert_eq!(
            shape(lane_by_id(&view, "app:test.zed")),
            [(0, 5, vec![1]), (20, 25, vec![3])]
        );
        assert!(lane_by_id(&view, "activity").blocks.is_empty());
    }
}
