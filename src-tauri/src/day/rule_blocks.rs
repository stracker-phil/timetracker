//! What rules match (ADR 0027): the tracker's summary by document, so each stretch counts as what was mostly
//! done in it, and external activity as it is.

use super::summary::summary;
use super::totals::union;
use super::transform::identified;
use super::{ActivityBlock, ViewSettings};
use crate::db::time::Ms;
use crate::store::settings::Titles;

/// The summary is keyed by document whatever the timeline shows, so title conditions see each window. Its
/// blocks lose the idle edges of their buckets and any sleep, lock or pause inside; `external` stays as is.
pub(super) fn rule_blocks(
    tracker: &[ActivityBlock],
    external: &[ActivityBlock],
    barriers: &[(Ms, Ms)],
    bounds: (Ms, Ms),
    settings: &ViewSettings,
) -> Vec<ActivityBlock> {
    let mut by_document = settings.clone();
    by_document.timeline.titles = Titles::Document;
    by_document.timeline.group_by_app = false;
    let pieces = identified(tracker.to_vec(), &by_document);
    let bucket = Ms::from(settings.timeline.summary_minutes) * 60_000;
    let active = union(tracker.iter().map(|b| (b.start, b.end)).collect());

    let mut blocks: Vec<ActivityBlock> = summary(&pieces, bucket, bounds)
        .blocks
        .into_iter()
        .flat_map(|block| worked_parts(block, &active, barriers))
        .collect();
    blocks.extend(external.iter().cloned());
    blocks.sort_by_key(|b| (b.start, b.end));
    blocks
}

/// `block` from its first activity to its last, without the barriers inside. `active` is sorted and disjoint.
fn worked_parts(
    block: ActivityBlock,
    active: &[(Ms, Ms)],
    barriers: &[(Ms, Ms)],
) -> Vec<ActivityBlock> {
    let inside: Vec<&(Ms, Ms)> = active
        .iter()
        .filter(|&&(start, end)| start < block.end && end > block.start)
        .collect();
    let (Some(first), Some(last)) = (inside.first(), inside.last()) else {
        return Vec::new();
    };
    let worked = (first.0.max(block.start), last.1.min(block.end));
    let parts = barriers
        .iter()
        .fold(vec![worked], |parts, &cut| without(parts, cut));
    parts
        .into_iter()
        .map(|(start, end)| ActivityBlock {
            start,
            end,
            ..block.clone()
        })
        .collect()
}

