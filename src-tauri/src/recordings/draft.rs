//! The editor's spans while they are edited: Rust rolls ends past midnight, adds the spans up and places a
//! new one, so the page only shows times (ADR 0011).

use chrono::TimeZone;
use serde::Serialize;
use ts_rs::TS;

use super::{MINUTE_MS, SnapEdge, Span, default_span, snap_moving, snap_span};
use crate::db::time::{Day, Ms, same_time_on_in};

/// How long a span added with "Add time" starts out.
const ADDED_SPAN_MS: Ms = 30 * MINUTE_MS;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct DraftSpans {
    /// In the order given, each ending after its start.
    pub spans: Vec<Span>,
    pub total_ms: i64,
}

/// The end at the first instant with its clock time after the start: an end at or before the start is on
/// the next day.
fn rolled<Tz: TimeZone>(span: Span, tz: &Tz) -> Span {
    let day = Day::of_in(span.start, tz);
    let end = same_time_on_in(span.end, day, tz);
    let end = if end > span.start {
        end
    } else {
        same_time_on_in(span.end, day.next(), tz)
    };
    Span {
        start: span.start,
        end,
    }
}

/// `spans` with their ends rolled, and their sum.
pub fn draft<Tz: TimeZone>(spans: &[Span], tz: &Tz) -> DraftSpans {
    let spans: Vec<Span> = spans.iter().map(|&s| rolled(s, tz)).collect();
    let total_ms = spans.iter().map(|s| s.end - s.start).sum();
    DraftSpans { spans, total_ms }
}

/// `spans` with the last one's end moved so they add up to `total_ms`; unchanged when that would end the
/// last span at or before its start, or a day or more after it.
pub fn with_total<Tz: TimeZone>(spans: &[Span], total_ms: i64, tz: &Tz) -> DraftSpans {
    let current = draft(spans, tz);
    let Some(&last) = current.spans.last() else {
        return current;
    };
    let end = last.end + total_ms - current.total_ms;
    if end <= last.start
        || rolled(
            Span {
                start: last.start,
                end,
            },
            tz,
        )
        .end != end
    {
        return current;
    }
    let mut spans = current.spans;
    if let Some(changed) = spans.last_mut() {
        changed.end = end;
    }
    draft(&spans, tz)
}

/// The draft after a click on an activity block: whether the block's span joined it or left it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct DraftToggle {
    /// By start, none touching or overlapping another.
    pub spans: Vec<Span>,
    pub total_ms: i64,
    pub included: bool,
}

/// `spans` by start, with those that touch or overlap merged into one.
fn merged(mut spans: Vec<Span>) -> Vec<Span> {
    spans.sort_by_key(|s| s.start);
    let mut out: Vec<Span> = Vec::with_capacity(spans.len());
    for span in spans {
        match out.last_mut() {
            Some(last) if span.start <= last.end => last.end = last.end.max(span.end),
            _ => out.push(span),
        }
    }
    out
}

/// `spans` without the time of `cut`; a span that sticks out on both sides becomes two.
fn without(spans: &[Span], cut: Span) -> Vec<Span> {
    spans
        .iter()
        .flat_map(|&s| {
            let before = (s.start < cut.start).then(|| Span {
                start: s.start,
                end: s.end.min(cut.start),
            });
            let after = (s.end > cut.end).then(|| Span {
                start: s.start.max(cut.end),
                end: s.end,
            });
            [before, after]
        })
        .flatten()
        .filter(|s| s.end > s.start)
        .collect()
}

/// The shared time of `a` and `b`, if any.
fn shared(a: Span, b: Span) -> Option<Span> {
    let span = Span {
        start: a.start.max(b.start),
        end: a.end.min(b.end),
    };
    (span.end > span.start).then_some(span)
}

/// `spans` with `span` in them, merged with the spans it touches or overlaps.
pub fn with_included<Tz: TimeZone>(spans: &[Span], span: Span, tz: &Tz) -> DraftSpans {
    let mut spans = draft(spans, tz).spans;
    spans.push(span);
    draft(&merged(spans), tz)
}

