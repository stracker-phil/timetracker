//! Clearing other recordings out of the way of a write (`Overlap`).

use super::checks::timer_running;
use super::ops::{Op, overlap_error};
use super::{Overlap, Span};
use crate::db::recordings::{self, Recording, RecordingSpan};
use crate::db::time::{Ms, now_ms};
use crate::db::{Db, new_id};
use crate::error::AppResult;

/// `[start, end)` without `cut`: nothing, one piece or two.
fn without(start: Ms, end: Ms, cut: Span) -> Vec<(Ms, Ms)> {
    if cut.end <= start || cut.start >= end {
        return vec![(start, end)];
    }
    let mut pieces = Vec::new();
    if start < cut.start {
        pieces.push((start, cut.start));
    }
    if end > cut.end {
        pieces.push((cut.end, end));
    }
    pieces
}

/// What clears `cut` of `other`'s spans: a span inside it goes, one sticking out is trimmed, one around it
/// is cut in two (both stay spans of `other`). Without spans left, `other` is deleted.
fn clear_op(other: Recording, cut: &[Span], now: Ms) -> AppResult<Op> {
    let mut rest = Vec::new();
    for span in &other.spans {
        let Some(end) = span.end else {
            if cut.iter().any(|c| c.end > span.start) {
                return Err(timer_running());
            }
            rest.push(span.clone());
            continue;
        };
        let mut pieces = vec![(span.start, end)];
        for &c in cut {
            pieces = pieces
                .into_iter()
                .flat_map(|(s, e)| without(s, e, c))
                .collect();
        }
        for (i, (start, end)) in pieces.into_iter().enumerate() {
            let id = if i == 0 { span.id.clone() } else { new_id() };
            rest.push(RecordingSpan {
                id,
                start,
                end: Some(end),
            });
        }
    }
    if rest.is_empty() {
        return Ok(Op::Delete { recording: other });
    }
    rest.sort_by_key(|s| s.start);
    let after = Recording {
        spans: rest,
        updated_at: now,
        ..other.clone()
    };
    Ok(Op::Update {
        before: other,
        after,
    })
}

/// The time `recording` claims: its spans, an open one up to the end of time.
pub(super) fn claimed(recording: &Recording) -> Vec<Span> {
    recording
        .spans
        .iter()
        .map(|s| Span {
            start: s.start,
            end: s.end.unwrap_or(Ms::MAX),
        })
        .collect()
}

/// Other recordings with a span sharing time with one of `spans`, each once.
pub(super) async fn in_the_way(db: &Db, spans: &[Span], except: &str) -> AppResult<Vec<Recording>> {
    let mut others: Vec<Recording> = Vec::new();
    for span in spans {
        for other in recordings::overlapping(db, span.start, span.end, Some(except)).await? {
            if !others.iter().any(|o| o.id == other.id) {
                others.push(other);
            }
        }
    }
    others.sort_by_key(Recording::start);
    Ok(others)
}

