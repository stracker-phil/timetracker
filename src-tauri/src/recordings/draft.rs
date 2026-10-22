//! The editor's spans while they are edited: Rust rolls ends past midnight, adds the spans up and places a
//! new one, so the page only shows times (ADR 0011).

use chrono::TimeZone;
use serde::Serialize;
use ts_rs::TS;

use super::{MINUTE_MS, Span, default_span};
use crate::db::time::{Day, Ms, same_time_on_in};

/// How long a span added with "Add span" starts out.
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
}