/// A click on a block with the snapped span `block`: when one of `spans` covers it, its time leaves the draft
/// (the time it shares with `others`, the other clicked blocks' snapped spans, stays); else it joins, merged
/// with the spans it touches or overlaps.
pub fn toggled<Tz: TimeZone>(spans: &[Span], block: Span, others: &[Span], tz: &Tz) -> DraftToggle {
    let spans = draft(spans, tz).spans;
    let covered = spans
        .iter()
        .any(|s| s.start <= block.start && block.end <= s.end);
    let next = if covered {
        let mut kept = without(&spans, block);
        kept.extend(others.iter().filter_map(|&o| shared(o, block)));
        merged(kept)
    } else {
        let mut joined = spans;
        joined.push(block);
        merged(joined)
    };
    let DraftSpans { spans, total_ms } = draft(&next, tz);
    DraftToggle {
        spans,
        total_ms,
        included: !covered,
    }
}

/// `toggled` with `block` and `others` (raw activity spans) snapped to the `minutes` grid first.
pub fn toggled_on_grid<Tz: TimeZone>(
    spans: &[Span],
    block: Span,
    others: &[Span],
    minutes: u32,
    tz: &Tz,
) -> DraftToggle {
    let snapped = |s: &Span| snap_span(s.start, s.end, minutes);
    let others: Vec<Span> = others.iter().map(snapped).collect();
    toggled(spans, snapped(&block), &others, tz)
}

/// `spans` with the one at `index` set to `span`, merged with the spans it now touches or overlaps; unchanged
/// when there is no span at `index`.
pub fn resized<Tz: TimeZone>(spans: &[Span], index: usize, span: Span, tz: &Tz) -> DraftSpans {
    let mut spans = draft(spans, tz).spans;
    let Some(resized) = spans.get_mut(index) else {
        return draft(&spans, tz);
    };
    *resized = span;
    draft(&merged(spans), tz)
}

/// `resized` with `span` (a drag's raw times) snapped to the `minutes` grid first, `moving` as the dragged end.
pub fn resized_on_grid<Tz: TimeZone>(
    spans: &[Span],
    index: usize,
    span: Span,
    moving: Option<SnapEdge>,
    minutes: u32,
    tz: &Tz,
) -> DraftSpans {
    let snapped = snap_moving(span.start, span.end, minutes, moving);
    resized(spans, index, snapped, tz)
}

/// `spans` and a new span after the last one, else the default span before `now`.
pub fn with_added<Tz: TimeZone>(spans: &[Span], now: Ms, snap_minutes: u32, tz: &Tz) -> DraftSpans {
    let mut spans = draft(spans, tz).spans;
    let start = spans
        .last()
        .map_or_else(|| default_span(now, snap_minutes).start, |s| s.end);
    spans.push(Span {
        start,
        end: start + ADDED_SPAN_MS,
    });
    draft(&spans, tz)
}

#[cfg(test)]
mod tests {
    use chrono_tz::Europe::Vienna;
    use serde_json::json;

    use super::*;

    const MIN: Ms = 60_000;
    const HOUR: Ms = 60 * MIN;

    fn vienna(month: u32, day: u32, hour: u32, minute: u32) -> Ms {
        Vienna
            .with_ymd_and_hms(2026, month, day, hour, minute, 0)
            .earliest()
            .unwrap()
            .timestamp_millis()
    }

    fn june(day: u32, hour: u32, minute: u32) -> Ms {
        vienna(6, day, hour, minute)
    }

    fn span(start: Ms, end: Ms) -> Span {
        Span { start, end }
    }

    #[test]
    fn a_span_ending_after_its_start_is_unchanged_and_summed() {
        let spans = [
            span(june(15, 9, 0), june(15, 10, 0)),
            span(june(15, 13, 0), june(15, 14, 30)),
        ];

        let drafted = draft(&spans, &Vienna);

        assert_eq!(
            drafted,
            DraftSpans {
                spans: spans.to_vec(),
                total_ms: 150 * MIN
            }
        );
    }

    #[test]
    fn an_end_before_its_start_rolls_to_the_next_day() {
        let drafted = draft(&[span(june(15, 23, 0), june(15, 1, 0))], &Vienna);

        assert_eq!(
            drafted,
            DraftSpans {
                spans: vec![span(june(15, 23, 0), june(16, 1, 0))],
                total_ms: 2 * HOUR
            }
        );
    }

    #[test]
    fn an_end_at_its_start_rolls_to_the_next_day() {
        let drafted = draft(&[span(june(15, 10, 0), june(15, 10, 0))], &Vienna);

        assert_eq!(
            drafted,
            DraftSpans {
                spans: vec![span(june(15, 10, 0), june(16, 10, 0))],
                total_ms: 24 * HOUR
            }
        );
    }

