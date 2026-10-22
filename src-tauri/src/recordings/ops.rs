//! Every change to recordings, and to the dismissed suggestions, is an `Op` with an exact inverse, applied in
//! one transaction (ADR 0010).

use std::collections::HashSet;

use sqlx::SqliteConnection;

use crate::db::Db;
use crate::db::dismissed::{self, Dismissal};
use crate::db::recordings::{self, Recording};
use crate::db::time::Day;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Create {
        recording: Recording,
    },
    Update {
        before: Recording,
        after: Recording,
    },
    Delete {
        recording: Recording,
    },
    Dismiss {
        dismissal: Dismissal,
    },
    Undismiss {
        dismissal: Dismissal,
    },
    /// Applied in order; a replace is trims and deletes followed by the new recording.
    Batch {
        label: String,
        ops: Vec<Op>,
    },
}

impl Op {
    pub fn inverse(&self) -> Op {
        match self {
            Op::Create { recording } => Op::Delete {
                recording: recording.clone(),
            },
            Op::Delete { recording } => Op::Create {
                recording: recording.clone(),
            },
            Op::Update { before, after } => Op::Update {
                before: after.clone(),
                after: before.clone(),
            },
            Op::Dismiss { dismissal } => Op::Undismiss {
                dismissal: dismissal.clone(),
            },
            Op::Undismiss { dismissal } => Op::Dismiss {
                dismissal: dismissal.clone(),
            },
            Op::Batch { label, ops } => Op::Batch {
                label: label.clone(),
                ops: ops.iter().rev().map(Op::inverse).collect(),
            },
        }
    }

    /// Every recording the op writes, as written.
    fn written(&self) -> Vec<&Recording> {
        match self {
            Op::Create { recording } => vec![recording],
            Op::Update { after, .. } => vec![after],
            Op::Delete { .. } | Op::Dismiss { .. } | Op::Undismiss { .. } => Vec::new(),
            Op::Batch { ops, .. } => ops.iter().flat_map(Op::written).collect(),
        }
    }

    /// Every recording the op touches, before and after.
    pub fn recordings(&self) -> Vec<&Recording> {
        match self {
            Op::Create { recording } | Op::Delete { recording } => vec![recording],
            Op::Update { before, after } => vec![before, after],
            Op::Dismiss { .. } | Op::Undismiss { .. } => Vec::new(),
            Op::Batch { ops, .. } => ops.iter().flat_map(Op::recordings).collect(),
        }
    }

    /// The days whose dismissed suggestions the op changes.
    pub fn dismissal_days(&self) -> Vec<Day> {
        match self {
            Op::Dismiss { dismissal } | Op::Undismiss { dismissal } => vec![dismissal.day],
            Op::Batch { ops, .. } => ops.iter().flat_map(Op::dismissal_days).collect(),
            _ => Vec::new(),
        }
    }
}

/// Applies `op` in one transaction and refuses it with `overlap` when a span it writes would share time
/// with another recording's; then nothing changes.
pub async fn apply(db: &Db, op: &Op) -> AppResult<()> {
    let mut tx = db.pool().begin().await?;
    run(&mut tx, op).await?;
    check_no_overlap(&mut tx, op).await?;
    tx.commit().await?;
    Ok(())
}

async fn run(conn: &mut SqliteConnection, op: &Op) -> AppResult<()> {
    match op {
        Op::Create { recording }
        | Op::Update {
            after: recording, ..
        } => recordings::put_on(conn, recording).await,
        Op::Delete { recording } => recordings::remove_on(conn, &recording.id).await,
        Op::Dismiss { dismissal } => dismissed::put_on(conn, dismissal).await,
        Op::Undismiss { dismissal } => dismissed::remove_on(conn, dismissal).await,
        Op::Batch { ops, .. } => {
            for op in ops {
                Box::pin(run(conn, op)).await?;
            }
            Ok(())
        }
    }
}

