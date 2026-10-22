//! The timeline settings as pure transforms of the day's tracker rows: hide apps, decide which rows are
//! "the same", merge them across short gaps, drop noise. Captured data never changes.

use std::collections::HashMap;

use super::totals::union_length;
use super::{ActivityBlock, PRIVATE_TITLE, ViewSettings};
use crate::db::away::AwayReason;
use crate::db::time::Ms;
use crate::store::settings::{Continuity, Noise, Titles};

const MIN: Ms = 60_000;

/// One row cut to the day, with the identity that decides which rows join one block.
#[derive(Debug, Clone)]
pub(super) struct Piece {
    pub key: String,
    pub block: ActivityBlock,
}

pub(crate) fn continuity_ms(continuity: Continuity) -> Ms {
    match continuity {
        Continuity::Off => 0,
        Continuity::Short => 2 * MIN,
        Continuity::Regular => 8 * MIN,
        Continuity::Extended => 16 * MIN,
    }
}

pub(super) fn noise_ms(noise: Noise) -> Ms {
    match noise {
        Noise::Off => 0,
        Noise::Low => 2 * MIN,
        Noise::Medium => 4 * MIN,
        Noise::High => 6 * MIN,
    }
}

/// Idle time is bridged like a gap; sleep, a locked screen or a pause end a block.
pub(crate) fn breaks_continuity(reason: AwayReason) -> bool {
    !matches!(reason, AwayReason::Idle)
}

pub(super) fn without_hidden(
    blocks: Vec<ActivityBlock>,
    settings: &ViewSettings,
) -> Vec<ActivityBlock> {
    blocks
        .into_iter()
        .filter(|b| !settings.app(&b.app_key).is_some_and(|a| a.hidden))
        .collect()
}

/// Labels each row by the effective title mode and keys it by app (group by app) or by app and label.
/// Private rows read "Private" and never join the app's other rows.
pub(super) fn identified(blocks: Vec<ActivityBlock>, settings: &ViewSettings) -> Vec<Piece> {
    let mut pieces: Vec<Piece> = blocks
        .into_iter()
        .map(|mut block| {
            let app = settings.app(&block.app_key);
            let titles = app
                .and_then(|a| a.titles)
                .unwrap_or(settings.timeline.titles);
            let by_app = app
                .and_then(|a| a.group_by_app)
                .unwrap_or(settings.timeline.group_by_app);
            block.label = label_of(&block, titles);
            let key = key_of(&block, by_app);
            Piece { key, block }
        })
        .collect();
    pieces.sort_by_key(|p| (p.block.start, p.block.end));
    pieces
}

fn label_of(block: &ActivityBlock, titles: Titles) -> String {
    if block.private {
        PRIVATE_TITLE.into()
    } else if titles == Titles::App || block.title.is_empty() {
        block.app.clone()
    } else {
        block.title.clone()
    }
}

/// Private rows end in a separator of their own, so no window title can share their key.
fn key_of(block: &ActivityBlock, by_app: bool) -> String {
    if block.private {
        format!("{}\u{1}", block.app_key)
    } else if by_app {
        block.app_key.clone()
    } else {
        format!("{}\u{0}{}", block.app_key, block.label)
    }
}

struct Run {
    start: Ms,
    end: Ms,
    pieces: Vec<Piece>,
}

/// Rows of one key merge across gaps shorter than the continuity (other keys' rows in the gap stay their
/// own blocks); then blocks shorter than the noise threshold are dropped.
pub(super) fn grouped(
    pieces: Vec<Piece>,
    settings: &ViewSettings,
    barriers: &[(Ms, Ms)],
) -> Vec<ActivityBlock> {
    let bridge = continuity_ms(settings.timeline.continuity);
    let noise = noise_ms(settings.timeline.noise);
    let mut runs: Vec<Run> = Vec::new();
    let mut latest_run: HashMap<String, usize> = HashMap::new();
    for piece in pieces {
        if let Some(&i) = latest_run.get(&piece.key) {
            let run = &mut runs[i];
            let gap = piece.block.start - run.end;
            if gap <= 0 || (gap < bridge && !crosses(barriers, run.end, piece.block.start)) {
                run.end = run.end.max(piece.block.end);
                run.pieces.push(piece);
                continue;
            }
        }
        latest_run.insert(piece.key.clone(), runs.len());
        runs.push(Run {
            start: piece.block.start,
            end: piece.block.end,
            pieces: vec![piece],
        });
    }
    runs.into_iter()
        .filter(|r| r.end - r.start >= noise)
        .map(|r| joined(r.pieces, (r.start, r.end)))
        .collect()
}