    #[test]
    fn rolled_ends_are_summed_with_the_other_spans() {
        let drafted = draft(
            &[
                span(june(15, 9, 0), june(15, 10, 0)),
                span(june(15, 22, 0), june(15, 2, 0)),
            ],
            &Vienna,
        );

        assert_eq!(drafted.spans[1].end, june(16, 2, 0));
        assert_eq!(drafted.total_ms, 5 * HOUR);
    }

    #[test]
    fn a_roll_over_the_spring_dst_night_counts_the_real_hours() {
        let start = vienna(3, 28, 23, 0);

        let drafted = draft(&[span(start, vienna(3, 28, 3, 0))], &Vienna);

        assert_eq!(
            drafted,
            DraftSpans {
                spans: vec![span(start, vienna(3, 29, 3, 0))],
                total_ms: 3 * HOUR
            }
        );
    }

    #[test]
    fn a_roll_over_the_autumn_dst_night_counts_the_real_hours() {
        let start = vienna(10, 24, 23, 0);

        let drafted = draft(&[span(start, vienna(10, 24, 1, 0))], &Vienna);

        assert_eq!(drafted.spans, [span(start, vienna(10, 25, 1, 0))]);
        assert_eq!(drafted.total_ms, 2 * HOUR);
    }

    #[test]
    fn no_spans_draft_to_nothing() {
        assert_eq!(
            draft(&[], &Vienna),
            DraftSpans {
                spans: Vec::new(),
                total_ms: 0
            }
        );
    }

    #[test]
    fn a_draft_serializes_with_camel_case_fields() {
        let drafted = draft(&[span(june(15, 9, 0), june(15, 10, 0))], &Vienna);

        assert_eq!(
            serde_json::to_value(&drafted).unwrap(),
            json!({
                "spans": [{ "start": june(15, 9, 0), "end": june(15, 10, 0) }],
                "totalMs": HOUR
            })
        );
    }

    #[test]
    fn with_total_moves_the_end_of_the_last_span() {
        let spans = [
            span(june(15, 9, 0), june(15, 10, 0)),
            span(june(15, 13, 0), june(15, 14, 0)),
        ];

        let drafted = with_total(&spans, 150 * MIN, &Vienna);

        assert_eq!(
            drafted,
            DraftSpans {
                spans: vec![spans[0], span(june(15, 13, 0), june(15, 14, 30))],
                total_ms: 150 * MIN
            }
        );
    }

    #[test]
    fn with_total_can_carry_the_end_past_midnight() {
        let drafted = with_total(
            &[span(june(15, 23, 0), june(15, 23, 30))],
            2 * HOUR,
            &Vienna,
        );

        assert_eq!(
            drafted,
            DraftSpans {
                spans: vec![span(june(15, 23, 0), june(16, 1, 0))],
                total_ms: 2 * HOUR
            }
        );
    }

    #[test]
    fn with_total_leaves_the_spans_when_the_last_would_end_at_or_before_its_start() {
        let spans = [
            span(june(15, 9, 0), june(15, 10, 0)),
            span(june(15, 13, 0), june(15, 14, 0)),
        ];
        let unchanged = draft(&spans, &Vienna);

        for total in [HOUR, 30 * MIN, 0, -HOUR] {
            assert_eq!(with_total(&spans, total, &Vienna), unchanged, "{total}");
        }
    }

    #[test]
    fn with_total_leaves_the_spans_when_the_last_would_end_more_than_a_day_after_its_start() {
        let spans = [span(june(15, 10, 0), june(15, 11, 0))];
        let unchanged = draft(&spans, &Vienna);

        assert_eq!(with_total(&spans, 25 * HOUR, &Vienna), unchanged);
    }

    #[test]
    fn with_total_of_no_spans_is_empty() {
        assert_eq!(
            with_total(&[], HOUR, &Vienna),
            DraftSpans {
                spans: Vec::new(),
                total_ms: 0
            }
        );
    }

    #[test]
    fn with_added_places_a_half_hour_span_after_the_last_one() {
        let first = span(june(15, 10, 0), june(15, 11, 0));

        let drafted = with_added(&[first], june(15, 20, 0), 5, &Vienna);

        assert_eq!(
            drafted,
            DraftSpans {
                spans: vec![first, span(june(15, 11, 0), june(15, 11, 30))],
                total_ms: 90 * MIN
            }
        );
    }

