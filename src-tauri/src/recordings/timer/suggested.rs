//! A timer another app starts runs as a stored suggestion with an open start (ADR 0041): the owner reviews
//! it in the Day view like any suggestion, and nothing is recorded until it is approved.

use super::{TimerInput, after_stopping, same_timer};
use crate::db::new_id;
use crate::db::recordings::{self, Recording, RecordingSpan};
use crate::db::stored_suggestions::{self, StoredSuggestion};
use crate::db::time::{Day, Ms, now_ms};
use crate::error::{AppError, AppResult};
use crate::recordings::checks::{check_tag, checked_note, usable_project};
use crate::recordings::ops::Op;
use crate::recordings::stored::checked_source;
use crate::recordings::undo::UndoStack;
use crate::recordings::{Change, RecordingInput, Recordings, new_spans};
use crate::timers::{self, Timer};

fn same_fields(
    suggestion: &StoredSuggestion,
    project_id: &str,
    note: &str,
    tag_id: Option<&str>,
) -> bool {
    suggestion.project_id.as_deref() == Some(project_id)
        && suggestion.note == note
        && suggestion.tag_id.as_deref() == tag_id
}

impl Recordings<'_> {
    /// Starts a suggested timer now from `source`; a running timer stops where it starts. Today's finished
    /// stored suggestion with the same project, note, tag and source continues; else a new one runs. When
    /// the running timer already has that project, note and tag, nothing changes: the owner's own timer wins.
    pub async fn start_suggested_timer(
        &self,
        input: TimerInput,
        source: &str,
        now: Ms,
    ) -> AppResult<Change<Timer>> {
        let mut undo = self.undo.lock().await;
        let note = checked_note(&input.note)?;
        let source = checked_source(source)?;
        let project = usable_project(self.db, &input.project_id, None).await?;
        let tag_id = input.tag_id;
        check_tag(self.db, tag_id.as_deref()).await?;
        let running = timers::current(self.db).await?;
        let unchanged = match &running {
            Some(Timer::Suggestion(s)) => same_fields(s, &project.id, &note, tag_id.as_deref()),
            Some(Timer::Recording(r)) => same_timer(r, &project.id, &note, tag_id.as_deref()),
            None => false,
        };
        if unchanged && let Some(timer) = &running {
            return Ok(Change::unchanged(timer.clone()));
        }
        let start = self.start_after(running.as_ref(), now).await?;
        let continued = self
            .continued_suggestion(&project.id, &note, tag_id.as_deref(), &source, start)
            .await?;
        let timed = match &continued {
            Some(before) => StoredSuggestion {
                open_start: Some(start),
                updated_at: now,
                ..before.clone()
            },
            None => StoredSuggestion {
                id: new_id(),
                project_id: Some(project.id),
                note,
                tag_id,
                source,
                day: Day::of(start),
                spans: Vec::new(),
                created_at: now,
                updated_at: now,
                open_start: Some(start),
            },
        };
        let timing = Op::Suggestion {
            before: continued,
            after: Some(timed.clone()),
        };
        let label = format!("Suggest timer for {}", project.name);
        let op = after_stopping(running, start, now, timing, &label);
        let days = self.apply_and_push(&mut undo, label, op).await?;
        Ok(Change {
            value: Timer::Suggestion(timed),
            days,
        })
    }

    /// The latest finished stored suggestion of `start`'s day with these fields that ends by `start`.
    async fn continued_suggestion(
        &self,
        project_id: &str,
        note: &str,
        tag_id: Option<&str>,
        source: &str,
        start: Ms,
    ) -> AppResult<Option<StoredSuggestion>> {
        let last_end = |s: &StoredSuggestion| s.spans.last().map(|span| span.end);
        Ok(stored_suggestions::list_for_day(self.db, Day::of(start))
            .await?
            .into_iter()
            .filter(|s| !s.is_running() && s.source == source)
            .filter(|s| same_fields(s, project_id, note, tag_id))
            .filter(|s| last_end(s).is_some_and(|end| end <= start))
            .max_by_key(last_end))
    }

    /// The running suggestion with another project, note and tag; its times stay.
    pub(super) async fn update_suggested_timer(
        &self,
        undo: &mut UndoStack,
        running: StoredSuggestion,
        input: TimerInput,
        now: Ms,
    ) -> AppResult<Change<StoredSuggestion>> {
        let note = checked_note(&input.note)?;
        let project =
            usable_project(self.db, &input.project_id, running.project_id.as_deref()).await?;
        check_tag(self.db, input.tag_id.as_deref()).await?;
        if same_fields(&running, &project.id, &note, input.tag_id.as_deref()) {
            return Ok(Change::unchanged(running));
        }
        let after = StoredSuggestion {
            project_id: Some(project.id),
            note,
            tag_id: input.tag_id,
            updated_at: now,
            ..running.clone()
        };
        let op = Op::Suggestion {
            before: Some(running),
            after: Some(after.clone()),
        };
        let days = self
            .apply_and_push(undo, "Edit suggestion".into(), op)
            .await?;
        Ok(Change { value: after, days })
    }

    /// Approves the running suggestion `id`: `input` becomes the running timer's recording, its spans the
    /// closed ones and its open span starting at `open_start`, and the suggestion is deleted, in one change.
    pub async fn approve_suggested_timer(
        &self,
        id: &str,
        input: RecordingInput,
        open_start: Ms,
    ) -> AppResult<Change<Recording>> {
        let mut undo = self.undo.lock().await;
        let suggestion = stored_suggestions::get(self.db, id).await?;
        if !suggestion.is_running() {
            return Err(AppError::NotFound("The suggestion is gone".into()));
        }
        let note = checked_note(&input.note)?;
        let project = usable_project(self.db, &input.project_id, None).await?;
        check_tag(self.db, input.tag_id.as_deref()).await?;
        let mut spans = new_spans(&input.spans);
        spans.push(RecordingSpan::new(open_start, None));
        recordings::check_spans(&spans)?;
        let now = now_ms();
        let recording = Recording {
            id: new_id(),
            project_id: project.id,
            note,
            tag_id: input.tag_id,
            created_at: now,
            updated_at: now,
            spans,
        };
        let label = format!("Approve timer for {}", project.name);
        let op = Op::Batch {
            label: label.clone(),
            ops: vec![
                Op::Suggestion {
                    before: Some(suggestion),
                    after: None,
                },
                Op::Create {
                    recording: recording.clone(),
                },
            ],
        };
        self.commit(&mut undo, label, op, &recording.id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::projects;
    use crate::error::ErrorKind;
    use crate::recordings::testing::*;

    fn timer(w: &World) -> TimerInput {
        TimerInput {
            project_id: w.project.id.clone(),
            note: String::new(),
            tag_id: None,
        }
    }

    fn review(w: &World) -> TimerInput {
        TimerInput {
            note: "Review".into(),
            tag_id: Some(w.alpha.id.clone()),
            ..timer(w)
        }
    }

    fn other_project(w: &World) -> TimerInput {
        TimerInput {
            project_id: w.other.id.clone(),
            ..timer(w)
        }
    }

    fn suggestion_of(timer: Timer) -> StoredSuggestion {
        match timer {
            Timer::Suggestion(s) => s,
            Timer::Recording(r) => panic!("expected a suggestion, got recording {}", r.id),
        }
    }

    async fn stored_rows(w: &World) -> Vec<StoredSuggestion> {
        let day = Day::of(at(12, 0));
        stored_suggestions::list_for_days(&w.db, day.prev().prev(), day.next().next())
            .await
            .unwrap()
    }

    async fn suggest(w: &World, input: TimerInput, now: Ms) -> StoredSuggestion {
        suggestion_of(
            w.service()
                .start_suggested_timer(input, "cli", now)
                .await
                .unwrap()
                .value,
        )
    }

    #[tokio::test]
    async fn a_suggested_timer_starts_as_a_stored_suggestion_with_an_open_start_at_the_snapped_now()
    {
        let w = World::new().await;
        let input = TimerInput {
            note: "  Review  ".into(),
            tag_id: Some(w.alpha.id.clone()),
            ..timer(&w)
        };

        let change = w
            .service()
            .start_suggested_timer(input, "  claude ", at(10, 3))
            .await
            .unwrap();

        let started = suggestion_of(change.value);
        assert_eq!(
            started,
            StoredSuggestion {
                id: started.id.clone(),
                project_id: Some(w.project.id.clone()),
                note: "Review".into(),
                tag_id: Some(w.alpha.id.clone()),
                source: "claude".into(),
                day: Day::of(at(10, 5)),
                spans: Vec::new(),
                created_at: at(10, 3),
                updated_at: at(10, 3),
                open_start: Some(at(10, 5)),
            }
        );
        assert!(change.days.contains(&Day::of(at(10, 5))));
        assert_eq!(stored_rows(&w).await, [started]);
        assert!(w.rows().await.is_empty());
        assert_eq!(w.status().await, undo_label("Suggest timer for Acme"));
    }

    #[tokio::test]
    async fn a_suggested_timer_started_after_a_recording_ended_since_the_snapped_time_starts_where_it_ended()
     {
        let w = World::new().await;
        crate::test_support::recording(&w.db, &w.other.id, at(10, 0), Some(at(10, 6))).await;

        let started = suggest(&w, timer(&w), at(10, 7)).await;

        assert_eq!(started.open_start, Some(at(10, 6)));
    }

    #[tokio::test]
    async fn a_running_recording_with_other_fields_stops_where_the_suggested_timer_starts_in_one_undo_step()
     {
        let w = World::new().await;
        let first = w
            .service()
            .start_timer(timer(&w), at(10, 0))
            .await
            .unwrap()
            .value;
        let before = w.rows().await;

        let started = suggest(&w, other_project(&w), at(10, 32)).await;

        let rows = w.rows().await;
        assert_eq!(rows.len(), 1);
        assert_eq!(
            (rows[0].id.as_str(), times(&rows[0])),
            (first.id.as_str(), vec![(at(10, 0), at(10, 30))])
        );
        assert_eq!(started.open_start, Some(at(10, 30)));
        assert_eq!(stored_rows(&w).await, [started]);
        assert_eq!(w.status().await, undo_label("Suggest timer for Beta"));

        let undone = w.service().undo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Suggest timer for Beta"));
        assert_eq!(w.rows().await, before);
        assert!(stored_rows(&w).await.is_empty());
    }

    #[tokio::test]
    async fn a_running_suggested_timer_stops_where_the_next_one_starts_and_undo_restores_both() {
        let w = World::new().await;
        let first = suggest(&w, timer(&w), at(10, 0)).await;

        let second = suggest(&w, other_project(&w), at(10, 32)).await;

        let stopped = StoredSuggestion {
            spans: vec![span(at(10, 0), at(10, 30))],
            open_start: None,
            updated_at: at(10, 32),
            ..first.clone()
        };
        assert_eq!(stored_rows(&w).await, [stopped, second.clone()]);
        assert_eq!(
            stored_suggestions::running(&w.db).await.unwrap(),
            Some(second)
        );

        w.service().undo().await.unwrap();

        assert_eq!(stored_rows(&w).await, [first]);
    }

    #[tokio::test]
    async fn a_suggested_timer_continues_the_latest_finished_one_of_the_day_with_the_same_fields() {
        let w = World::new().await;
        let first = suggest(&w, review(&w), at(10, 0)).await;
        w.service().stop_timer(at(10, 47)).await.unwrap();
        let stopped = stored_suggestions::get(&w.db, &first.id).await.unwrap();
        assert_eq!(stopped.spans, [span(at(10, 0), at(10, 45))]);

        let continued = suggest(&w, review(&w), at(11, 3)).await;

        assert_eq!(
            continued,
            StoredSuggestion {
                open_start: Some(at(11, 5)),
                updated_at: at(11, 3),
                ..stopped.clone()
            }
        );
        assert_eq!(stored_rows(&w).await, [continued]);

        w.service().undo().await.unwrap();

        assert_eq!(stored_rows(&w).await, [stopped]);
    }

    #[tokio::test]
    async fn a_suggested_timer_with_another_project_note_tag_or_source_starts_a_new_suggestion() {
        let w = World::new().await;
        let first = suggest(&w, review(&w), at(10, 0)).await;
        w.service().stop_timer(at(10, 47)).await.unwrap();
        let cases = [
            (
                "project",
                TimerInput {
                    project_id: w.other.id.clone(),
                    ..review(&w)
                },
                "cli",
            ),
            (
                "note",
                TimerInput {
                    note: "Other".into(),
                    ..review(&w)
                },
                "cli",
            ),
            (
                "tag",
                TimerInput {
                    tag_id: Some(w.beta.id.clone()),
                    ..review(&w)
                },
                "cli",
            ),
            (
                "no tag",
                TimerInput {
                    tag_id: None,
                    ..review(&w)
                },
                "cli",
            ),
            ("source", review(&w), "claude"),
        ];

        for (name, input, source) in cases {
            let started = suggestion_of(
                w.service()
                    .start_suggested_timer(input, source, at(11, 0))
                    .await
                    .unwrap()
                    .value,
            );

            assert_ne!(started.id, first.id, "{name}");
            assert_eq!(stored_rows(&w).await.len(), 2, "{name}");
            w.service().undo().await.unwrap();
        }
    }

    #[tokio::test]
    async fn starting_the_suggested_timer_that_already_runs_changes_nothing() {
        let w = World::new().await;
        let running = suggest(&w, review(&w), at(10, 0)).await;
        let status = w.status().await;

        let again = w
            .service()
            .start_suggested_timer(review(&w), "claude", at(10, 20))
            .await
            .unwrap();

        assert_eq!(again.value, Timer::Suggestion(running.clone()));
        assert!(again.days.is_empty());
        assert_eq!(stored_rows(&w).await, [running]);
        assert_eq!(w.status().await, status);
    }

    #[tokio::test]
    async fn a_suggested_timer_with_the_fields_of_the_running_recording_leaves_the_recording_running()
     {
        let w = World::new().await;
        let running = w
            .service()
            .start_timer(review(&w), at(10, 0))
            .await
            .unwrap()
            .value;
        let status = w.status().await;

        let again = w
            .service()
            .start_suggested_timer(review(&w), "claude", at(10, 20))
            .await
            .unwrap();

        assert_eq!(again.value, Timer::Recording(running.clone()));
        assert!(again.days.is_empty());
        assert_eq!(w.rows().await, [running]);
        assert!(stored_rows(&w).await.is_empty());
        assert_eq!(w.status().await, status);
    }

    #[tokio::test]
    async fn a_suggested_timer_is_refused_for_unusable_fields_and_nothing_is_written() {
        let w = World::new().await;
        projects::set_archived(&w.db, &w.other.id, true)
            .await
            .unwrap();
        let cases = [
            (
                "archived project",
                other_project(&w),
                "cli".to_string(),
                ErrorKind::InvalidInput,
                Some("projectId"),
            ),
            (
                "unknown project",
                TimerInput {
                    project_id: "nope".into(),
                    ..timer(&w)
                },
                "cli".into(),
                ErrorKind::NotFound,
                None,
            ),
            (
                "note with a line break",
                TimerInput {
                    note: "a\nb".into(),
                    ..timer(&w)
                },
                "cli".into(),
                ErrorKind::InvalidInput,
                Some("note"),
            ),
            (
                "unknown tag",
                TimerInput {
                    tag_id: Some("nope".into()),
                    ..timer(&w)
                },
                "cli".into(),
                ErrorKind::InvalidInput,
                Some("tagId"),
            ),
            (
                "empty source",
                timer(&w),
                "  ".into(),
                ErrorKind::InvalidInput,
                Some("source"),
            ),
            (
                "source with a line break",
                timer(&w),
                "a\nb".into(),
                ErrorKind::InvalidInput,
                Some("source"),
            ),
            (
                "source of 41 characters",
                timer(&w),
                "x".repeat(41),
                ErrorKind::InvalidInput,
                Some("source"),
            ),
        ];
        for (name, input, source, kind, field) in cases {
            let err = w
                .service()
                .start_suggested_timer(input, &source, at(10, 0))
                .await
                .unwrap_err();

            assert_eq!(err.kind(), kind, "{name}");
            assert_eq!(field_of(&err).as_deref(), field, "{name}");
        }
        assert!(stored_rows(&w).await.is_empty());
        assert_eq!(w.status().await, no_status());
    }
}
