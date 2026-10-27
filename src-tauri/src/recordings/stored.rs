//! Stored suggestions as other apps write them (ADR 0028): checked and snapped like a recording, on one local
//! day, free to share time with anything until approved, and undoable. One may have no project yet (ADR 0030).

use super::checks::{check_tag, checked_note, invalid, snapped_spans, usable_project};
use super::ops::Op;
use super::{Change, RecordingInput, Recordings, new_spans};
use crate::db::new_id;
use crate::db::recordings::{self, RecordingPatch, Span};
use crate::db::stored_suggestions::{self, StoredSuggestion};
use crate::db::time::{Day, format_duration, now_ms};
use crate::error::AppResult;

const MAX_SOURCE_CHARS: usize = 40;

/// A recording's input whose project may be left for the owner to pick.
#[derive(Debug, Clone, PartialEq)]
pub struct SuggestionInput {
    pub project_id: Option<String>,
    pub spans: Vec<Span>,
    pub note: String,
    pub tag_id: Option<String>,
}

impl From<RecordingInput> for SuggestionInput {
    fn from(input: RecordingInput) -> Self {
        Self {
            project_id: Some(input.project_id),
            spans: input.spans,
            note: input.note,
            tag_id: input.tag_id,
        }
    }
}

fn checked_source(source: &str) -> AppResult<String> {
    let source = source.trim();
    if source.is_empty() || source.contains(['\n', '\r']) {
        return Err(invalid("source", "must be one line of text"));
    }
    if source.chars().count() > MAX_SOURCE_CHARS {
        return Err(invalid(
            "source",
            &format!("must be at most {MAX_SOURCE_CHARS} characters"),
        ));
    }
    Ok(source.to_string())
}

/// The spans snapped, by start, apart from each other, and the one local day they lie on.
fn checked_spans(spans: &[Span], snap_minutes: u32) -> AppResult<(Vec<Span>, Day)> {
    let spans = snapped_spans(spans, snap_minutes)?;
    recordings::check_spans(&new_spans(&spans))?;
    let day = Day::of(spans[0].start);
    if spans.iter().any(|s| Day::of(s.end - 1) != day) {
        return Err(invalid("spans", "must lie on one day"));
    }
    Ok((spans, day))
}