/// `parts` with `cut` taken out.
fn without(parts: Vec<(Ms, Ms)>, (cut_start, cut_end): (Ms, Ms)) -> Vec<(Ms, Ms)> {
    parts
        .into_iter()
        .flat_map(|(start, end)| [(start, end.min(cut_start)), (start.max(cut_end), end)])
        .filter(|(start, end)| start < end)
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::day::DayView;
    use crate::day::testing::*;
    use crate::db::away::AwayReason;
    use crate::db::time::Ms;
    use crate::store::settings::Titles;

    /// `(start_min, end_min, app, title)` of each rule block.
    fn blocks_of(view: &DayView) -> Vec<(Ms, Ms, &str, &str)> {
        view.rule_blocks
            .iter()
            .map(|b| (b.start / MIN, b.end / MIN, b.app.as_str(), b.title.as_str()))
            .collect()
    }

    fn five_minute_buckets() -> crate::day::ViewSettings {
        with_timeline(settings(), |t| t.summary_minutes = 5)
    }

    fn zoom_with_a_slack_glance() -> Vec<crate::db::activities::ActivityRow> {
        rows(&[
            (600, 606, "Zoom", ""),
            (606, 608, "Slack", ""),
            (608, 613, "Zoom", ""),
        ])
    }

    #[test]
    fn a_short_glance_at_another_app_is_absorbed_into_the_surrounding_work() {
        let view = render(zoom_with_a_slack_glance(), &five_minute_buckets());

        assert_eq!(blocks_of(&view), [(600, 613, "Zoom", "")]);
    }

    #[test]
    fn the_rules_see_the_summary_whether_or_not_the_summary_lane_is_shown() {
        for summary_lane in [false, true] {
            let settings = with_timeline(five_minute_buckets(), |t| t.summary_lane = summary_lane);

            let view = render(zoom_with_a_slack_glance(), &settings);

            assert_eq!(
                blocks_of(&view),
                [(600, 613, "Zoom", "")],
                "summary lane {summary_lane}"
            );
            assert_eq!(
                view.lanes.iter().any(|l| l.id == "summary"),
                summary_lane,
                "summary lane {summary_lane}"
            );
        }
    }

    #[test]
    fn rule_blocks_keep_each_windows_title_whatever_the_timeline_groups_or_shows() {
        let cases = [
            (true, Titles::App),
            (true, Titles::Document),
            (false, Titles::App),
            (false, Titles::Document),
        ];
        for (group_by_app, titles) in cases {
            let settings = with_timeline(five_minute_buckets(), |t| {
                t.group_by_app = group_by_app;
                t.titles = titles;
            });

            let view = render(
                rows(&[(600, 610, "Code", "a.rs"), (610, 615, "Code", "b.rs")]),
                &settings,
            );

            assert_eq!(
                blocks_of(&view),
                [(600, 610, "Code", "a.rs"), (610, 615, "Code", "b.rs")],
                "group by app {group_by_app}, titles {titles:?}"
            );
        }
    }

    #[test]
    fn a_block_runs_from_its_first_to_its_last_activity_not_to_the_bucket_edges() {
        let settings = with_timeline(settings(), |t| t.summary_minutes = 15);

        let view = render(rows(&[(603, 612, "Code", "")]), &settings);

        assert_eq!(blocks_of(&view), [(603, 612, "Code", "")]);
    }

    #[test]
    fn sleep_lock_and_pause_are_cut_out_of_a_block_but_idle_is_not() {
        let cases = [
            (AwayReason::Sleep, vec![(600, 610), (620, 630)]),
            (AwayReason::Lock, vec![(600, 610), (620, 630)]),
            (AwayReason::Paused, vec![(600, 610), (620, 630)]),
            (AwayReason::Idle, vec![(600, 630)]),
        ];
        for (reason, expected) in cases {
            let settings = with_timeline(settings(), |t| t.summary_minutes = 30);
            let input = input(
                rows(&[(600, 630, "Code", "")]),
                vec![away(1, mins(610), Some(mins(620)), reason)],
            );

            let view = crate::day::view(&input, &settings);

            let expected: Vec<_> = expected
                .into_iter()
                .map(|(start, end)| (start, end, "Code", ""))
                .collect();
            assert_eq!(blocks_of(&view), expected, "{reason:?}");
        }
    }

    #[test]
    fn external_activity_is_matched_on_its_own_next_to_the_summary() {
        let calendar = from_source(
            activity(10, mins(600), Some(mins(630)), "Calendar", "Standup"),
            "external:calendar",
        );
        let settings = with_timeline(settings(), |t| t.summary_minutes = 15);

        let view = render(vec![calendar, code(1, 603, 612)], &settings);

        assert_eq!(
            blocks_of(&view),
            [(600, 630, "Calendar", "Standup"), (603, 612, "Code", "")]
        );
        let sources: Vec<&str> = view.rule_blocks.iter().map(|b| b.source.as_str()).collect();
        assert_eq!(sources, ["external:calendar", "tracker"]);
    }

    #[test]
    fn the_bucket_size_follows_the_summary_minutes_setting() {
        let cases = [
            (5, vec![(0, 10, "Code"), (10, 20, "Mail")]),
            (15, vec![(0, 15, "Code"), (15, 20, "Mail")]),
            (30, vec![(0, 20, "Code")]),
        ];
        for (minutes, expected) in cases {
            let settings = with_timeline(settings(), |t| t.summary_minutes = minutes);

            let view = render(
                rows(&[(0, 10, "Code", ""), (10, 20, "Mail", "")]),
                &settings,
            );

            let expected: Vec<_> = expected
                .into_iter()
                .map(|(start, end, app)| (start, end, app, ""))
                .collect();
            assert_eq!(blocks_of(&view), expected, "{minutes} minutes");
        }
    }

    #[test]
    fn hidden_apps_take_no_part_in_the_rule_blocks() {
        let settings = hidden(five_minute_buckets(), &bundle_of("Notes"));

        let view = render(
            rows(&[(600, 610, "Notes", ""), (620, 630, "Mail", "")]),
            &settings,
        );

        assert_eq!(blocks_of(&view), [(620, 630, "Mail", "")]);
    }

    #[test]
    fn a_brief_moment_in_a_bucket_makes_no_rule_block() {
        let cases = [(40_000, true), (89_999, true), (90_000, false)];
        for (length, empty) in cases {
            let start = mins(600);
            let moment = activity(1, start, Some(start + length), "Zoom", "");

            let view = render(vec![moment], &five_minute_buckets());

            assert_eq!(view.rule_blocks.is_empty(), empty, "{length} ms");
        }
    }

    #[test]
    fn a_moment_next_to_real_work_adds_no_rule_block() {
        let rows = vec![
            code(1, 600, 610),
            activity(2, mins(610), Some(mins(610) + 40_000), "Slack", ""),
        ];

        let view = render(rows, &five_minute_buckets());

        assert_eq!(blocks_of(&view), [(600, 610, "Code", "")]);
    }

    #[test]
    fn a_day_without_activity_has_no_rule_blocks() {
        let view = render(Vec::new(), &five_minute_buckets());

        assert!(view.rule_blocks.is_empty());
    }
}