/// `base` as is when nothing is in the way of `new`, else per `overlap`: refused, or batched after what
/// clears its spans.
pub(super) async fn clearing(
    db: &Db,
    base: Op,
    new: &Recording,
    overlap: Overlap,
    label: &str,
) -> AppResult<Op> {
    let cut = claimed(new);
    let others = in_the_way(db, &cut, &new.id).await?;
    if others.is_empty() {
        return Ok(base);
    }
    if overlap == Overlap::Cancel {
        return Err(overlap_error(others));
    }
    let now = now_ms();
    let mut ops = Vec::new();
    for other in others {
        ops.push(clear_op(other, &cut, now)?);
    }
    ops.push(base);
    Ok(Op::Batch {
        label: label.to_string(),
        ops,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;
    use crate::recordings::RecordingInput;
    use crate::recordings::testing::*;
    use crate::test_support;

    #[tokio::test]
    async fn cancel_refuses_an_overlap_listing_the_recordings_in_the_way() {
        let w = World::new().await;
        let first = w.create(at(10, 0), at(11, 0)).await;
        let second = w.create(at(11, 0), at(12, 0)).await;

        let err = w
            .service()
            .create(w.input(at(10, 30), at(11, 30)), Overlap::Cancel)
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::Overlap);
        let in_the_way: Vec<Recording> =
            serde_json::from_value(err.details().unwrap().clone()).unwrap();
        assert_eq!(in_the_way, [first.clone(), second.clone()]);
        assert_eq!(w.rows().await, [first, second]);
    }

    #[tokio::test]
    async fn cancel_lists_a_neighbour_with_several_spans_in_the_way_once() {
        let w = World::new().await;
        let neighbour = w
            .create_spans(&[(at(10, 0), at(11, 0)), (at(12, 0), at(13, 0))])
            .await;

        let err = w
            .service()
            .create(
                input_of(&w, &[(at(10, 30), at(10, 45)), (at(12, 15), at(12, 30))]),
                Overlap::Cancel,
            )
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::Overlap);
        assert_eq!(overlap_ids(&err), [neighbour.id.clone()]);
        assert_eq!(w.rows().await, [neighbour]);
    }

    #[tokio::test]
    async fn a_span_in_the_gap_between_a_neighbours_spans_is_not_in_the_way() {
        let w = World::new().await;
        let neighbour = w
            .create_spans(&[(at(10, 0), at(11, 0)), (at(13, 0), at(14, 0))])
            .await;

        let saved = w.create(at(11, 0), at(13, 0)).await;

        assert_eq!(w.rows().await, [neighbour, saved]);
    }

    #[tokio::test]
    async fn recordings_that_only_touch_do_not_overlap() {
        let w = World::new().await;
        w.create(at(10, 0), at(11, 0)).await;
        w.create(at(12, 0), at(13, 0)).await;

        w.create(at(11, 0), at(12, 0)).await;

        assert_eq!(
            spans(&w.rows().await),
            [
                (at(10, 0), at(11, 0)),
                (at(11, 0), at(12, 0)),
                (at(12, 0), at(13, 0))
            ]
        );
    }

    #[tokio::test]
    async fn replace_trims_splits_and_deletes_what_is_in_the_way() {
        type Spans = Vec<(Ms, Ms)>;
        let cases: Vec<(&str, Spans, (Ms, Ms), Spans)> = vec![
            (
                "trims the one sticking out on the left",
                vec![(at(10, 0), at(11, 0))],
                (at(10, 30), at(11, 30)),
                vec![(at(10, 0), at(10, 30)), (at(10, 30), at(11, 30))],
            ),
            (
                "trims the one sticking out on the right",
                vec![(at(10, 0), at(11, 0))],
                (at(9, 30), at(10, 30)),
                vec![(at(9, 30), at(10, 30)), (at(10, 30), at(11, 0))],
            ),
            (
                "deletes the one inside",
                vec![(at(10, 0), at(11, 0))],
                (at(9, 0), at(12, 0)),
                vec![(at(9, 0), at(12, 0))],
            ),
            (
                "deletes the one with the same span",
                vec![(at(10, 0), at(11, 0))],
                (at(10, 0), at(11, 0)),
                vec![(at(10, 0), at(11, 0))],
            ),
            (
                "splits the one around it",
                vec![(at(10, 0), at(11, 0))],
                (at(10, 20), at(10, 40)),
                vec![
                    (at(10, 0), at(10, 20)),
                    (at(10, 20), at(10, 40)),
                    (at(10, 40), at(11, 0)),
                ],
            ),
            (
                "handles several at once",
                vec![
                    (at(10, 0), at(11, 0)),
                    (at(11, 0), at(12, 0)),
                    (at(12, 0), at(13, 0)),
                ],
                (at(10, 30), at(12, 30)),
                vec![
                    (at(10, 0), at(10, 30)),
                    (at(10, 30), at(12, 30)),
                    (at(12, 30), at(13, 0)),
                ],
            ),
            (
                "leaves touching neighbours alone",
                vec![(at(9, 0), at(10, 0)), (at(11, 0), at(12, 0))],
                (at(10, 0), at(11, 0)),
                vec![
                    (at(9, 0), at(10, 0)),
                    (at(10, 0), at(11, 0)),
                    (at(11, 0), at(12, 0)),
                ],
            ),
        ];

        for (name, existing, (start, end), expected) in cases {
            let w = World::new().await;
            for (s, e) in existing {
                w.create(s, e).await;
            }

            w.service()
                .create(w.input(start, end), Overlap::Replace)
                .await
                .unwrap();

            let rows = w.rows().await;
            assert_eq!(spans(&rows), expected, "{name}");
            assert!(no_two_overlap(&rows), "{name}");
        }
    }

    #[tokio::test]
    async fn cutting_a_span_sticking_out_on_both_sides_leaves_two_spans_of_the_same_recording() {
        let w = World::new().await;
        let original = w
            .service()
            .create(
                RecordingInput {
                    project_id: w.other.id.clone(),
                    note: "keep".into(),
                    tag_id: Some(w.alpha.id.clone()),
                    ..w.input(at(10, 0), at(11, 0))
                },
                Overlap::Cancel,
            )
            .await
            .unwrap()
            .value;

        w.service()
            .create(w.input(at(10, 20), at(10, 40)), Overlap::Replace)
            .await
            .unwrap();

        let rows = w.rows().await;
        assert_eq!(rows.len(), 2);
        let (cut, new) = (&rows[0], &rows[1]);
        assert_eq!(cut.id, original.id);
        assert_eq!(
            *cut,
            Recording {
                updated_at: cut.updated_at,
                spans: vec![
                    RecordingSpan {
                        id: original.spans[0].id.clone(),
                        start: at(10, 0),
                        end: Some(at(10, 20)),
                    },
                    RecordingSpan {
                        id: cut.spans[1].id.clone(),
                        start: at(10, 40),
                        end: Some(at(11, 0)),
                    },
                ],
                ..original.clone()
            }
        );
        assert_ne!(cut.spans[1].id, original.spans[0].id);
        assert_eq!(new.project_id, w.project.id);
        assert_eq!(times(new), [(at(10, 20), at(10, 40))]);
    }

    #[tokio::test]
    async fn replace_deletes_inside_spans_trims_edge_spans_and_cuts_a_span_in_two() {
        let w = World::new().await;
        let one = w
            .create_spans(&[(at(8, 0), at(9, 0)), (at(10, 0), at(11, 0))])
            .await;
        let two = w
            .create_spans(&[(at(12, 0), at(13, 0)), (at(14, 0), at(15, 0))])
            .await;

        w.service()
            .create(
                input_of(&w, &[(at(10, 30), at(10, 45)), (at(12, 30), at(16, 0))]),
                Overlap::Replace,
            )
            .await
            .unwrap();

        let rows = w.rows().await;
        assert_eq!(rows.len(), 3);
        let one_after = rows.iter().find(|r| r.id == one.id).unwrap();
        let two_after = rows.iter().find(|r| r.id == two.id).unwrap();
        assert_eq!(
            times(one_after),
            [
                (at(8, 0), at(9, 0)),
                (at(10, 0), at(10, 30)),
                (at(10, 45), at(11, 0))
            ]
        );
        assert_eq!(times(two_after), [(at(12, 0), at(12, 30))]);
        assert_eq!(one_after.spans[0].id, one.spans[0].id);
        assert_eq!(one_after.spans[1].id, one.spans[1].id);
        assert!(no_two_overlap(&rows));
    }

    #[tokio::test]
    async fn replace_deletes_a_recording_left_without_spans() {
        let w = World::new().await;
        w.create_spans(&[(at(10, 0), at(10, 30)), (at(11, 0), at(11, 30))])
            .await;

        w.service()
            .create(w.input(at(9, 0), at(12, 0)), Overlap::Replace)
            .await
            .unwrap();

        assert_eq!(spans(&w.rows().await), [(at(9, 0), at(12, 0))]);
        assert_eq!(w.rows().await.len(), 1);
    }

    #[tokio::test]
    async fn undoing_and_redoing_a_replace_restores_the_exact_span_ids() {
        let w = World::new().await;
        let one = w
            .create_spans(&[(at(10, 0), at(11, 0)), (at(12, 0), at(13, 0))])
            .await;
        let two = w.create(at(14, 0), at(15, 0)).await;
        let before = w.rows().await;
        w.service()
            .create(
                input_of(&w, &[(at(10, 20), at(10, 40)), (at(12, 0), at(15, 0))]),
                Overlap::Replace,
            )
            .await
            .unwrap();
        let replaced = w.rows().await;
        assert!(replaced.iter().all(|r| r.id != two.id));

        w.service().undo().await.unwrap();

        let restored = w.rows().await;
        assert_eq!(restored, before);
        assert_eq!(
            span_ids(restored.iter().find(|r| r.id == one.id).unwrap()),
            span_ids(&one)
        );

        w.service().redo().await.unwrap();

        assert_eq!(w.rows().await, replaced);
    }

    #[tokio::test]
    async fn a_replace_is_one_undo_step_that_restores_the_trimmed_recordings_exactly() {
        let w = World::new().await;
        w.service()
            .create(
                RecordingInput {
                    note: "keep".into(),
                    tag_id: Some(w.alpha.id.clone()),
                    ..w.input(at(10, 0), at(11, 0))
                },
                Overlap::Cancel,
            )
            .await
            .unwrap();
        w.create(at(11, 0), at(12, 0)).await;
        let before = w.rows().await;
        w.service()
            .create(w.input(at(10, 20), at(11, 30)), Overlap::Replace)
            .await
            .unwrap();
        let replaced = w.rows().await;

        let undone = w.service().undo().await.unwrap();
        let after_undo = w.rows().await;
        let redone = w.service().redo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Record 1 h 10 min for Acme"));
        assert_eq!(after_undo, before);
        assert_eq!(redone.value.as_deref(), Some("Record 1 h 10 min for Acme"));
        assert_eq!(w.rows().await, replaced);
    }

    #[tokio::test]
    async fn a_running_recording_in_the_way_cannot_be_replaced() {
        let w = World::new().await;
        let timer = test_support::recording(&w.db, &w.project.id, at(10, 0), None).await;

        let replace = w
            .service()
            .create(w.input(at(9, 0), at(10, 30)), Overlap::Replace)
            .await
            .unwrap_err();
        let cancel = w
            .service()
            .create(w.input(at(9, 0), at(10, 30)), Overlap::Cancel)
            .await
            .unwrap_err();

        assert_eq!(replace.kind(), ErrorKind::InvalidInput);
        assert_eq!(cancel.kind(), ErrorKind::Overlap);
        assert_eq!(w.rows().await, [timer]);
        assert_eq!(w.status().await, no_status());
    }

    #[tokio::test]
    async fn a_recording_before_the_running_one_does_not_overlap_it() {
        let w = World::new().await;
        test_support::recording(&w.db, &w.project.id, at(12, 0), None).await;

        let saved = w.create(at(10, 0), at(11, 0)).await;

        assert_eq!(times(&saved), [(at(10, 0), at(11, 0))]);
        assert_eq!(w.rows().await.len(), 2);
    }

    #[tokio::test]
    async fn update_into_another_recording_follows_the_overlap_mode() {
        let w = World::new().await;
        let first = w.create(at(10, 0), at(11, 0)).await;
        let second = w.create(at(12, 0), at(13, 0)).await;
        let earlier = || respan(at(10, 30), at(13, 0));

        let err = w
            .service()
            .update(&second.id, earlier(), Overlap::Cancel)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Overlap);
        assert_eq!(w.rows().await, [first, second.clone()]);

        w.service()
            .update(&second.id, earlier(), Overlap::Replace)
            .await
            .unwrap();
        let rows = w.rows().await;
        assert_eq!(
            spans(&rows),
            [(at(10, 0), at(10, 30)), (at(10, 30), at(13, 0))]
        );
        assert!(no_two_overlap(&rows));
    }

    #[tokio::test]
    async fn a_recording_can_grow_and_move_within_its_own_span() {
        let w = World::new().await;
        let saved = w.create(at(10, 0), at(11, 0)).await;

        let updated = w
            .service()
            .update(&saved.id, respan(at(10, 0), at(11, 30)), Overlap::Cancel)
            .await
            .unwrap()
            .value;

        assert_eq!(times(&updated), [(at(10, 0), at(11, 30))]);
        assert_eq!(w.rows().await, [updated]);
    }
}