impl Recordings<'_> {
    pub async fn create_suggestion(
        &self,
        input: impl Into<SuggestionInput>,
        source: &str,
    ) -> AppResult<Change<StoredSuggestion>> {
        let input = input.into();
        let mut undo = self.undo.lock().await;
        let (spans, day) = checked_spans(&input.spans, self.snap_minutes)?;
        let note = checked_note(&input.note)?;
        let source = checked_source(source)?;
        let project = match &input.project_id {
            Some(id) => Some(usable_project(self.db, id, None).await?),
            None => None,
        };
        check_tag(self.db, input.tag_id.as_deref()).await?;
        let now = now_ms();
        let suggestion = StoredSuggestion {
            id: new_id(),
            project_id: project.as_ref().map(|p| p.id.clone()),
            note,
            tag_id: input.tag_id,
            source,
            day,
            spans,
            created_at: now,
            updated_at: now,
        };
        let op = Op::Suggestion {
            before: None,
            after: Some(suggestion.clone()),
        };
        let duration = format_duration(suggestion.duration());
        let label = match project {
            Some(project) => format!("Suggest {duration} for {}", project.name),
            None => format!("Suggest {duration}"),
        };
        let days = self.apply_and_push(&mut undo, label, op).await?;
        Ok(Change {
            value: suggestion,
            days,
        })
    }

    /// `patch` applied like a recording's; the source stays, and so does a project once picked.
    pub async fn update_suggestion(
        &self,
        id: &str,
        patch: RecordingPatch,
    ) -> AppResult<Change<StoredSuggestion>> {
        let mut undo = self.undo.lock().await;
        let current = stored_suggestions::get(self.db, id).await?;
        let (spans, day) = match &patch.spans {
            Some(spans) => checked_spans(spans, self.snap_minutes)?,
            None => (current.spans.clone(), current.day),
        };
        let note = match &patch.note {
            Some(note) => checked_note(note)?,
            None => current.note.clone(),
        };
        let project_id = match patch.project_id {
            Some(id) => {
                usable_project(self.db, &id, current.project_id.as_deref()).await?;
                Some(id)
            }
            None => current.project_id.clone(),
        };
        let tag_id = match patch.tag_id {
            Some(tag_id) => {
                check_tag(self.db, tag_id.as_deref()).await?;
                tag_id
            }
            None => current.tag_id.clone(),
        };
        let after = StoredSuggestion {
            project_id,
            note,
            tag_id,
            day,
            spans,
            updated_at: now_ms(),
            ..current.clone()
        };
        let unchanged = StoredSuggestion {
            updated_at: current.updated_at,
            ..after.clone()
        } == current;
        if unchanged {
            return Ok(Change::unchanged(current));
        }
        let op = Op::Suggestion {
            before: Some(current),
            after: Some(after.clone()),
        };
        let days = self
            .apply_and_push(&mut undo, "Edit suggestion".into(), op)
            .await?;
        Ok(Change { value: after, days })
    }

    pub async fn delete_suggestion(&self, id: &str) -> AppResult<Change<StoredSuggestion>> {
        let mut undo = self.undo.lock().await;
        let suggestion = stored_suggestions::get(self.db, id).await?;
        let op = Op::Suggestion {
            before: Some(suggestion.clone()),
            after: None,
        };
        let days = self
            .apply_and_push(&mut undo, "Delete suggestion".into(), op)
            .await?;
        Ok(Change {
            value: suggestion,
            days,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::projects;
    use crate::db::time::Ms;
    use crate::error::ErrorKind;
    use crate::recordings::testing::*;

    fn today() -> Day {
        Day::of(at(12, 0))
    }

    fn on_day(day: Day, hour: i64, minute: i64) -> Ms {
        day.bounds().0 + hour * HOUR + minute * MIN
    }

    fn local(hour: i64, minute: i64) -> Ms {
        on_day(today(), hour, minute)
    }

    async fn stored_rows(w: &World) -> Vec<StoredSuggestion> {
        let (from, to) = (today().prev().prev(), today().next().next());
        stored_suggestions::list_for_days(&w.db, from, to)
            .await
            .unwrap()
    }

    async fn suggest(w: &World, from: (i64, i64), to: (i64, i64)) -> StoredSuggestion {
        let input = w.input(local(from.0, from.1), local(to.0, to.1));
        w.service()
            .create_suggestion(input, "cli")
            .await
            .unwrap()
            .value
    }

    #[tokio::test]
    async fn create_snaps_the_times_trims_note_and_source_and_stores_it_on_its_day() {
        let w = World::new().await;
        let input = RecordingInput {
            note: "  review PR  ".into(),
            tag_id: Some(w.alpha.id.clone()),
            ..w.input(local(10, 2), local(10, 58))
        };

        let change = w
            .service()
            .create_suggestion(input, "  claude ")
            .await
            .unwrap();

        let saved = change.value;
        assert_eq!(
            saved,
            StoredSuggestion {
                id: saved.id.clone(),
                project_id: Some(w.project.id.clone()),
                note: "review PR".into(),
                tag_id: Some(w.alpha.id.clone()),
                source: "claude".into(),
                day: today(),
                spans: vec![span(local(10, 0), local(11, 0))],
                created_at: saved.created_at,
                updated_at: saved.created_at,
            }
        );
        assert_eq!(change.days, [today()]);
        assert_eq!(stored_rows(&w).await, [saved]);
        assert!(w.rows().await.is_empty());
    }

    #[tokio::test]
    async fn create_with_several_spans_snaps_and_sorts_them_into_one_suggestion() {
        let w = World::new().await;
        let input = input_of(
            &w,
            &[(local(16, 7), local(20, 13)), (local(14, 23), local(15, 2))],
        );

        let saved = w
            .service()
            .create_suggestion(input, "cli")
            .await
            .unwrap()
            .value;

        assert_eq!(
            saved.spans,
            [
                span(local(14, 25), local(15, 0)),
                span(local(16, 5), local(20, 15))
            ]
        );
        assert_eq!(w.status().await, undo_label("Suggest 4h 45m for Acme"));
    }

    #[tokio::test]
    async fn create_is_labelled_with_the_duration_and_the_project() {
        for (minutes, label) in [
            (45, "Suggest 45m for Acme"),
            (60, "Suggest 1h 00m for Acme"),
            (65, "Suggest 1h 05m for Acme"),
        ] {
            let w = World::new().await;
            let input = w.input(local(8, 0), local(8, 0) + minutes * MIN);

            w.service().create_suggestion(input, "cli").await.unwrap();

            assert_eq!(w.status().await, undo_label(label), "{minutes} min");
        }
    }

    #[tokio::test]
    async fn a_suggestion_may_share_time_with_a_recording() {
        let w = World::new().await;
        let recording = w.create(local(10, 0), local(11, 0)).await;

        let saved = suggest(&w, (10, 30), (11, 30)).await;

        assert_eq!(stored_rows(&w).await, [saved]);
        assert_eq!(w.rows().await, [recording]);
    }

    #[tokio::test]
    async fn a_source_of_forty_characters_is_accepted() {
        let w = World::new().await;
        let source = "x".repeat(40);

        let saved = w
            .service()
            .create_suggestion(w.input(local(10, 0), local(11, 0)), &source)
            .await
            .unwrap()
            .value;

        assert_eq!(saved.source, source);
    }

    #[tokio::test]
    async fn invalid_input_is_refused_naming_the_field_and_stores_nothing() {
        let w = World::new().await;
        projects::set_archived(&w.db, &w.other.id, true)
            .await
            .unwrap();
        let (_, midnight, _) = midnight();
        let valid = w.input(local(10, 0), local(11, 0));
        let cases: Vec<(&str, RecordingInput, String, &str)> = vec![
            (
                "a span across midnight",
                input_of(&w, &[(midnight - HOUR, midnight + HOUR)]),
                "cli".into(),
                "spans",
            ),
            (
                "spans on two days",
                input_of(
                    &w,
                    &[
                        (midnight - HOUR, midnight - 30 * MIN),
                        (midnight + HOUR, midnight + 2 * HOUR),
                    ],
                ),
                "cli".into(),
                "spans",
            ),
            (
                "spans overlapping each other",
                input_of(
                    &w,
                    &[(local(10, 0), local(11, 0)), (local(10, 30), local(12, 0))],
                ),
                "cli".into(),
                "spans",
            ),
            ("no span", input_of(&w, &[]), "cli".into(), "spans"),
            (
                "an end before the start",
                input_of(&w, &[(local(10, 0), local(9, 0))]),
                "cli".into(),
                "end",
            ),
            (
                "a note with a line break",
                RecordingInput {
                    note: "a\nb".into(),
                    ..valid.clone()
                },
                "cli".into(),
                "note",
            ),
            ("an empty source", valid.clone(), String::new(), "source"),
            ("a blank source", valid.clone(), "   ".into(), "source"),
            (
                "a source with a line break",
                valid.clone(),
                "a\nb".into(),
                "source",
            ),
            (
                "a source of 41 characters",
                valid.clone(),
                "x".repeat(41),
                "source",
            ),
            (
                "an archived project",
                RecordingInput {
                    project_id: w.other.id.clone(),
                    ..valid.clone()
                },
                "cli".into(),
                "projectId",
            ),
            (
                "an unknown tag",
                RecordingInput {
                    tag_id: Some("nope".into()),
                    ..valid.clone()
                },
                "cli".into(),
                "tagId",
            ),
        ];

        for (name, input, source, field) in cases {
            let err = w
                .service()
                .create_suggestion(input, &source)
                .await
                .unwrap_err();
            assert_eq!(field_of(&err).as_deref(), Some(field), "{name}");
        }

        assert!(stored_rows(&w).await.is_empty());
        assert_eq!(w.status().await, no_status());
    }

    #[tokio::test]
    async fn spans_on_two_days_say_they_must_lie_on_one_day() {
        let w = World::new().await;
        let (_, midnight, _) = midnight();

        let err = w
            .service()
            .create_suggestion(input_of(&w, &[(midnight - HOUR, midnight + HOUR)]), "cli")
            .await
            .unwrap_err();

        assert_eq!(err.field().unwrap().message, "must lie on one day");
    }

    #[tokio::test]
    async fn a_suggestion_for_an_unknown_project_is_not_found() {
        let w = World::new().await;
        let input = RecordingInput {
            project_id: "nope".into(),
            ..w.input(local(10, 0), local(11, 0))
        };

        let err = w
            .service()
            .create_suggestion(input, "cli")
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(stored_rows(&w).await.is_empty());
    }

    #[tokio::test]
    async fn update_changes_the_given_fields_and_keeps_the_source() {
        let w = World::new().await;
        let saved = suggest(&w, (10, 0), (11, 0)).await;
        let change = patch(|p| {
            p.note = Some("  later  ".into());
            p.project_id = Some(w.other.id.clone());
            p.tag_id = Some(Some(w.beta.id.clone()));
            p.spans = Some(vec![span(local(14, 2), local(14, 58))]);
        });

        let updated = w
            .service()
            .update_suggestion(&saved.id, change)
            .await
            .unwrap();

        assert_eq!(updated.days, [today()]);
        assert_eq!(
            updated.value,
            StoredSuggestion {
                project_id: Some(w.other.id.clone()),
                note: "later".into(),
                tag_id: Some(w.beta.id.clone()),
                spans: vec![span(local(14, 0), local(15, 0))],
                updated_at: updated.value.updated_at,
                ..saved
            }
        );
        assert_eq!(stored_rows(&w).await, [updated.value]);
        assert_eq!(w.status().await, undo_label("Edit suggestion"));
    }

    #[tokio::test]
    async fn update_clears_or_keeps_the_tag_depending_on_the_patch() {
        let w = World::new().await;
        let input = RecordingInput {
            tag_id: Some(w.alpha.id.clone()),
            ..w.input(local(10, 0), local(11, 0))
        };
        let saved = w
            .service()
            .create_suggestion(input, "cli")
            .await
            .unwrap()
            .value;

        let kept = w
            .service()
            .update_suggestion(&saved.id, patch(|p| p.note = Some("n".into())))
            .await
            .unwrap()
            .value;
        let cleared = w
            .service()
            .update_suggestion(&saved.id, patch(|p| p.tag_id = Some(None)))
            .await
            .unwrap()
            .value;

        assert_eq!(kept.tag_id, Some(w.alpha.id.clone()));
        assert_eq!(cleared.tag_id, None);
    }

    #[tokio::test]
    async fn update_to_another_day_moves_the_suggestion_and_reports_both_days() {
        let w = World::new().await;
        let saved = suggest(&w, (10, 0), (11, 0)).await;
        let tomorrow = today().next();

        let updated = w
            .service()
            .update_suggestion(
                &saved.id,
                respan(on_day(tomorrow, 10, 0), on_day(tomorrow, 11, 0)),
            )
            .await
            .unwrap();

        assert_eq!(updated.days, [today(), tomorrow]);
        assert_eq!(updated.value.day, tomorrow);
        assert_eq!(
            stored_suggestions::list_for_day(&w.db, tomorrow)
                .await
                .unwrap(),
            [updated.value]
        );
        assert!(
            stored_suggestions::list_for_day(&w.db, today())
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn an_update_that_changes_nothing_reports_no_day_and_leaves_no_undo_entry() {
        let w = World::new().await;
        let saved = suggest(&w, (10, 0), (11, 0)).await;
        let before = w.status().await;
        let same = [
            patch(|_| {}),
            patch(|p| p.project_id = saved.project_id.clone()),
            patch(|p| p.note = Some(saved.note.clone())),
            patch(|p| p.tag_id = Some(None)),
            respan(local(10, 2), local(10, 58)),
        ];

        for change in same {
            let result = w
                .service()
                .update_suggestion(&saved.id, change)
                .await
                .unwrap();

            assert!(result.days.is_empty());
            assert_eq!(result.value, saved);
        }

        assert_eq!(w.status().await, before);
        assert_eq!(stored_rows(&w).await, [saved]);
    }

    #[tokio::test]
    async fn update_refuses_invalid_patches_naming_the_field_and_changes_nothing() {
        let w = World::new().await;
        projects::set_archived(&w.db, &w.other.id, true)
            .await
            .unwrap();
        let saved = suggest(&w, (10, 0), (11, 0)).await;
        let (_, midnight, _) = midnight();
        let cases = [
            (
                "an end before the start",
                respan(local(12, 0), local(11, 0)),
                "end",
            ),
            ("no span", patch(|p| p.spans = Some(Vec::new())), "spans"),
            (
                "spans across midnight",
                respan(midnight - HOUR, midnight + HOUR),
                "spans",
            ),
            (
                "spans overlapping each other",
                patch(|p| {
                    p.spans = Some(vec![
                        span(local(8, 0), local(9, 30)),
                        span(local(9, 0), local(10, 0)),
                    ]);
                }),
                "spans",
            ),
            (
                "a note with a line break",
                patch(|p| p.note = Some("a\nb".into())),
                "note",
            ),
            (
                "an archived project",
                patch(|p| p.project_id = Some(w.other.id.clone())),
                "projectId",
            ),
            (
                "an unknown tag",
                patch(|p| p.tag_id = Some(Some("nope".into()))),
                "tagId",
            ),
        ];

        for (name, change, field) in cases {
            let err = w
                .service()
                .update_suggestion(&saved.id, change)
                .await
                .unwrap_err();
            assert_eq!(field_of(&err).as_deref(), Some(field), "{name}");
        }

        assert_eq!(stored_rows(&w).await, [saved]);
        assert_eq!(w.status().await, undo_label("Suggest 1h 00m for Acme"));
    }

    #[tokio::test]
    async fn update_of_an_unknown_suggestion_is_not_found() {
        let w = World::new().await;

        let err = w
            .service()
            .update_suggestion("nope", patch(|p| p.note = Some("n".into())))
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn a_suggestion_of_an_archived_project_can_still_be_edited() {
        let w = World::new().await;
        let saved = suggest(&w, (10, 0), (11, 0)).await;
        projects::set_archived(&w.db, &w.project.id, true)
            .await
            .unwrap();

        let updated = w
            .service()
            .update_suggestion(&saved.id, patch(|p| p.note = Some("n".into())))
            .await
            .unwrap()
            .value;

        assert_eq!(
            (updated.note.as_str(), updated.project_id.as_deref()),
            ("n", Some(w.project.id.as_str()))
        );
    }

    async fn suggest_without_project(
        w: &World,
        from: (i64, i64),
        to: (i64, i64),
    ) -> StoredSuggestion {
        let input = SuggestionInput {
            project_id: None,
            spans: vec![span(local(from.0, from.1), local(to.0, to.1))],
            note: "review".into(),
            tag_id: None,
        };
        w.service()
            .create_suggestion(input, "cli")
            .await
            .unwrap()
            .value
    }

    #[tokio::test]
    async fn a_suggestion_can_be_created_without_a_project_and_is_labelled_without_one() {
        let w = World::new().await;

        let saved = suggest_without_project(&w, (10, 0), (11, 0)).await;

        assert_eq!(saved.project_id, None);
        assert_eq!(stored_rows(&w).await, [saved]);
        assert_eq!(w.status().await, undo_label("Suggest 1h 00m"));
    }

    #[tokio::test]
    async fn a_suggestion_without_a_project_still_refuses_an_unknown_tag() {
        let w = World::new().await;
        let input = SuggestionInput {
            project_id: None,
            spans: vec![span(local(10, 0), local(11, 0))],
            note: String::new(),
            tag_id: Some("nope".into()),
        };

        let err = w
            .service()
            .create_suggestion(input, "cli")
            .await
            .unwrap_err();

        assert_eq!(field_of(&err).as_deref(), Some("tagId"));
        assert!(stored_rows(&w).await.is_empty());
    }

    #[tokio::test]
    async fn update_with_a_project_gives_a_suggestion_without_one_that_project() {
        let w = World::new().await;
        let saved = suggest_without_project(&w, (10, 0), (11, 0)).await;

        let updated = w
            .service()
            .update_suggestion(
                &saved.id,
                patch(|p| p.project_id = Some(w.other.id.clone())),
            )
            .await
            .unwrap();

        assert_eq!(updated.days, [today()]);
        assert_eq!(
            updated.value,
            StoredSuggestion {
                project_id: Some(w.other.id.clone()),
                updated_at: updated.value.updated_at,
                ..saved
            }
        );
        assert_eq!(stored_rows(&w).await, [updated.value]);
    }

    #[tokio::test]
    async fn update_with_an_archived_project_is_refused_for_a_suggestion_without_one() {
        let w = World::new().await;
        let saved = suggest_without_project(&w, (10, 0), (11, 0)).await;
        projects::set_archived(&w.db, &w.other.id, true)
            .await
            .unwrap();

        let err = w
            .service()
            .update_suggestion(
                &saved.id,
                patch(|p| p.project_id = Some(w.other.id.clone())),
            )
            .await
            .unwrap_err();

        assert_eq!(field_of(&err).as_deref(), Some("projectId"));
        assert_eq!(stored_rows(&w).await, [saved]);
    }

    #[tokio::test]
    async fn update_without_a_project_in_the_patch_keeps_a_suggestion_projectless() {
        let w = World::new().await;
        let saved = suggest_without_project(&w, (10, 0), (11, 0)).await;

        let updated = w
            .service()
            .update_suggestion(&saved.id, patch(|p| p.note = Some("later".into())))
            .await
            .unwrap()
            .value;

        assert_eq!(updated.project_id, None);
        assert_eq!(updated.note, "later");
    }

    #[tokio::test]
    async fn undo_then_redo_restores_the_creation_of_a_suggestion_without_a_project() {
        let w = World::new().await;
        let saved = suggest_without_project(&w, (10, 0), (11, 0)).await;

        let undone = w.service().undo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Suggest 1h 00m"));
        assert!(stored_rows(&w).await.is_empty());

        let redone = w.service().redo().await.unwrap();

        assert_eq!(redone.value.as_deref(), Some("Suggest 1h 00m"));
        assert_eq!(stored_rows(&w).await, [saved]);
    }

    #[tokio::test]
    async fn delete_removes_the_suggestion_and_is_labelled() {
        let w = World::new().await;
        let saved = suggest(&w, (10, 0), (11, 0)).await;

        let deleted = w.service().delete_suggestion(&saved.id).await.unwrap();

        assert_eq!(deleted.value, saved);
        assert_eq!(deleted.days, [today()]);
        assert!(stored_rows(&w).await.is_empty());
        assert_eq!(
            w.status().await.undo_label.as_deref(),
            Some("Delete suggestion")
        );
    }

    #[tokio::test]
    async fn delete_of_an_unknown_suggestion_is_not_found_and_not_undoable() {
        let w = World::new().await;

        let err = w.service().delete_suggestion("nope").await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(w.status().await, no_status());
    }

    #[tokio::test]
    async fn undo_then_redo_restores_a_creation_exactly() {
        let w = World::new().await;
        let saved = suggest(&w, (10, 0), (11, 0)).await;

        let undone = w.service().undo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Suggest 1h 00m for Acme"));
        assert_eq!(undone.days, [today()]);
        assert!(stored_rows(&w).await.is_empty());

        let redone = w.service().redo().await.unwrap();

        assert_eq!(redone.value.as_deref(), Some("Suggest 1h 00m for Acme"));
        assert_eq!(stored_rows(&w).await, [saved]);
    }

    #[tokio::test]
    async fn undo_then_redo_restores_an_edit_exactly() {
        let w = World::new().await;
        let saved = suggest(&w, (10, 0), (11, 0)).await;
        let updated = w
            .service()
            .update_suggestion(
                &saved.id,
                patch(|p| {
                    p.note = Some("after".into());
                    p.project_id = Some(w.other.id.clone());
                    p.spans = Some(vec![span(local(12, 0), local(13, 0))]);
                }),
            )
            .await
            .unwrap()
            .value;

        let undone = w.service().undo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Edit suggestion"));
        assert_eq!(stored_rows(&w).await, [saved]);

        w.service().redo().await.unwrap();

        assert_eq!(stored_rows(&w).await, [updated]);
    }

    #[tokio::test]
    async fn undo_then_redo_restores_a_deletion_exactly() {
        let w = World::new().await;
        let saved = suggest(&w, (10, 0), (11, 0)).await;
        w.service().delete_suggestion(&saved.id).await.unwrap();

        let undone = w.service().undo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Delete suggestion"));
        assert_eq!(undone.days, [today()]);
        assert_eq!(stored_rows(&w).await, [saved.clone()]);

        w.service().redo().await.unwrap();

        assert!(stored_rows(&w).await.is_empty());
    }

    #[tokio::test]
    async fn undoing_a_deletion_works_while_a_recording_covers_the_suggestions_time() {
        let w = World::new().await;
        let saved = suggest(&w, (10, 0), (11, 0)).await;
        let recording = w.create(local(10, 0), local(11, 0)).await;
        w.service().delete_suggestion(&saved.id).await.unwrap();

        w.service().undo().await.unwrap();

        assert_eq!(stored_rows(&w).await, [saved]);
        assert_eq!(w.rows().await, [recording]);
    }
}
