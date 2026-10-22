//! The summary lane: the day in clock-aligned buckets, each won by the activity with the most time in it.

use std::collections::HashMap;

use super::transform::{Piece, joined};
use super::{ActivityBlock, Lane, LaneKind};
use crate::db::time::Ms;

/// A key's part of one bucket.
#[derive(Clone, Copy)]
struct Share {
    time: Ms,
    first_start: Ms,
}

/// The summary lane in buckets of `bucket` from the day's start: one block per run of buckets with one winner.
pub(super) fn summary(pieces: &[Piece], bucket: Ms, bounds: (Ms, Ms)) -> Lane {
    let buckets = Buckets { bucket, bounds };
    Lane {
        id: "summary".into(),
        kind: LaneKind::Summary,
        label: "Summary".into(),
        app_key: None,
        bundle_id: None,
        blocks: run_blocks(pieces, &bucket_winners(pieces, &buckets), &buckets),
    }
}

/// The day cut into buckets of `bucket` from its start, the last one ending with the day.
struct Buckets {
    bucket: Ms,
    bounds: (Ms, Ms),
}

impl Buckets {
    fn count(&self) -> usize {
        usize::try_from((self.bounds.1 - self.bounds.0 + self.bucket - 1) / self.bucket)
            .unwrap_or(0)
    }

    fn span(&self, i: usize) -> (Ms, Ms) {
        let start = self.bounds.0 + self.bucket * i as Ms;
        (start, (start + self.bucket).min(self.bounds.1))
    }

    fn index_of(&self, ms: Ms) -> usize {
        usize::try_from((ms - self.bounds.0) / self.bucket).unwrap_or(0)
    }
}

/// Each bucket's key with the most time in it (on a tie, the one that started first); `None` without activity.
fn bucket_winners<'a>(pieces: &'a [Piece], buckets: &Buckets) -> Vec<Option<&'a str>> {
    let mut shares: Vec<HashMap<&str, Share>> = vec![HashMap::new(); buckets.count()];
    for piece in pieces {
        let block = &piece.block;
        for (i, share) in shares
            .iter_mut()
            .enumerate()
            .take(buckets.index_of(block.end - 1) + 1)
            .skip(buckets.index_of(block.start))
        {
            let (from, to) = buckets.span(i);
            let entry = share.entry(&piece.key).or_insert(Share {
                time: 0,
                first_start: Ms::MAX,
            });
            entry.time += block.end.min(to) - block.start.max(from);
            entry.first_start = entry.first_start.min(block.start);
        }
    }
    shares
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
        .collect()
}

/// One block over each run of buckets with one winner, so every block starts and ends on the grid.
fn run_blocks(pieces: &[Piece], winners: &[Option<&str>], buckets: &Buckets) -> Vec<ActivityBlock> {
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < winners.len() {
        let Some(key) = winners[i] else {
            i += 1;
            continue;
        };
        let first = i;
        while i + 1 < winners.len() && winners[i + 1] == Some(key) {
            i += 1;
        }
        let (from, to) = (buckets.span(first).0, buckets.span(i).1);
        let members: Vec<Piece> = pieces
            .iter()
            .filter(|p| p.key == key && p.block.start < to && p.block.end > from)
            .cloned()
            .collect();
        blocks.push(ActivityBlock {
            id: format!("s{from}"),
            col: 0,
            cols: 1,
            ..joined(members, (from, to))
        });
        i += 1;
    }
    blocks
}

#[cfg(test)]
mod tests {
    use crate::day::testing::*;
    use crate::day::{DayInput, LaneKind, view};
    use crate::db::activities::ActivityRow;
    use crate::store::settings::{Noise, SUMMARY_MINUTES};

    fn active_minutes(lane: &crate::day::Lane) -> Vec<i64> {
        lane.blocks.iter().map(|b| b.active_ms / MIN).collect()
    }

    #[test]
    fn the_summary_lane_comes_first_with_single_column_blocks_named_by_their_start() {
        let view = render(rows(&[(20, 25, "Code", "")]), &summarizing(15));

        assert_eq!(lane_id_list(&view), ["summary", "activity"]);
        let lane = lane_by_id(&view, "summary");
        assert_eq!(lane.kind, LaneKind::Summary);
        assert_eq!(lane.label, "Summary");
        assert_eq!(lane.app_key, None);
        assert_eq!(lane.bundle_id, None);
        let block = &lane.blocks[0];
        assert_eq!(
            (block.id.as_str(), block.col, block.cols),
            (format!("s{}", mins(15)).as_str(), 0, 1)
        );
        assert_eq!((block.start, block.end), (mins(15), mins(30)));
    }

    #[test]
    fn buckets_align_to_the_clock_and_cover_every_bucket_the_activity_touches() {
        let cases = [
            (
                (13 * 60 + 12, 13 * 60 + 49),
                5,
                (13 * 60 + 10, 13 * 60 + 50),
            ),
            ((7, 8), 15, (0, 15)),
            ((62, 63), 30, (60, 90)),
            ((61, 62), 60, (60, 120)),
        ];

        for ((start, end), minutes, expected) in cases {
            let view = render(rows(&[(start, end, "Code", "")]), &summarizing(minutes));

            assert_eq!(
                shape(lane_by_id(&view, "summary")),
                [(expected.0, expected.1, vec![1])],
                "{start}-{end} in {minutes} minute buckets"
            );
        }
    }