    #[test]
    fn with_added_follows_the_rolled_end_of_the_last_span() {
        let late = [span(june(15, 22, 0), june(15, 1, 0))];

        let drafted = with_added(&late, june(16, 12, 0), 5, &Vienna);

        assert_eq!(
            drafted.spans,
            [
                span(june(15, 22, 0), june(16, 1, 0)),
                span(june(16, 1, 0), june(16, 1, 30))
            ]
        );
    }

    #[test]
    fn with_added_without_spans_starts_from_the_default_span_before_now() {
        let drafted = with_added(&[], june(15, 13, 12), 5, &Vienna);

        assert_eq!(
            drafted,
            DraftSpans {
                spans: vec![span(june(15, 12, 40), june(15, 13, 10))],
                total_ms: 30 * MIN
            }
        );
    }

    #[test]
    fn with_included_merges_spans_that_touch_or_overlap() {
        let existing = [span(june(15, 9, 0), june(15, 10, 0))];

        for (added, expected_end) in [
            (span(june(15, 10, 0), june(15, 11, 0)), june(15, 11, 0)),
            (span(june(15, 9, 30), june(15, 11, 0)), june(15, 11, 0)),
            (span(june(15, 9, 15), june(15, 9, 45)), june(15, 10, 0)),
        ] {
            let drafted = with_included(&existing, added, &Vienna);

            assert_eq!(
                drafted.spans,
                [span(june(15, 9, 0), expected_end)],
                "{added:?}"
            );
            assert_eq!(drafted.total_ms, expected_end - june(15, 9, 0), "{added:?}");
        }
    }

    #[test]
    fn with_included_keeps_a_span_with_a_gap_separate_and_sorted() {
        let existing = [span(june(15, 13, 0), june(15, 14, 0))];

        let drafted = with_included(&existing, span(june(15, 9, 0), june(15, 10, 0)), &Vienna);

        assert_eq!(
            drafted,
            DraftSpans {
                spans: vec![
                    span(june(15, 9, 0), june(15, 10, 0)),
                    span(june(15, 13, 0), june(15, 14, 0))
                ],
                total_ms: 2 * HOUR
            }
        );
    }

    #[test]
    fn toggling_an_uncovered_block_adds_it() {
        let toggle = toggled(&[], span(june(15, 9, 0), june(15, 10, 0)), &[], &Vienna);

        assert_eq!(
            toggle,
            DraftToggle {
                spans: vec![span(june(15, 9, 0), june(15, 10, 0))],
                total_ms: HOUR,
                included: true
            }
        );
    }

    #[test]
    fn toggling_an_uncovered_block_merges_it_with_the_spans_it_touches() {
        let spans = [
            span(june(15, 9, 0), june(15, 10, 0)),
            span(june(15, 11, 0), june(15, 12, 0)),
        ];

        let toggle = toggled(&spans, span(june(15, 10, 0), june(15, 11, 0)), &[], &Vienna);

        assert_eq!(
            toggle,
            DraftToggle {
                spans: vec![span(june(15, 9, 0), june(15, 12, 0))],
                total_ms: 3 * HOUR,
                included: true
            }
        );
    }

    #[test]
    fn toggling_two_separate_blocks_leaves_the_gap_unassigned() {
        let first = span(june(15, 9, 0), june(15, 10, 0));
        let second = span(june(15, 11, 0), june(15, 12, 0));

        let after_first = toggled(&[], first, &[], &Vienna);
        let after_second = toggled(&after_first.spans, second, &[first], &Vienna);

        assert_eq!(
            after_second,
            DraftToggle {
                spans: vec![first, second],
                total_ms: 2 * HOUR,
                included: true
            }
        );
    }

    #[test]
    fn toggling_a_covered_block_removes_it() {
        let spans = [
            span(june(15, 9, 0), june(15, 10, 0)),
            span(june(15, 11, 0), june(15, 12, 0)),
        ];

        let toggle = toggled(&spans, spans[1], &[], &Vienna);

        assert_eq!(
            toggle,
            DraftToggle {
                spans: vec![spans[0]],
                total_ms: HOUR,
                included: false
            }
        );
    }