async fn check_no_overlap(conn: &mut SqliteConnection, op: &Op) -> AppResult<()> {
    let mut seen = HashSet::new();
    for recording in op.written() {
        if !seen.insert(recording.id.as_str()) {
            continue;
        }
        for span in &recording.spans {
            let end = span.end.unwrap_or(i64::MAX);
            let others =
                recordings::overlapping_on(conn, span.start, end, Some(&recording.id)).await?;
            if !others.is_empty() {
                return Err(overlap_error(others));
            }
        }
    }
    Ok(())
}

/// `overlap`, with the recordings in the way as `details` for the page to list.
pub fn overlap_error(recordings: Vec<Recording>) -> AppError {
    let message = match recordings.len() {
        1 => "Overlaps another recording".to_string(),
        n => format!("Overlaps {n} recordings"),
    };
    AppError::Overlap {
        message,
        details: serde_json::to_value(&recordings).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::recordings::{NewRecording, RecordingSpan};
    use crate::error::ErrorKind;
    use crate::recordings::testing::*;
    use crate::rules::testing::save_rule;
    use crate::rules::types::{RuleField, RuleOp};
    use crate::test_support;

    fn create(recording: &Recording) -> Op {
        Op::Create {
            recording: recording.clone(),
        }
    }

    fn delete(recording: &Recording) -> Op {
        Op::Delete {
            recording: recording.clone(),
        }
    }

    fn update(before: &Recording, after: &Recording) -> Op {
        Op::Update {
            before: before.clone(),
            after: after.clone(),
        }
    }

    fn dismissal(rule_id: &str, start: i64, end: i64) -> Dismissal {
        Dismissal {
            rule_id: rule_id.into(),
            day: Day::of(at(12, 0)),
            start,
            end,
        }
    }

    fn dismiss(dismissal: &Dismissal) -> Op {
        Op::Dismiss {
            dismissal: dismissal.clone(),
        }
    }

    fn undismiss(dismissal: &Dismissal) -> Op {
        Op::Undismiss {
            dismissal: dismissal.clone(),
        }
    }

    fn batch(ops: Vec<Op>) -> Op {
        Op::Batch {
            label: "batch".into(),
            ops,
        }
    }

    /// Stored through the repository, so its timestamps are the database's.
    async fn saved(w: &World, tag_id: Option<String>, start: i64, end: i64) -> Recording {
        recordings::insert(
            &w.db,
            NewRecording {
                id: crate::db::new_id(),
                project_id: w.project.id.clone(),
                spans: vec![(start, Some(end))],
                note: "note".into(),
                tag_id,
            },
        )
        .await
        .unwrap()
    }

    /// One closed span under `id`.
    fn one_span(id: &str, start: i64, end: i64) -> Vec<RecordingSpan> {
        vec![RecordingSpan {
            id: id.into(),
            start,
            end: Some(end),
        }]
    }

    /// `recording` with its only span's id kept and its times replaced.
    fn retimed(recording: &Recording, start: i64, end: i64) -> Recording {
        Recording {
            spans: one_span(&recording.spans[0].id, start, end),
            ..recording.clone()
        }
    }

    /// Not stored; belongs to the world's project.
    fn unsaved(w: &World, id: &str, start: i64, end: i64) -> Recording {
        Recording {
            project_id: w.project.id.clone(),
            ..row(id, start, end)
        }
    }

    #[test]
    fn the_inverse_of_each_op_undoes_it() {
        let (a, b, c) = (row("a", 0, 10), row("b", 20, 30), row("c", 40, 50));
        let d = dismissal("rule", 60, 70);
        let moved = Recording {
            updated_at: 3_000,
            ..retimed(&a, 5, 10)
        };
        let cases = [
            ("create", create(&a), delete(&a)),
            ("delete", delete(&a), create(&a)),
            ("update", update(&a, &moved), update(&moved, &a)),
            ("dismiss", dismiss(&d), undismiss(&d)),
            ("undismiss", undismiss(&d), dismiss(&d)),
            (
                "batch",
                batch(vec![delete(&b), update(&a, &moved), create(&c)]),
                batch(vec![delete(&c), update(&moved, &a), create(&b)]),
            ),
            (
                "batch with a dismissal",
                batch(vec![create(&c), dismiss(&d)]),
                batch(vec![undismiss(&d), delete(&c)]),
            ),
        ];
        for (name, op, inverse) in cases {
            assert_eq!(op.inverse(), inverse, "{name}");
            assert_eq!(inverse.inverse(), op, "{name} twice");
        }
    }

    #[test]
    fn the_inverse_of_a_batch_keeps_its_label() {
        let op = Op::Batch {
            label: "Record 1h 00m for Acme".into(),
            ops: vec![],
        };

        assert_eq!(
            op.inverse(),
            Op::Batch {
                label: "Record 1h 00m for Acme".into(),
                ops: vec![]
            }
        );
    }

    #[tokio::test]
    async fn applying_a_create_and_then_its_inverse_leaves_no_trace() {
        let w = World::new().await;
        let recording = Recording {
            project_id: w.project.id.clone(),
            tag_id: Some(w.alpha.id.clone()),
            ..row("r", at(10, 0), at(11, 0))
        };
        let op = create(&recording);

        apply(&w.db, &op).await.unwrap();
        assert_eq!(w.rows().await, [recording]);

        apply(&w.db, &op.inverse()).await.unwrap();
        assert!(w.rows().await.is_empty());
    }

    #[tokio::test]
    async fn applying_an_update_and_then_its_inverse_restores_the_exact_row() {
        let w = World::new().await;
        let before = saved(&w, Some(w.alpha.id.clone()), at(10, 0), at(11, 0)).await;
        let after = Recording {
            project_id: w.other.id.clone(),
            note: "changed".into(),
            tag_id: Some(w.beta.id.clone()),
            created_at: before.created_at + 1,
            updated_at: before.updated_at + 1_000,
            ..retimed(&before, at(10, 30), at(11, 30))
        };
        let op = update(&before, &after);

        apply(&w.db, &op).await.unwrap();
        assert_eq!(w.rows().await, [after]);

        apply(&w.db, &op.inverse()).await.unwrap();
        assert_eq!(w.rows().await, [before]);
    }

    #[tokio::test]
    async fn applying_a_delete_and_then_its_inverse_restores_the_exact_row() {
        let w = World::new().await;
        let before = saved(&w, Some(w.beta.id.clone()), at(10, 0), at(11, 0)).await;
        let op = delete(&before);

        apply(&w.db, &op).await.unwrap();
        assert!(w.rows().await.is_empty());

        apply(&w.db, &op.inverse()).await.unwrap();
        assert_eq!(w.rows().await, [before]);
    }

    #[tokio::test]
    async fn applying_a_batch_and_then_its_inverse_restores_every_row() {
        let w = World::new().await;
        let trimmed = saved(&w, Some(w.alpha.id.clone()), at(10, 0), at(11, 0)).await;
        let removed = saved(&w, Some(w.beta.id.clone()), at(11, 0), at(12, 0)).await;
        let before = w.rows().await;
        let shorter = Recording {
            updated_at: trimmed.updated_at + 1_000,
            ..retimed(&trimmed, at(10, 0), at(10, 30))
        };
        let added = Recording {
            id: "added".into(),
            spans: one_span("added-span", at(10, 30), at(12, 0)),
            tag_id: None,
            ..shorter.clone()
        };
        let op = batch(vec![
            update(&trimmed, &shorter),
            delete(&removed),
            create(&added),
        ]);

        apply(&w.db, &op).await.unwrap();
        assert_eq!(w.rows().await, [shorter, added]);

        apply(&w.db, &op.inverse()).await.unwrap();
        assert_eq!(w.rows().await, before);
    }

    #[tokio::test]
    async fn applying_a_dismissal_and_then_its_inverse_leaves_no_trace() {
        let w = World::new().await;
        let conditions = [(RuleField::App, RuleOp::Is, "Slack")];
        let rule = save_rule(&w.db, "Slack", &w.project.id, &conditions).await;
        let d = dismissal(&rule.id, at(9, 0), at(10, 0));
        let listed = || dismissed::list_for_day(&w.db, d.day);

        apply(&w.db, &dismiss(&d)).await.unwrap();
        assert_eq!(listed().await.unwrap(), [d.clone()]);

        apply(&w.db, &dismiss(&d).inverse()).await.unwrap();
        assert!(listed().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn dismissing_for_a_rule_that_is_gone_applies_without_writing() {
        let w = World::new().await;
        let d = dismissal("gone", at(9, 0), at(10, 0));

        apply(&w.db, &dismiss(&d)).await.unwrap();

        assert!(
            dismissed::list_for_day(&w.db, d.day)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_batch_with_a_failing_step_applies_nothing() {
        let w = World::new().await;
        let existing = saved(&w, None, at(10, 0), at(11, 0)).await;
        let new = unsaved(&w, "new", at(12, 0), at(13, 0));
        let missing = row("missing", at(14, 0), at(15, 0));
        let ops = [
            batch(vec![create(&new), delete(&missing)]),
            batch(vec![delete(&existing), delete(&missing)]),
        ];

        for op in ops {
            let err = apply(&w.db, &op).await.unwrap_err();

            assert_eq!(err.kind(), ErrorKind::NotFound);
            assert_eq!(w.rows().await, [existing.clone()]);
        }
    }

    #[tokio::test]
    async fn an_op_that_would_overlap_is_refused_listing_the_recordings_and_changes_nothing() {
        let w = World::new().await;
        let first = saved(&w, None, at(10, 0), at(11, 0)).await;
        let second = saved(&w, None, at(12, 0), at(13, 0)).await;
        let overlapping = Recording {
            id: "new".into(),
            spans: one_span("new-span", at(10, 30), at(12, 30)),
            tag_id: None,
            ..first.clone()
        };
        let ops = [
            ("create", create(&overlapping)),
            (
                "update",
                update(&second, &retimed(&second, at(10, 30), at(13, 0))),
            ),
            ("batch", batch(vec![delete(&second), create(&overlapping)])),
        ];

        for (name, op) in ops {
            let err = apply(&w.db, &op).await.unwrap_err();

            assert_eq!(err.kind(), ErrorKind::Overlap, "{name}");
            assert_eq!(w.rows().await, [first.clone(), second.clone()], "{name}");
        }
    }

    #[tokio::test]
    async fn the_overlap_error_lists_the_recordings_in_the_way() {
        let w = World::new().await;
        let first = saved(&w, Some(w.alpha.id.clone()), at(10, 0), at(11, 0)).await;
        let overlapping = Recording {
            id: "new".into(),
            spans: one_span("new-span", at(10, 30), at(11, 30)),
            ..first.clone()
        };

        let err = apply(&w.db, &create(&overlapping)).await.unwrap_err();

        let AppError::Overlap { details, .. } = err else {
            panic!("not an overlap: {err:?}");
        };
        assert_eq!(
            serde_json::from_value::<Vec<Recording>>(details).unwrap(),
            [first]
        );
    }

    #[tokio::test]
    async fn a_batch_may_overlap_in_between_when_it_ends_clear() {
        let w = World::new().await;
        let existing = saved(&w, None, at(10, 0), at(11, 0)).await;
        let new = Recording {
            id: "new".into(),
            spans: one_span("new-span", at(10, 30), at(11, 30)),
            ..existing.clone()
        };
        let trimmed = retimed(&existing, at(10, 0), at(10, 30));
        let op = batch(vec![create(&new), update(&existing, &trimmed)]);

        apply(&w.db, &op).await.unwrap();

        assert_eq!(
            spans(&w.rows().await),
            [(at(10, 0), at(10, 30)), (at(10, 30), at(11, 30))]
        );
    }

    #[tokio::test]
    async fn a_running_recording_counts_as_overlapping_everything_after_its_start() {
        let w = World::new().await;
        let timer = test_support::recording(&w.db, &w.project.id, at(10, 0), None).await;
        let later = unsaved(&w, "later", at(20, 0), at(21, 0));

        let err = apply(&w.db, &create(&later)).await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::Overlap);
        assert_eq!(w.rows().await, [timer]);
    }

    #[test]
    fn the_overlap_error_names_how_many_recordings_are_in_the_way() {
        let one = overlap_error(vec![row("a", 0, 10)]);
        let two = overlap_error(vec![row("a", 0, 10), row("b", 20, 30)]);

        assert_eq!(one.kind(), ErrorKind::Overlap);
        assert_eq!(one.to_string(), "Overlaps another recording");
        assert_eq!(two.to_string(), "Overlaps 2 recordings");
    }

    #[test]
    fn the_overlap_error_carries_the_recordings_as_camel_case_json() {
        let err = overlap_error(vec![row("a", 0, 10)]);

        assert_eq!(
            err.details(),
            Some(&serde_json::json!([{
                "id": "a",
                "projectId": "p",
                "note": "",
                "tagId": null,
                "createdAt": 1_000,
                "updatedAt": 2_000,
                "spans": [{"id": "a-span", "start": 0, "end": 10}]
            }]))
        );
    }

    #[tokio::test]
    async fn a_second_span_that_overlaps_another_recording_refuses_the_op() {
        let w = World::new().await;
        let existing = saved(&w, None, at(14, 0), at(15, 0)).await;
        let two_spans = Recording {
            id: "new".into(),
            spans: vec![
                RecordingSpan {
                    id: "s1".into(),
                    start: at(10, 0),
                    end: Some(at(11, 0)),
                },
                RecordingSpan {
                    id: "s2".into(),
                    start: at(14, 30),
                    end: Some(at(16, 0)),
                },
            ],
            ..existing.clone()
        };

        let err = apply(&w.db, &create(&two_spans)).await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::Overlap);
        assert_eq!(w.rows().await, [existing]);
    }

    #[tokio::test]
    async fn the_spans_of_one_recording_never_conflict_with_each_other_or_its_older_self() {
        let w = World::new().await;
        let before = w
            .create_spans(&[(at(10, 0), at(11, 0)), (at(11, 0), at(12, 0))])
            .await;
        let grown = Recording {
            spans: vec![
                RecordingSpan {
                    end: Some(at(11, 30)),
                    ..before.spans[0].clone()
                },
                RecordingSpan {
                    start: at(11, 30),
                    ..before.spans[1].clone()
                },
            ],
            ..before.clone()
        };

        apply(&w.db, &update(&before, &grown)).await.unwrap();

        assert_eq!(w.rows().await, [grown]);
    }

    #[tokio::test]
    async fn a_cut_span_and_its_inverse_restore_the_exact_span_ids() {
        let w = World::new().await;
        let before = w.create(at(10, 0), at(11, 0)).await;
        let cut = Recording {
            spans: vec![
                RecordingSpan {
                    end: Some(at(10, 20)),
                    ..before.spans[0].clone()
                },
                RecordingSpan {
                    id: "right".into(),
                    start: at(10, 40),
                    end: Some(at(11, 0)),
                },
            ],
            ..before.clone()
        };
        let op = update(&before, &cut);

        apply(&w.db, &op).await.unwrap();
        assert_eq!(w.rows().await, [cut]);

        apply(&w.db, &op.inverse()).await.unwrap();
        assert_eq!(w.rows().await, [before]);
    }
}