fn crosses(barriers: &[(Ms, Ms)], from: Ms, to: Ms) -> bool {
    barriers
        .iter()
        .any(|&(start, end)| start < to && end > from)
}

/// One block over `span` for `pieces`, styled as the longest; `active_ms` is the time the pieces fill of it.
pub(super) fn joined(pieces: Vec<Piece>, span: (Ms, Ms)) -> ActivityBlock {
    let active_ms = union_length(
        pieces
            .iter()
            .map(|p| (p.block.start.max(span.0), p.block.end.min(span.1)))
            .filter(|(start, end)| start < end)
            .collect(),
    );
    let longest = pieces
        .iter()
        .max_by_key(|p| {
            (
                p.block.end - p.block.start,
                std::cmp::Reverse(p.block.start),
            )
        })
        .expect("a block joins at least one piece");
    let members: Vec<i64> = pieces
        .iter()
        .flat_map(|p| p.block.members.iter().copied())
        .collect();
    ActivityBlock {
        id: format!("a{}", members[0]),
        start: span.0,
        end: span.1,
        active_ms,
        members,
        ..longest.block.clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::day::testing::*;
    use crate::day::{DayView, HiddenApp, view};
    use crate::db::activities::ActivityRow;
    use crate::db::away::{AwayReason, AwayRow};
    use crate::db::time::Ms;
    use crate::store::settings::{Continuity, Noise, Titles};

    fn with_continuity(continuity: Continuity) -> crate::day::ViewSettings {
        with_timeline(settings(), |t| t.continuity = continuity)
    }

    fn activity_members(view: &DayView) -> Vec<Vec<i64>> {
        lane_by_id(view, "activity")
            .blocks
            .iter()
            .map(|b| b.members.clone())
            .collect()
    }

    fn labels(view: &DayView) -> Vec<&str> {
        lane_by_id(view, "activity")
            .blocks
            .iter()
            .map(|b| b.label.as_str())
            .collect()
    }

    fn private_at(mut rows: Vec<ActivityRow>, index: usize) -> Vec<ActivityRow> {
        rows[index] = private(rows[index].clone());
        rows
    }

    fn without_bundle_ids(rows: Vec<ActivityRow>) -> Vec<ActivityRow> {
        rows.into_iter().map(|r| with_bundle_id(r, None)).collect()
    }

    fn two_code_rows(gap: Ms) -> Vec<ActivityRow> {
        vec![
            activity(1, 0, Some(mins(10)), "Code", ""),
            activity(2, mins(10) + gap, Some(mins(20) + gap), "Code", ""),
        ]
    }

    fn merged_or_apart(bridged: bool) -> Vec<Vec<i64>> {
        if bridged {
            vec![vec![1, 2]]
        } else {
            vec![vec![1], vec![2]]
        }
    }

    #[test]
    fn hidden_app_rows_are_dropped_from_every_lane_but_still_count_as_tracked_time() {
        let rows = rows(&[
            (0, 10, "Code", ""),
            (10, 20, "Mail", ""),
            (20, 30, "Code", ""),
        ]);
        let settings = with_timeline(hidden(settings(), &bundle_of("Mail")), |t| {
            t.summary_lane = true
        });

        let view = render(rows, &settings);

        assert_eq!(
            shape(lane_by_id(&view, "summary")),
            [(0, 10, vec![1]), (20, 30, vec![3])]
        );
        assert_eq!(
            shape(lane_by_id(&view, "activity")),
            [(0, 10, vec![1]), (20, 30, vec![3])]
        );
        assert!(
            view.lanes
                .iter()
                .flat_map(|l| &l.blocks)
                .all(|b| b.app != "Mail")
        );
        assert_eq!(view.totals.tracked_ms, mins(30));
    }

    #[test]
    fn external_sources_are_never_hidden() {
        let rows = vec![from_source(
            activity(1, 0, Some(mins(10)), "Mail", ""),
            "external:planner",
        )];
        let settings = hidden(settings(), &bundle_of("Mail"));

        let view = render(rows, &settings);

        assert_eq!(
            shape(lane_by_id(&view, "source:planner")),
            [(0, 10, vec![1])]
        );
        assert_eq!(view.sources, ["external:planner"]);
    }

    #[test]
    fn a_block_is_labeled_by_the_title_mode_and_private_rows_read_private() {
        let cases = [
            ("app mode", Titles::App, "main.rs", false, "Code"),
            (
                "document mode",
                Titles::Document,
                "main.rs",
                false,
                "main.rs",
            ),
            (
                "empty title in document mode",
                Titles::Document,
                "",
                false,
                "Code",
            ),
            ("empty title in app mode", Titles::App, "", false, "Code"),
            (
                "private in document mode",
                Titles::Document,
                "Bank",
                true,
                "Private",
            ),
            ("private in app mode", Titles::App, "Bank", true, "Private"),
        ];

        for (name, titles, title, is_private, expected) in cases {
            let mut rows = rows(&[(0, 10, "Code", title)]);
            if is_private {
                rows = private_at(rows, 0);
            }
            let settings = with_timeline(settings(), |t| t.titles = titles);

            let view = render(rows, &settings);

            assert_eq!(labels(&view), [expected], "{name}");
        }
    }

    #[test]
    fn private_rows_form_their_own_block_apart_from_the_apps_other_rows() {
        let build = || {
            private_at(
                rows(&[
                    (0, 10, "Safari", "a"),
                    (10, 20, "Safari", "x"),
                    (20, 30, "Safari", "b"),
                ]),
                1,
            )
        };
        let cases = [
            (true, vec![(0, 30, vec![1, 3]), (10, 20, vec![2])]),
            (
                false,
                vec![(0, 10, vec![1]), (10, 20, vec![2]), (20, 30, vec![3])],
            ),
        ];

        for (group_by_app, expected) in cases {
            let settings = with_timeline(settings(), |t| {
                t.group_by_app = group_by_app;
                t.continuity = Continuity::Extended;
            });

            let view = render(build(), &settings);

            let lane = lane_by_id(&view, "activity");
            assert_eq!(shape(lane), expected, "group by app {group_by_app}");
            assert!(
                lane.blocks
                    .iter()
                    .filter(|b| b.private)
                    .all(|b| b.label == "Private" && b.members == [2]),
                "group by app {group_by_app}"
            );
        }
    }

    #[test]
    fn group_by_app_merges_the_titles_of_one_app_into_the_block_of_the_longer_one() {
        let rows = || rows(&[(0, 10, "Code", "a.rs"), (10, 30, "Code", "b.rs")]);

        let on = render(
            rows(),
            &with_timeline(settings(), |t| t.group_by_app = true),
        );
        let off = render(
            rows(),
            &with_timeline(settings(), |t| t.group_by_app = false),
        );

        let merged = &lane_by_id(&on, "activity").blocks;
        assert_eq!(shape(lane_by_id(&on, "activity")), [(0, 30, vec![1, 2])]);
        assert_eq!(
            (merged[0].title.as_str(), merged[0].label.as_str()),
            ("b.rs", "b.rs")
        );
        assert_eq!(
            shape(lane_by_id(&off, "activity")),
            [(0, 10, vec![1]), (10, 30, vec![2])]
        );
    }

    #[test]
    fn a_per_app_group_by_app_override_wins_over_the_global_setting_for_that_app_only() {
        let rows = || {
            rows(&[
                (0, 10, "Code", "a"),
                (10, 20, "Code", "b"),
                (20, 30, "Mail", "a"),
                (30, 40, "Mail", "b"),
            ])
        };
        let cases = [
            ("global off, Code on", false, "test.code", true),
            ("global on, Mail off", true, "test.mail", false),
        ];

        for (name, global, key, over) in cases {
            let settings = with_override(
                with_timeline(settings(), |t| t.group_by_app = global),
                key,
                |a| a.group_by_app = Some(over),
            );

            let view = render(rows(), &settings);

            assert_eq!(
                shape(lane_by_id(&view, "activity")),
                [(0, 20, vec![1, 2]), (20, 30, vec![3]), (30, 40, vec![4])],
                "{name}"
            );
        }
    }

    #[test]
    fn a_per_app_titles_override_wins_over_the_global_setting_for_that_app_only() {
        let rows = || rows(&[(0, 10, "Code", "main.rs"), (10, 20, "Mail", "inbox")]);
        let cases = [
            (Titles::Document, "test.code", Titles::App),
            (Titles::App, "test.mail", Titles::Document),
        ];

        for (global, key, over) in cases {
            let settings =
                with_override(with_timeline(settings(), |t| t.titles = global), key, |a| {
                    a.titles = Some(over)
                });

            let view = render(rows(), &settings);

            assert_eq!(
                labels(&view),
                ["Code", "inbox"],
                "{global:?} with {key} {over:?}"
            );
        }
    }

    #[test]
    fn overrides_of_rows_without_a_bundle_id_are_keyed_by_the_app_name() {
        let bundle_key = bundle_of("Code");
        let cases = [("Code", true), (bundle_key.as_str(), false)];

        for (key, merged) in cases {
            let rows = without_bundle_ids(rows(&[(0, 10, "Code", "a"), (10, 20, "Code", "b")]));
            let settings = with_override(settings(), key, |a| a.group_by_app = Some(true));

            let view = render(rows, &settings);

            assert_eq!(
                activity_members(&view),
                merged_or_apart(merged),
                "key {key}"
            );
        }
    }

    #[test]
    fn continuity_bridges_a_gap_below_its_threshold_and_not_one_at_or_above_it() {
        let cases = [
            (Continuity::Off, 0, true),
            (Continuity::Off, 1, false),
            (Continuity::Short, 2 * MIN - 1, true),
            (Continuity::Short, 2 * MIN, false),
            (Continuity::Regular, 8 * MIN - 1, true),
            (Continuity::Regular, 8 * MIN, false),
            (Continuity::Extended, 16 * MIN - 1, true),
            (Continuity::Extended, 16 * MIN, false),
        ];

        for (continuity, gap, bridged) in cases {
            let view = render(two_code_rows(gap), &with_continuity(continuity));

            assert_eq!(
                activity_members(&view),
                merged_or_apart(bridged),
                "{continuity:?} with a gap of {gap} ms"
            );
            if bridged {
                let block = &lane_by_id(&view, "activity").blocks[0];
                assert_eq!((block.start, block.end), (0, mins(20) + gap));
            }
        }
    }

    #[test]
    fn active_ms_of_a_joined_block_excludes_bridged_gaps_and_counts_overlaps_once() {
        let cases = [
            ("single row", vec![(0, 10)], 10),
            ("bridged gap", vec![(0, 10), (20, 30)], 20),
            ("overlapping rows", vec![(0, 10), (5, 15)], 15),
        ];

        for (name, spans, expected) in cases {
            let rows = spans
                .iter()
                .enumerate()
                .map(|(i, &(s, e))| code(i as i64 + 1, s, e))
                .collect();

            let view = render(rows, &with_continuity(Continuity::Extended));

            let lane = lane_by_id(&view, "activity");
            assert_eq!(lane.blocks.len(), 1, "{name}");
            assert_eq!(lane.blocks[0].active_ms, mins(expected), "{name}");
        }
    }

    #[test]
    fn a_bridged_app_keeps_the_block_of_the_app_in_between_and_overlaps_it() {
        let rows = rows(&[
            (0, 10, "Code", ""),
            (10, 14, "Mail", ""),
            (14, 20, "Code", ""),
        ]);

        let view = render(rows, &with_continuity(Continuity::Regular));

        let lane = lane_by_id(&view, "activity");
        assert_eq!(shape(lane), [(0, 20, vec![1, 3]), (10, 14, vec![2])]);
        let columns: Vec<(u8, u8)> = lane.blocks.iter().map(|b| (b.col, b.cols)).collect();
        assert_eq!(columns, [(0, 2), (1, 2)]);
    }

    #[test]
    fn sleep_lock_and_pause_end_a_block_but_idle_is_bridged() {
        let away_at = |start: i64, end: i64, reason: AwayReason| -> AwayRow {
            away(1, mins(start), Some(mins(end)), reason)
        };
        let cases = [
            ("sleep", away_at(10, 20, AwayReason::Sleep), false),
            ("lock", away_at(10, 20, AwayReason::Lock), false),
            ("pause", away_at(10, 20, AwayReason::Paused), false),
            (
                "sleep reaching into the gap",
                away_at(15, 25, AwayReason::Sleep),
                false,
            ),
            ("idle", away_at(10, 20, AwayReason::Idle), true),
            (
                "sleep after the gap",
                away_at(40, 50, AwayReason::Sleep),
                true,
            ),
        ];

        for (name, away_row, bridged) in cases {
            let rows = rows(&[(0, 10, "Code", ""), (20, 30, "Code", "")]);

            let view = view(
                &input(rows, vec![away_row]),
                &with_continuity(Continuity::Extended),
            );

            assert_eq!(activity_members(&view), merged_or_apart(bridged), "{name}");
        }
    }

    #[test]
    fn noise_drops_blocks_shorter_than_its_threshold_and_keeps_the_time_tracked() {
        let cases = [
            (Noise::Off, 1, true),
            (Noise::Low, 2 * MIN - 1, false),
            (Noise::Low, 2 * MIN, true),
            (Noise::Medium, 4 * MIN - 1, false),
            (Noise::Medium, 4 * MIN, true),
            (Noise::High, 6 * MIN - 1, false),
            (Noise::High, 6 * MIN, true),
        ];

        for (noise, length, kept) in cases {
            let rows = vec![activity(1, 0, Some(length), "Code", "")];
            let settings = with_timeline(settings(), |t| t.noise = noise);

            let view = render(rows, &settings);

            assert_eq!(
                lane_by_id(&view, "activity").blocks.len(),
                usize::from(kept),
                "{noise:?} with a block of {length} ms"
            );
            assert_eq!(view.totals.tracked_ms, length, "{noise:?} {length}");
        }
    }

    #[test]
    fn noise_is_judged_after_bridging_and_leaves_tracked_time_and_gaps_alone() {
        let rows = || rows(&[(0, 1, "Code", ""), (2, 3, "Code", ""), (10, 11, "Mail", "")]);
        let denoised = with_timeline(settings(), |t| {
            t.continuity = Continuity::Short;
            t.noise = Noise::Low;
        });
        let unbridged = with_timeline(settings(), |t| t.noise = Noise::Low);

        let bridged = render(rows(), &denoised);
        let alone = render(rows(), &unbridged);

        assert_eq!(
            shape(lane_by_id(&bridged, "activity")),
            [(0, 3, vec![1, 2])]
        );
        assert!(lane_by_id(&alone, "activity").blocks.is_empty());
        for view in [&bridged, &alone] {
            assert_eq!(view.totals.tracked_ms, mins(3));
            let gaps: Vec<(Ms, Ms)> = view.gaps.iter().map(|g| (g.start, g.end)).collect();
            assert_eq!(gaps, [(mins(3), mins(10))]);
        }
    }

    #[test]
    fn the_view_copies_the_compact_setting() {
        for compact in [true, false] {
            let view = render(vec![], &with_timeline(settings(), |t| t.compact = compact));

            assert_eq!(view.compact, compact);
        }
    }

    #[test]
    fn hidden_apps_lists_every_hidden_override_by_name_taking_the_name_from_the_days_rows() {
        let rows = vec![
            activity(1, 0, Some(mins(10)), "Mail", ""),
            with_bundle_id(activity(2, mins(10), Some(mins(20)), "Notes", ""), None),
        ];
        let settings = [
            bundle_of("Mail").as_str(),
            "Notes",
            "zed.app",
            "alpha.app",
            "Beta",
        ]
        .into_iter()
        .fold(own_lane(settings(), &bundle_of("Code")), hidden);

        let view = render(rows, &settings);

        let hidden_app = |key: &str, app: &str| HiddenApp {
            app_key: key.into(),
            app: app.into(),
        };
        assert_eq!(
            view.hidden_apps,
            [
                hidden_app("alpha.app", "alpha.app"),
                hidden_app("Beta", "Beta"),
                hidden_app("test.mail", "Mail"),
                hidden_app("Notes", "Notes"),
                hidden_app("zed.app", "zed.app"),
            ]
        );
    }
}