    #[test]
    fn buckets_count_from_the_start_of_the_day_and_the_last_one_is_cut_at_its_end() {
        let rows = vec![
            activity(1, mins(20), Some(mins(21)), "Code", ""),
            activity(2, mins(66), Some(mins(69)), "Mail", ""),
        ];
        let input = DayInput {
            bounds: (mins(2), mins(70)),
            ..input(rows, vec![])
        };

        let view = view(&input, &summarizing(15));

        assert_eq!(
            spans(lane_by_id(&view, "summary")),
            [(mins(17), mins(32)), (mins(62), mins(70))]
        );
    }

    #[test]
    fn no_summary_block_starts_or_ends_off_the_grid() {
        let off_grid =
            |id: i64, start: i64, end: i64, app: &str| activity(id, start, Some(end), app, "");
        let inputs: Vec<Vec<ActivityRow>> = vec![
            vec![off_grid(1, 1_234, mins(7) + 999, "Code")],
            vec![
                off_grid(1, 1_234, mins(7) + 999, "Code"),
                off_grid(2, mins(7) + 999, mins(23) + 17, "Mail"),
                off_grid(3, mins(60) + 1, mins(61), "Code"),
            ],
            vec![
                off_grid(1, mins(59) + 59_999, mins(61) + 1, "Code"),
                off_grid(2, mins(61) + 1, mins(200) + 12_345, "Mail"),
            ],
            vec![off_grid(1, DAY_LENGTH - 1_000, DAY_LENGTH + 5_000, "Code")],
        ];

        for (n, rows) in inputs.into_iter().enumerate() {
            for &minutes in SUMMARY_MINUTES {
                let bucket = mins(i64::from(minutes));

                let view = render(rows.clone(), &summarizing(minutes));

                let lane = lane_by_id(&view, "summary");
                assert!(!lane.blocks.is_empty(), "input {n}, {minutes} minutes");
                for block in &lane.blocks {
                    assert_eq!(
                        (block.start % bucket, block.end % bucket),
                        (0, 0),
                        "input {n}, {minutes} minutes: {}-{}",
                        block.start,
                        block.end
                    );
                }
            }
        }
    }

    #[test]
    fn without_the_summary_setting_there_is_no_summary_lane() {
        let view = render(rows(&[(0, 10, "Code", "")]), &settings());

        assert_eq!(lane_id_list(&view), ["activity"]);
    }

    #[test]
    fn an_interval_goes_to_the_key_with_the_most_time_and_the_block_spans_the_whole_interval() {
        let rows = rows(&[(0, 8, "Code", ""), (8, 30, "Mail", "")]);

        let view = render(rows, &summarizing(15));

        assert_eq!(
            shape(lane_by_id(&view, "summary")),
            [(0, 15, vec![1]), (15, 30, vec![2])]
        );
    }

    #[test]
    fn active_ms_is_the_winners_time_inside_the_block() {
        let rows = rows(&[(0, 8, "Code", ""), (8, 30, "Mail", "")]);

        let view = render(rows, &summarizing(15));

        assert_eq!(active_minutes(lane_by_id(&view, "summary")), [8, 15]);
    }

    #[test]
    fn a_merged_block_counts_only_the_time_its_activity_fills() {
        let rows = rows(&[(0, 10, "Code", ""), (20, 25, "Code", "")]);

        let view = render(rows, &summarizing(15));

        let lane = lane_by_id(&view, "summary");
        assert_eq!(shape(lane), [(0, 30, vec![1, 2])]);
        assert_eq!(active_minutes(lane), [15]);
    }

    #[test]
    fn on_a_tie_the_key_that_started_first_wins() {
        let rows = rows(&[(5, 10, "Mail", ""), (0, 5, "Code", "")]);

        let view = render(rows, &summarizing(15));

        assert_eq!(shape(lane_by_id(&view, "summary")), [(0, 15, vec![2])]);
    }

    #[test]
    fn adjacent_intervals_with_one_winner_merge_into_one_block_with_all_its_rows() {
        let rows = rows(&[(0, 20, "Code", ""), (25, 40, "Code", "")]);

        let view = render(rows, &summarizing(15));

        assert_eq!(shape(lane_by_id(&view, "summary")), [(0, 45, vec![1, 2])]);
    }

    #[test]
    fn different_winners_stay_apart_and_empty_intervals_have_no_block() {
        let rows = rows(&[(0, 10, "Code", ""), (50, 60, "Mail", "")]);

        let view = render(rows, &summarizing(15));

        assert_eq!(
            shape(lane_by_id(&view, "summary")),
            [(0, 15, vec![1]), (45, 60, vec![2])]
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
            (15, vec![(0, 15, vec![1]), (15, 30, vec![2])]),
            (30, vec![(0, 30, vec![1])]),
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
            (true, vec![(0, 15, vec![1, 2])]),
            (false, vec![(0, 15, vec![1])]),
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
            [(0, 15, vec![1]), (30, 45, vec![3])]
        );
        assert!(lane_by_id(&view, "activity").blocks.is_empty());
        assert_eq!(
            shape(lane_by_id(&view, "app:test.mail")),
            [(0, 10, vec![1])]
        );
    }
}