    #[test]
    fn toggling_a_block_inside_a_span_splits_the_span() {
        let spans = [span(june(15, 9, 0), june(15, 12, 0))];

        let toggle = toggled(&spans, span(june(15, 10, 0), june(15, 11, 0)), &[], &Vienna);

        assert_eq!(
            toggle,
            DraftToggle {
                spans: vec![
                    span(june(15, 9, 0), june(15, 10, 0)),
                    span(june(15, 11, 0), june(15, 12, 0))
                ],
                total_ms: 2 * HOUR,
                included: false
            }
        );
    }

    #[test]
    fn toggling_off_the_only_span_leaves_nothing() {
        let only = span(june(15, 9, 0), june(15, 10, 0));

        let toggle = toggled(&[only], only, &[], &Vienna);

        assert_eq!(
            toggle,
            DraftToggle {
                spans: Vec::new(),
                total_ms: 0,
                included: false
            }
        );
    }

    #[test]
    fn removing_a_block_keeps_the_time_it_shares_with_other_blocks() {
        let spans = [span(june(15, 9, 0), june(15, 12, 0))];
        let block = span(june(15, 9, 30), june(15, 10, 30));
        let other = span(june(15, 10, 0), june(15, 11, 0));

        let toggle = toggled(&spans, block, &[other], &Vienna);

        assert_eq!(
            toggle,
            DraftToggle {
                spans: vec![
                    span(june(15, 9, 0), june(15, 9, 30)),
                    span(june(15, 10, 0), june(15, 12, 0))
                ],
                total_ms: 150 * MIN,
                included: false
            }
        );
    }

    #[test]
    fn removing_a_block_ignores_other_blocks_that_share_no_time_with_it() {
        let spans = [span(june(15, 9, 0), june(15, 12, 0))];
        let block = span(june(15, 9, 0), june(15, 10, 0));
        let apart = span(june(15, 13, 0), june(15, 14, 0));
        let touching = span(june(15, 10, 0), june(15, 10, 30));

        let toggle = toggled(&spans, block, &[apart, touching], &Vienna);

        assert_eq!(toggle.spans, [span(june(15, 10, 0), june(15, 12, 0))]);
        assert_eq!(toggle.total_ms, 2 * HOUR);
    }

    #[test]
    fn a_block_only_partly_overlapping_a_span_is_added_and_merged() {
        let spans = [span(june(15, 9, 0), june(15, 10, 0))];

        let toggle = toggled(&spans, span(june(15, 9, 30), june(15, 11, 0)), &[], &Vienna);

        assert_eq!(
            toggle,
            DraftToggle {
                spans: vec![span(june(15, 9, 0), june(15, 11, 0))],
                total_ms: 2 * HOUR,
                included: true
            }
        );
    }

    #[test]
    fn an_unsnapped_block_is_added_on_the_grid_and_toggled_off_again() {
        let raw = span(june(15, 13, 12), june(15, 13, 38));

        let added = toggled_on_grid(&[], raw, &[], 5, &Vienna);
        let removed = toggled_on_grid(&added.spans, raw, &[], 5, &Vienna);

        assert_eq!(
            added,
            DraftToggle {
                spans: vec![span(june(15, 13, 10), june(15, 13, 40))],
                total_ms: 30 * MIN,
                included: true
            }
        );
        assert_eq!(
            removed,
            DraftToggle {
                spans: Vec::new(),
                total_ms: 0,
                included: false
            }
        );
    }

    fn two_spans() -> [Span; 2] {
        [
            span(june(15, 9, 0), june(15, 10, 0)),
            span(june(15, 11, 0), june(15, 12, 0)),
        ]
    }

    #[test]
    fn resizing_replaces_the_span_at_the_index() {
        let resized = resized(
            &two_spans(),
            1,
            span(june(15, 11, 0), june(15, 13, 0)),
            &Vienna,
        );

        assert_eq!(
            resized,
            DraftSpans {
                spans: vec![
                    span(june(15, 9, 0), june(15, 10, 0)),
                    span(june(15, 11, 0), june(15, 13, 0))
                ],
                total_ms: 3 * HOUR
            }
        );
    }

