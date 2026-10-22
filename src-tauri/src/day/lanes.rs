//! Which lane a block goes to and its column inside the lane.

use std::collections::HashMap;

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
                    bundle_id: block.bundle_id.clone(),
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
            bundle_id: None,
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
            bundle_id: None,
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

#[cfg(test)]
mod tests {
    use crate::day::testing::*;

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

    #[test]
    fn only_app_lanes_carry_the_bundle_id_of_their_app() {
        let rows = vec![
            activity(1, 0, Some(mins(10)), "Mail", ""),
            with_bundle_id(activity(2, mins(10), Some(mins(20)), "Notes", ""), None),
            from_source(
                activity(3, mins(20), Some(mins(30)), "Code", ""),
                "external:planner",
            ),
            code(4, 30, 40),
        ];
        let settings = own_lane(own_lane(settings(), &bundle_of("Mail")), "Notes");

        let view = render(rows, &settings);

        let bundle_ids: Vec<(&str, Option<&str>)> = view
            .lanes
            .iter()
            .map(|l| (l.id.as_str(), l.bundle_id.as_deref()))
            .collect();
        assert_eq!(
            bundle_ids,
            [
                ("activity", None),
                ("app:test.mail", Some("test.mail")),
                ("app:Notes", None),
                ("source:planner", None),
            ]
        );
    }
}