    #[test]
    fn resizing_a_span_onto_or_into_another_merges_them() {
        for (index, new_span, expected) in [
            (
                0,
                span(june(15, 9, 0), june(15, 11, 0)),
                span(june(15, 9, 0), june(15, 12, 0)),
            ),
            (
                0,
                span(june(15, 9, 0), june(15, 11, 30)),
                span(june(15, 9, 0), june(15, 12, 0)),
            ),
            (
                1,
                span(june(15, 10, 0), june(15, 12, 0)),
                span(june(15, 9, 0), june(15, 12, 0)),
            ),
            (
                1,
                span(june(15, 8, 0), june(15, 12, 0)),
                span(june(15, 8, 0), june(15, 12, 0)),
            ),
        ] {
            let resized = resized(&two_spans(), index, new_span, &Vienna);

            assert_eq!(resized.spans, [expected], "{index} {new_span:?}");
            assert_eq!(
                resized.total_ms,
                expected.end - expected.start,
                "{index} {new_span:?}"
            );
        }
    }

    #[test]
    fn resizing_keeps_spans_that_do_not_touch_separate_and_sorted() {
        let resized = resized(
            &two_spans(),
            1,
            span(june(15, 7, 0), june(15, 8, 0)),
            &Vienna,
        );

        assert_eq!(
            resized,
            DraftSpans {
                spans: vec![
                    span(june(15, 7, 0), june(15, 8, 0)),
                    span(june(15, 9, 0), june(15, 10, 0))
                ],
                total_ms: 2 * HOUR
            }
        );
    }

    #[test]
    fn resizing_an_index_out_of_range_leaves_the_spans_unchanged() {
        let resized = resized(
            &two_spans(),
            2,
            span(june(15, 9, 0), june(15, 12, 0)),
            &Vienna,
        );

        assert_eq!(resized, draft(&two_spans(), &Vienna));
    }

    #[test]
    fn resizing_on_the_grid_snaps_the_moved_start() {
        let resized = resized_on_grid(
            &two_spans(),
            1,
            span(june(15, 10, 33), june(15, 12, 0)),
            Some(SnapEdge::Start),
            5,
            &Vienna,
        );

        assert_eq!(
            resized,
            DraftSpans {
                spans: vec![
                    span(june(15, 9, 0), june(15, 10, 0)),
                    span(june(15, 10, 35), june(15, 12, 0))
                ],
                total_ms: 85 * MIN + HOUR
            }
        );
    }

    #[test]
    fn resizing_on_the_grid_snaps_the_moved_end() {
        let resized = resized_on_grid(
            &two_spans(),
            0,
            span(june(15, 9, 0), june(15, 10, 33)),
            Some(SnapEdge::End),
            5,
            &Vienna,
        );

        assert_eq!(
            resized.spans,
            [
                span(june(15, 9, 0), june(15, 10, 35)),
                span(june(15, 11, 0), june(15, 12, 0))
            ]
        );
    }

    #[test]
    fn resizing_on_the_grid_keeps_a_dragged_end_a_step_from_the_other_end() {
        let spans = two_spans();

        let end_dragged_before_start = resized_on_grid(
            &spans,
            1,
            span(june(15, 11, 0), june(15, 10, 40)),
            Some(SnapEdge::End),
            5,
            &Vienna,
        );
        let start_dragged_past_end = resized_on_grid(
            &spans,
            1,
            span(june(15, 12, 20), june(15, 12, 0)),
            Some(SnapEdge::Start),
            5,
            &Vienna,
        );

        assert_eq!(
            end_dragged_before_start.spans[1],
            span(june(15, 11, 0), june(15, 11, 5))
        );
        assert_eq!(
            start_dragged_past_end.spans[1],
            span(june(15, 11, 55), june(15, 12, 0))
        );
    }

    #[test]
    fn dragging_an_end_to_the_next_span_on_the_grid_merges_them() {
        let resized = resized_on_grid(
            &two_spans(),
            0,
            span(june(15, 9, 0), june(15, 10, 58)),
            Some(SnapEdge::End),
            5,
            &Vienna,
        );

        assert_eq!(
            resized,
            DraftSpans {
                spans: vec![span(june(15, 9, 0), june(15, 12, 0))],
                total_ms: 3 * HOUR
            }
        );
    }

    #[test]
    fn a_toggle_serializes_with_camel_case_fields() {
        let toggle = toggled(&[], span(june(15, 9, 0), june(15, 10, 0)), &[], &Vienna);

        assert_eq!(
            serde_json::to_value(&toggle).unwrap(),
            json!({
                "spans": [{ "start": june(15, 9, 0), "end": june(15, 10, 0) }],
                "totalMs": HOUR,
                "included": true
            })
        );
    }
}
