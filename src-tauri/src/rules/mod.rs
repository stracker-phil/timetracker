//! Rules ("activity like this belongs to project X") and the suggested recordings they produce. Rules never
//! write recordings; the owner reviews each suggestion: approving it is an ordinary, undoable create,
//! dismissing it keeps its time from being suggested again.

pub mod commands;
pub mod matching;
mod review;
pub mod store;
pub mod suggest;
#[cfg(test)]
pub(crate) mod testing;
pub mod types;

use crate::calendar::titles::day_title;
use crate::day::transform::continuity_ms;
use crate::day::{ActivityBlock, DayView, Names, TimesheetCard};
use crate::db::Db;
use crate::db::dismissed::Dismissal;
use crate::db::time::{Day, Ms, now_ms};
use crate::error::AppResult;
use crate::store::settings::{DayTitle, Settings};
use matching::{Compiled, compile, matches};
pub use review::{approve, approve_all, approve_edited, approve_rule, dismiss};
use types::{PreviewHit, RuleDay, RuleInput, RulePreview, Suggestion};

/// Keeps the preview's reply small; its total still counts every hit.
pub const PREVIEW_HITS: usize = 200;

/// The blocks rules look at, by start.
pub(crate) fn candidates(view: &DayView) -> Vec<&ActivityBlock> {
    view.rule_blocks.iter().collect()
}

/// The view's suggestions under `rules` (compiled, in order), without the `dismissed` time, each placed
/// among the timesheet's cards.
pub fn day_suggestions(
    view: &DayView,
    rules: &[Compiled],
    names: &Names,
    settings: &Settings,
    dismissed: &[Dismissal],
) -> Vec<Suggestion> {
    if rules.is_empty() {
        return Vec::new();
    }
    let mut suggestions = suggest::suggest(
        &candidates(view),
        &view.recordings,
        &view.barriers,
        rules,
        names,
        continuity_ms(settings.timeline.continuity),
        dismissed,
    );
    for suggestion in &mut suggestions {
        suggestion.next_card = next_card(&view.timesheet, suggestion.start);
    }
    suggestions
}

/// The first card starting after `start`, which a suggestion at `start` goes before.
fn next_card(timesheet: &[TimesheetCard], start: Ms) -> Option<String> {
    timesheet
        .iter()
        .find(|card| card.start > start)
        .map(|card| card.id.clone())
}

pub async fn suggestions_of(db: &Db, day: Day, settings: &Settings) -> AppResult<Vec<Suggestion>> {
    Ok(crate::day::load(db, day, settings, now_ms())
        .await?
        .suggestions)
}

/// Each active rule's `day`, in position order.
pub async fn day_of_rules(db: &Db, day: Day, settings: &Settings) -> AppResult<Vec<RuleDay>> {
    let view = crate::day::load(db, day, settings, now_ms()).await?;
    let rules = compile(&store::active(db).await?);
    let matched = suggest::matched_ms(&candidates(&view), &rules);
    Ok(rules
        .iter()
        .zip(matched)
        .map(|(rule, matched_ms)| {
            let pending: Vec<&Suggestion> = view
                .suggestions
                .iter()
                .filter(|s| s.rule_ids.contains(&rule.rule_id))
                .collect();
            RuleDay {
                rule_id: rule.rule_id.clone(),
                matched_ms,
                pending: pending.len() as u32,
                pending_ms: pending.iter().map(|s| s.end - s.start).sum(),
            }
        })
        .collect())
}

/// The blocks of the last `days` days up to `today` that `input`'s conditions match, on their own.
pub async fn preview(
    db: &Db,
    settings: &Settings,
    input: &RuleInput,
    today: Day,
    days: u32,
) -> AppResult<RulePreview> {
    store::check_conditions(&input.conditions)?;
    let rule = Compiled::conditions(&input.conditions);
    let now = now_ms();
    let mut hits = Vec::new();
    let (mut total_ms, mut hit_days) = (0, 0);
    let mut day = today;
    for _ in 0..days {
        let view = crate::day::view_of(db, day, settings, now).await?;
        let mut found: Vec<&ActivityBlock> = candidates(&view)
            .into_iter()
            .filter(|b| matches(&rule, b))
            .collect();
        if !found.is_empty() {
            hit_days += 1;
        }
        total_ms += found.iter().map(|b| b.end - b.start).sum::<Ms>();
        found.reverse();
        let day_label = day_title(day, DayTitle::Short, today);
        hits.extend(found.into_iter().map(|block| PreviewHit {
            block: block.clone(),
            day,
            day_label: day_label.clone(),
        }));
        day = day.prev();
    }
    hits.truncate(PREVIEW_HITS);
    Ok(RulePreview {
        hits,
        total_ms,
        days: hit_days,
    })
}

#[cfg(test)]
mod tests {
    use tokio::sync::Mutex;

    use super::testing::*;
    use super::types::RuleField::*;
    use super::types::{Rule, RuleOp};
    use super::*;
    use crate::db::clients::{self, ClientInput};
    use crate::db::projects::{self, Project, ProjectInput};
    use crate::db::recordings as stored;
    use crate::db::recordings::Recording;
    use crate::db::tags;
    use crate::error::ErrorKind;
    use crate::recordings::undo::UndoStack;
    use crate::recordings::{Overlap, RecordingInput, Recordings, Span};
    use crate::store::settings::Continuity;
    use crate::test_support::{activity, project};

    fn day(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    struct World {
        db: Db,
        undo: Mutex<UndoStack>,
        project: Project,
        tag_id: String,
        settings: Settings,
    }

    impl World {
        async fn new() -> Self {
            let db = Db::open_memory().await.unwrap();
            let project = project(&db, "Portal").await;
            let tag_id = tags::insert(&db, "billing").await.unwrap().id;
            Self {
                db,
                undo: Mutex::new(UndoStack::default()),
                project,
                tag_id,
                settings: plain_settings(Continuity::Off),
            }
        }

        fn service(&self) -> Recordings<'_> {
            Recordings {
                db: &self.db,
                undo: &self.undo,
                snap_minutes: 5,
            }
        }

        async fn app_rule(&self, app: &str) -> Rule {
            let input = RuleInput {
                note: format!("{app} work"),
                tag_id: Some(self.tag_id.clone()),
                ..rule_input(app, &self.project.id, &[(App, RuleOp::Is, app)])
            };
            store::save(&self.db, None, &input).await.unwrap()
        }

        /// Like `app_rule`, but every such rule shares one note, so touching suggestions of two apps join.
        async fn shared_rule(&self, app: &str) -> Rule {
            let input = RuleInput {
                note: "shared".into(),
                tag_id: Some(self.tag_id.clone()),
                ..rule_input(app, &self.project.id, &[(App, RuleOp::Is, app)])
            };
            store::save(&self.db, None, &input).await.unwrap()
        }

        async fn work(&self, day: Day, app: &str, from: (i64, i64), to: (i64, i64)) {
            let (start, end) = (on(day, from.0, from.1), on(day, to.0, to.1));
            activity(&self.db, start, Some(end), app, "").await;
        }

        fn input_on(&self, day: Day, from: (i64, i64), to: (i64, i64)) -> RecordingInput {
            RecordingInput {
                project_id: self.project.id.clone(),
                spans: vec![Span {
                    start: on(day, from.0, from.1),
                    end: on(day, to.0, to.1),
                }],
                note: String::new(),
                tag_id: None,
            }
        }

        async fn suggestions(&self, day: Day) -> Vec<Suggestion> {
            suggestions_of(&self.db, day, &self.settings).await.unwrap()
        }

        async fn recordings(&self) -> Vec<Recording> {
            stored::list_for_range(&self.db, Ms::MIN, Ms::MAX)
                .await
                .unwrap()
        }
    }

    #[tokio::test]
    async fn accepting_a_suggestion_records_it_with_the_rules_project_note_and_tag() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let rule = w.app_rule("Slack").await;
        w.work(today, "Slack", (9, 0), (10, 0)).await;
        let suggestion = w.suggestions(today).await.remove(0);

        let change = approve(&w.service(), &w.settings, today, &suggestion.id)
            .await
            .unwrap();

        let recording = change.value;
        assert_eq!(
            (
                recording.project_id.as_str(),
                recording.start(),
                recording.end(),
                recording.note.as_str(),
                recording.tag_id.clone(),
            ),
            (
                w.project.id.as_str(),
                on(today, 9, 0),
                Some(on(today, 10, 0)),
                "Slack work",
                Some(w.tag_id.clone()),
            )
        );
        assert_eq!(recording.spans.len(), 1);
        assert_eq!(suggestion.rule_id, rule.id);
        assert_eq!(w.recordings().await, [recording]);
        assert!(change.days.contains(&today));
    }

    #[tokio::test]
    async fn an_accepted_suggestion_reports_the_length_of_its_recorded_span() {
        let w = World::new().await;
        let today = day("2026-01-05");
        w.app_rule("Slack").await;
        w.work(today, "Slack", (9, 0), (10, 15)).await;
        let id = w.suggestions(today).await.remove(0).id;

        let change = approve(&w.service(), &w.settings, today, &id)
            .await
            .unwrap();

        let accepted = commands::SuggestionApproved {
            duration_ms: change.value.duration(on(today, 23, 0)),
            recording: change.value,
        };
        let wire = serde_json::to_value(&accepted).unwrap();
        assert_eq!(wire["durationMs"], 75 * MIN);
        assert_eq!(wire["recording"]["spans"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn an_accepted_suggestion_is_gone_and_a_stale_id_is_not_found() {
        let w = World::new().await;
        let today = day("2026-01-05");
        w.app_rule("Slack").await;
        w.work(today, "Slack", (9, 0), (10, 0)).await;
        let id = w.suggestions(today).await.remove(0).id;
        approve(&w.service(), &w.settings, today, &id)
            .await
            .unwrap();

        let again = approve(&w.service(), &w.settings, today, &id)
            .await
            .unwrap_err();
        let unknown = approve(&w.service(), &w.settings, today, "nope")
            .await
            .unwrap_err();

        assert!(w.suggestions(today).await.is_empty());
        assert_eq!(again.kind(), ErrorKind::NotFound);
        assert_eq!(unknown.kind(), ErrorKind::NotFound);
        assert_eq!(w.recordings().await.len(), 1);
    }

    #[tokio::test]
    async fn an_accepted_suggestion_is_undone_with_one_undo() {
        let w = World::new().await;
        let today = day("2026-01-05");
        w.app_rule("Slack").await;
        w.work(today, "Slack", (9, 0), (10, 0)).await;
        let id = w.suggestions(today).await.remove(0).id;
        approve(&w.service(), &w.settings, today, &id)
            .await
            .unwrap();

        w.service().undo().await.unwrap();

        assert!(w.recordings().await.is_empty());
        assert_eq!(w.suggestions(today).await.len(), 1);
    }

    #[tokio::test]
    async fn accepting_a_rule_records_all_its_suggestions_of_the_day_in_one_undo_step() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let slack = w.app_rule("Slack").await;
        let code = w.app_rule("Code").await;
        w.work(today, "Slack", (9, 0), (10, 0)).await;
        w.work(today, "Code", (11, 0), (12, 0)).await;
        w.work(today, "Slack", (13, 0), (14, 0)).await;

        let change = approve_rule(&w.service(), &w.settings, today, &slack.id)
            .await
            .unwrap();

        let spans: Vec<Vec<(Ms, Option<Ms>)>> = change
            .value
            .iter()
            .map(|r| r.spans.iter().map(|s| (s.start, s.end)).collect())
            .collect();
        assert_eq!(
            spans,
            [
                vec![(on(today, 9, 0), Some(on(today, 10, 0)))],
                vec![(on(today, 13, 0), Some(on(today, 14, 0)))],
            ]
        );
        assert!(change.value.iter().all(|r| r.note == "Slack work"));
        let left: Vec<String> = w
            .suggestions(today)
            .await
            .into_iter()
            .map(|s| s.rule_id)
            .collect();
        assert_eq!(left, [code.id]);

        let undone = w.service().undo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Approve 2 suggestions"));
        assert!(w.recordings().await.is_empty());
        assert_eq!(w.suggestions(today).await.len(), 3);
    }

    #[tokio::test]
    async fn accepting_a_rule_without_suggestions_is_not_found() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let slack = w.app_rule("Slack").await;

        let none = approve_rule(&w.service(), &w.settings, today, &slack.id)
            .await
            .unwrap_err();
        let unknown = approve_rule(&w.service(), &w.settings, today, "nope")
            .await
            .unwrap_err();

        assert_eq!(none.kind(), ErrorKind::NotFound);
        assert_eq!(unknown.kind(), ErrorKind::NotFound);
        assert!(w.recordings().await.is_empty());
    }

    /// Slack at 9, Code at 11 and Slack at 13, an hour each, with a rule for each app.
    async fn three_suggestions(w: &World, today: Day) -> Vec<Suggestion> {
        w.app_rule("Slack").await;
        w.app_rule("Code").await;
        w.work(today, "Slack", (9, 0), (10, 0)).await;
        w.work(today, "Code", (11, 0), (12, 0)).await;
        w.work(today, "Slack", (13, 0), (14, 0)).await;
        w.suggestions(today).await
    }

    fn ids_of(suggestions: &[Suggestion]) -> Vec<String> {
        suggestions.iter().map(|s| s.id.clone()).collect()
    }

    #[tokio::test]
    async fn approving_listed_suggestions_records_only_those_in_one_undo_step() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let all = three_suggestions(&w, today).await;
        let ids = vec![all[0].id.clone(), "gone".to_string(), all[2].id.clone()];

        let change = approve_all(&w.service(), &w.settings, today, &ids)
            .await
            .unwrap();

        let times: Vec<(Ms, Option<Ms>)> =
            change.value.iter().map(|r| (r.start(), r.end())).collect();
        assert_eq!(
            times,
            [
                (on(today, 9, 0), Some(on(today, 10, 0))),
                (on(today, 13, 0), Some(on(today, 14, 0))),
            ]
        );
        assert_eq!(w.recordings().await, change.value);
        assert_eq!(ids_of(&w.suggestions(today).await), [all[1].id.clone()]);

        let undone = w.service().undo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Approve 2 suggestions"));
        assert!(w.recordings().await.is_empty());
        assert_eq!(w.suggestions(today).await, all);
        assert_eq!(w.service().undo().await.unwrap().value, None);

        w.service().redo().await.unwrap();

        assert_eq!(w.recordings().await, change.value);
    }

    #[tokio::test]
    async fn approving_one_suggestion_of_a_list_is_labelled_in_the_singular() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let all = three_suggestions(&w, today).await;

        approve_all(&w.service(), &w.settings, today, &[all[1].id.clone()])
            .await
            .unwrap();

        let undone = w.service().undo().await.unwrap();
        assert_eq!(undone.value.as_deref(), Some("Approve suggestion"));
    }

    #[tokio::test]
    async fn approving_only_unknown_suggestions_is_not_found_and_records_nothing() {
        let w = World::new().await;
        let today = day("2026-01-05");
        three_suggestions(&w, today).await;

        let err = approve_all(&w.service(), &w.settings, today, &["gone".into()])
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(w.recordings().await.is_empty());
        assert_eq!(w.service().undo().await.unwrap().value, None);
    }

    #[tokio::test]
    async fn approving_an_edited_suggestion_records_the_edit_and_does_not_suggest_the_rest() {
        let w = World::new().await;
        let today = day("2026-01-05");
        w.app_rule("Slack").await;
        w.work(today, "Slack", (9, 0), (10, 0)).await;
        let suggestion = w.suggestions(today).await.remove(0);
        let edited = RecordingInput {
            note: "edited".into(),
            tag_id: None,
            ..w.input_on(today, (9, 0), (9, 30))
        };

        let change = approve_edited(
            &w.service(),
            &w.settings,
            today,
            &suggestion.id,
            edited,
            Overlap::Cancel,
        )
        .await
        .unwrap();

        let recording = change.value;
        assert_eq!(
            (
                recording.note.as_str(),
                recording.tag_id.clone(),
                recording.start(),
                recording.end()
            ),
            ("edited", None, on(today, 9, 0), Some(on(today, 9, 30)))
        );
        assert!(change.days.contains(&today));
        assert_eq!(w.recordings().await, [recording.clone()]);
        assert!(w.suggestions(today).await.is_empty());

        let undone = w.service().undo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Record 30m for Portal"));
        assert!(w.recordings().await.is_empty());
        assert_eq!(w.suggestions(today).await, [suggestion]);
        assert_eq!(w.service().undo().await.unwrap().value, None);

        w.service().redo().await.unwrap();

        assert_eq!(w.recordings().await, [recording]);
        assert!(w.suggestions(today).await.is_empty());
    }

    #[tokio::test]
    async fn approving_an_edited_suggestion_that_is_gone_is_not_found() {
        let w = World::new().await;
        let today = day("2026-01-05");

        let err = approve_edited(
            &w.service(),
            &w.settings,
            today,
            "gone",
            w.input_on(today, (9, 0), (9, 30)),
            Overlap::Cancel,
        )
        .await
        .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(w.recordings().await.is_empty());
    }

    #[tokio::test]
    async fn dismissing_returns_how_many_and_skips_unknown_ids() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let all = three_suggestions(&w, today).await;
        let ids = vec![all[0].id.clone(), "gone".to_string(), all[2].id.clone()];

        let change = dismiss(&w.service(), &w.settings, today, &ids)
            .await
            .unwrap();

        assert_eq!(change.value, 2);
        assert_eq!(change.days, [today]);
        assert_eq!(ids_of(&w.suggestions(today).await), [all[1].id.clone()]);
        assert!(w.recordings().await.is_empty());
    }

    #[tokio::test]
    async fn dismissing_only_unknown_suggestions_is_not_found_and_not_undoable() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let all = three_suggestions(&w, today).await;

        let err = dismiss(&w.service(), &w.settings, today, &["gone".into()])
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(w.suggestions(today).await, all);
        assert_eq!(w.service().undo().await.unwrap().value, None);
    }

    #[tokio::test]
    async fn undoing_a_dismissal_brings_the_suggestions_back_and_redoing_removes_them_again() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let all = three_suggestions(&w, today).await;
        let ids = vec![all[0].id.clone(), all[2].id.clone()];
        dismiss(&w.service(), &w.settings, today, &ids)
            .await
            .unwrap();

        let undone = w.service().undo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Dismiss 2 suggestions"));
        assert_eq!(undone.days, [today]);
        assert_eq!(w.suggestions(today).await, all);

        let redone = w.service().redo().await.unwrap();

        assert_eq!(redone.value.as_deref(), Some("Dismiss 2 suggestions"));
        assert_eq!(redone.days, [today]);
        assert_eq!(ids_of(&w.suggestions(today).await), [all[1].id.clone()]);
    }

    #[tokio::test]
    async fn dismissing_one_suggestion_is_labelled_in_the_singular() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let all = three_suggestions(&w, today).await;
        dismiss(&w.service(), &w.settings, today, &[all[0].id.clone()])
            .await
            .unwrap();

        let undone = w.service().undo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Dismiss suggestion"));
        assert_eq!(w.suggestions(today).await, all);
    }

    #[tokio::test]
    async fn a_dismissed_suggestion_stays_gone_after_the_rule_is_edited() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let rule = w.app_rule("Slack").await;
        w.work(today, "Slack", (9, 0), (10, 0)).await;
        let id = w.suggestions(today).await.remove(0).id;
        dismiss(&w.service(), &w.settings, today, &[id])
            .await
            .unwrap();
        assert!(w.suggestions(today).await.is_empty());

        let edited = RuleInput {
            note: "changed".into(),
            tag_id: None,
            ..rule_input(
                "Slack renamed",
                &w.project.id,
                &[(App, RuleOp::Is, "Slack")],
            )
        };
        store::save(&w.db, Some(&rule.id), &edited).await.unwrap();

        assert!(w.suggestions(today).await.is_empty());
    }

    #[tokio::test]
    async fn a_dismissed_suggestion_stays_gone_after_a_recording_over_part_of_it_is_deleted() {
        let w = World::new().await;
        let today = day("2026-01-05");
        w.app_rule("Slack").await;
        w.work(today, "Slack", (9, 0), (10, 0)).await;
        let id = w.suggestions(today).await.remove(0).id;
        dismiss(&w.service(), &w.settings, today, &[id])
            .await
            .unwrap();
        let created = w
            .service()
            .create(w.input_on(today, (9, 20), (9, 40)), Overlap::Cancel)
            .await
            .unwrap()
            .value;
        assert!(w.suggestions(today).await.is_empty());

        w.service().delete(&created.id).await.unwrap();

        assert!(w.suggestions(today).await.is_empty());
    }

    #[tokio::test]
    async fn activity_added_after_a_dismissed_span_is_still_suggested() {
        let w = World::new().await;
        let today = day("2026-01-05");
        w.app_rule("Slack").await;
        w.work(today, "Slack", (9, 0), (10, 0)).await;
        let id = w.suggestions(today).await.remove(0).id;
        dismiss(&w.service(), &w.settings, today, &[id])
            .await
            .unwrap();

        w.work(today, "Slack", (10, 0), (11, 0)).await;

        let spans: Vec<(Ms, Ms)> = w
            .suggestions(today)
            .await
            .iter()
            .map(|s| (s.start, s.end))
            .collect();
        assert_eq!(spans, [(on(today, 10, 0), on(today, 11, 0))]);
    }

    #[tokio::test]
    async fn a_suggestion_points_at_the_first_card_that_starts_after_it() {
        let w = World::new().await;
        let today = day("2026-01-05");
        w.app_rule("Slack").await;
        w.work(today, "Slack", (9, 0), (10, 0)).await;
        w.work(today, "Slack", (13, 0), (14, 0)).await;
        w.service()
            .create(w.input_on(today, (8, 0), (8, 30)), Overlap::Cancel)
            .await
            .unwrap();
        let later = w
            .service()
            .create(w.input_on(today, (11, 0), (12, 0)), Overlap::Cancel)
            .await
            .unwrap()
            .value;

        let next_cards: Vec<Option<String>> = w
            .suggestions(today)
            .await
            .into_iter()
            .map(|s| s.next_card)
            .collect();

        assert_eq!(next_cards, [Some(later.id), None]);
    }

    #[tokio::test]
    async fn a_suggestion_carries_the_clients_name_and_the_tags_name() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let client = clients::insert(
            &w.db,
            ClientInput {
                name: "Initech".into(),
                color: String::new(),
            },
        )
        .await
        .unwrap();
        projects::update(
            &w.db,
            &w.project.id,
            ProjectInput {
                client_id: Some(client.id),
                name: w.project.name.clone(),
                color: w.project.color.clone(),
                shade: None,
                billable: false,
            },
        )
        .await
        .unwrap();
        tags::set_emoji(&w.db, &w.tag_id, Some("🧪")).await.unwrap();
        w.app_rule("Slack").await;
        w.work(today, "Slack", (9, 0), (10, 0)).await;

        let suggestion = w.suggestions(today).await.remove(0);

        assert_eq!(
            (
                suggestion.client_name.as_deref(),
                suggestion.tag_id.as_ref(),
                suggestion.tag.as_deref(),
                suggestion.tag_emoji.as_deref()
            ),
            (
                Some("Initech"),
                Some(&w.tag_id),
                Some("billing"),
                Some("🧪")
            )
        );
    }

    #[tokio::test]
    async fn suggestions_bridge_a_short_idle_gap_but_not_a_lock_or_sleep() {
        use crate::db::away::{self, AwayReason, NewAway};

        let cases = [
            ("idle", Some(AwayReason::Idle), 1),
            ("lock", Some(AwayReason::Lock), 2),
            ("sleep", Some(AwayReason::Sleep), 2),
            ("nothing", None, 1),
        ];
        for (name, reason, count) in cases {
            let mut w = World::new().await;
            w.settings = plain_settings(Continuity::Regular);
            let today = day("2026-01-05");
            w.app_rule("Slack").await;
            w.work(today, "Slack", (9, 0), (9, 10)).await;
            w.work(today, "Slack", (9, 15), (9, 30)).await;
            if let Some(reason) = reason {
                let start = on(today, 9, 11);
                away::insert(&w.db, NewAway { start, reason })
                    .await
                    .unwrap();
                away::close_open(&w.db, on(today, 9, 14)).await.unwrap();
            }

            let suggestions = w.suggestions(today).await;

            assert_eq!(suggestions.len(), count, "{name}");
        }
    }

    #[tokio::test]
    async fn a_rules_day_counts_matched_time_under_first_match_and_its_pending_suggestions() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let pid = &w.project.id;
        let broad = save_rule(&w.db, "Broad", pid, &[(App, RuleOp::Contains, "Cod")]).await;
        let exact = save_rule(&w.db, "Exact", pid, &[(App, RuleOp::Is, "Code")]).await;
        let brief = save_rule(&w.db, "Brief", pid, &[(App, RuleOp::Is, "Slack")]).await;
        w.work(today, "Code", (11, 0), (12, 0)).await;
        w.work(today, "Slack", (9, 0), (9, 3)).await;

        let days = day_of_rules(&w.db, today, &w.settings).await.unwrap();

        assert_eq!(
            days,
            [
                RuleDay {
                    rule_id: broad.id.clone(),
                    matched_ms: 60 * MIN,
                    pending: 1,
                    pending_ms: 60 * MIN,
                },
                RuleDay {
                    rule_id: exact.id,
                    matched_ms: 0,
                    pending: 0,
                    pending_ms: 0,
                },
                RuleDay {
                    rule_id: brief.id,
                    matched_ms: 3 * MIN,
                    pending: 0,
                    pending_ms: 0,
                },
            ]
        );

        let id = w.suggestions(today).await.remove(0).id;
        dismiss(&w.service(), &w.settings, today, &[id])
            .await
            .unwrap();

        let after = day_of_rules(&w.db, today, &w.settings).await.unwrap();

        assert_eq!(
            after[0],
            RuleDay {
                rule_id: broad.id,
                matched_ms: 60 * MIN,
                pending: 0,
                pending_ms: 0,
            }
        );
    }

    /// Zoom 10:00 to 10:13 with a two-minute look at Slack inside it, under a rule for each app.
    async fn zoom_with_a_slack_glance(w: &World, today: Day) -> (Rule, Rule) {
        let zoom = w.app_rule("Zoom").await;
        let slack = w.app_rule("Slack").await;
        w.work(today, "Zoom", (10, 0), (10, 6)).await;
        w.work(today, "Slack", (10, 6), (10, 8)).await;
        w.work(today, "Zoom", (10, 8), (10, 13)).await;
        (zoom, slack)
    }

    #[tokio::test]
    async fn a_short_glance_at_another_app_stays_part_of_the_meeting_suggestion() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let (zoom, _) = zoom_with_a_slack_glance(&w, today).await;

        let found: Vec<(String, Ms, Ms)> = w
            .suggestions(today)
            .await
            .into_iter()
            .map(|s| (s.rule_id, s.start, s.end))
            .collect();

        assert_eq!(found, [(zoom.id, on(today, 10, 0), on(today, 10, 13))]);
    }

    #[tokio::test]
    async fn a_rules_day_counts_only_the_time_its_app_won_in_the_summary() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let (zoom, slack) = zoom_with_a_slack_glance(&w, today).await;

        let days = day_of_rules(&w.db, today, &w.settings).await.unwrap();

        let matched: Vec<(String, Ms)> = days
            .into_iter()
            .map(|d| (d.rule_id, d.matched_ms))
            .collect();
        assert_eq!(matched, [(zoom.id, 13 * MIN), (slack.id, 0)]);
    }

    #[tokio::test]
    async fn the_preview_sees_the_summary_so_a_glance_has_no_hit() {
        let w = World::new().await;
        let today = day("2026-01-05");
        zoom_with_a_slack_glance(&w, today).await;
        let input = rule_input("Slack", &w.project.id, &[(App, RuleOp::Is, "Slack")]);

        let preview = preview(&w.db, &w.settings, &input, today, 1).await.unwrap();

        assert_eq!(
            (preview.hits.len(), preview.total_ms, preview.days),
            (0, 0, 0)
        );
    }

    /// Slack 9 to 10 and Code 10 to 11 under two rules with one target: a single joined suggestion.
    async fn joined_suggestion(w: &World, today: Day) -> (Rule, Rule, Suggestion) {
        let slack = w.shared_rule("Slack").await;
        let code = w.shared_rule("Code").await;
        w.work(today, "Slack", (9, 0), (10, 0)).await;
        w.work(today, "Code", (10, 0), (11, 0)).await;
        let mut found = w.suggestions(today).await;
        assert_eq!(found.len(), 1);
        (slack, code, found.remove(0))
    }

    #[tokio::test]
    async fn touching_work_of_two_rules_with_one_target_is_one_joined_suggestion() {
        let w = World::new().await;
        let today = day("2026-01-05");

        let (slack, code, joined) = joined_suggestion(&w, today).await;

        assert_eq!(joined.rule_ids, [slack.id.clone(), code.id]);
        assert_eq!(joined.rule_id, slack.id);
        assert_eq!(joined.rule_name, "Slack, Code");
        assert_eq!(
            (joined.start, joined.end),
            (on(today, 9, 0), on(today, 11, 0))
        );
    }

    #[tokio::test]
    async fn dismissing_a_joined_suggestion_dismisses_it_for_each_of_its_rules() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let (_, _, joined) = joined_suggestion(&w, today).await;

        let change = dismiss(&w.service(), &w.settings, today, &[joined.id.clone()])
            .await
            .unwrap();

        assert_eq!(change.value, 1);
        assert!(w.suggestions(today).await.is_empty());
        w.work(today, "Slack", (11, 0), (12, 0)).await;
        let spans: Vec<(Ms, Ms)> = w
            .suggestions(today)
            .await
            .iter()
            .map(|s| (s.start, s.end))
            .collect();
        assert_eq!(spans, [(on(today, 11, 0), on(today, 12, 0))]);
    }

    #[tokio::test]
    async fn undoing_the_dismissal_of_a_joined_suggestion_brings_it_back_whole() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let (_, _, joined) = joined_suggestion(&w, today).await;
        dismiss(&w.service(), &w.settings, today, &[joined.id.clone()])
            .await
            .unwrap();

        let undone = w.service().undo().await.unwrap();

        assert_eq!(undone.value.as_deref(), Some("Dismiss suggestion"));
        assert_eq!(w.suggestions(today).await, [joined]);
    }

    #[tokio::test]
    async fn approving_by_the_second_rule_records_the_joined_suggestion_it_shares() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let (_, code, _) = joined_suggestion(&w, today).await;

        let change = approve_rule(&w.service(), &w.settings, today, &code.id)
            .await
            .unwrap();

        let times: Vec<(Ms, Option<Ms>)> =
            change.value.iter().map(|r| (r.start(), r.end())).collect();
        assert_eq!(times, [(on(today, 9, 0), Some(on(today, 11, 0)))]);
        assert_eq!(change.value[0].note, "shared");
        assert!(w.suggestions(today).await.is_empty());
    }

    #[tokio::test]
    async fn approving_an_edited_joined_suggestion_leaves_nothing_of_it_for_either_rule() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let (_, _, joined) = joined_suggestion(&w, today).await;

        approve_edited(
            &w.service(),
            &w.settings,
            today,
            &joined.id,
            w.input_on(today, (9, 0), (9, 30)),
            Overlap::Cancel,
        )
        .await
        .unwrap();

        assert!(w.suggestions(today).await.is_empty());
        assert_eq!(w.recordings().await.len(), 1);
    }

    #[tokio::test]
    async fn a_joined_suggestion_is_pending_for_each_of_its_rules() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let (slack, code, _) = joined_suggestion(&w, today).await;

        let days = day_of_rules(&w.db, today, &w.settings).await.unwrap();

        assert_eq!(
            days,
            [
                RuleDay {
                    rule_id: slack.id,
                    matched_ms: 60 * MIN,
                    pending: 1,
                    pending_ms: 120 * MIN,
                },
                RuleDay {
                    rule_id: code.id,
                    matched_ms: 60 * MIN,
                    pending: 1,
                    pending_ms: 120 * MIN,
                },
            ]
        );
    }

    #[tokio::test]
    async fn the_preview_lists_the_last_days_most_recent_first_with_the_summed_time() {
        let w = World::new().await;
        let (d7, d5, d1) = (day("2026-01-07"), day("2026-01-05"), day("2026-01-01"));
        w.work(d7, "Slack", (9, 0), (10, 0)).await;
        w.work(d7, "Code", (10, 0), (11, 0)).await;
        w.work(d7, "Slack", (14, 0), (14, 30)).await;
        w.work(d5, "Slack", (9, 0), (9, 45)).await;
        w.work(d1, "Slack", (8, 0), (9, 0)).await;
        w.work(day("2025-12-31"), "Slack", (9, 0), (10, 0)).await;
        let input = rule_input("Slack", &w.project.id, &[(App, RuleOp::Is, "Slack")]);

        let preview = preview(&w.db, &w.settings, &input, d7, 7).await.unwrap();

        let hits: Vec<(Day, Ms)> = preview
            .hits
            .iter()
            .map(|h| (h.day, h.block.start))
            .collect();
        assert_eq!(
            hits,
            [
                (d7, on(d7, 14, 0)),
                (d7, on(d7, 9, 0)),
                (d5, on(d5, 9, 0)),
                (d1, on(d1, 8, 0)),
            ]
        );
        let labels: Vec<&str> = preview.hits.iter().map(|h| h.day_label.as_str()).collect();
        assert_eq!(
            labels,
            ["Wed, Jan 7", "Wed, Jan 7", "Mon, Jan 5", "Thu, Jan 1"]
        );
        assert_eq!(preview.total_ms, (60 + 30 + 45 + 60) * MIN);
        assert_eq!(preview.days, 3);
    }

    #[tokio::test]
    async fn the_preview_lists_at_most_two_hundred_hits_but_counts_all_the_time() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let midnight = on(today, 0, 0);
        for i in 0..250 {
            let start = midnight + i * 5 * MIN;
            let title = format!("window {i}");
            activity(&w.db, start, Some(start + MIN), "Slack", &title).await;
        }
        let input = rule_input("Slack", &w.project.id, &[(App, RuleOp::Is, "Slack")]);

        let preview = preview(&w.db, &w.settings, &input, today, 7).await.unwrap();

        assert_eq!(preview.hits.len(), PREVIEW_HITS);
        assert_eq!(preview.hits[0].block.start, midnight + 249 * 5 * MIN);
        assert_eq!(preview.hits[199].block.start, midnight + 50 * 5 * MIN);
        assert_eq!((preview.total_ms, preview.days), (250 * MIN, 1));
    }

    #[tokio::test]
    async fn the_preview_of_a_rule_without_conditions_is_refused() {
        let w = World::new().await;
        let input = rule_input("Nothing", &w.project.id, &[]);

        let err = preview(&w.db, &w.settings, &input, day("2026-01-05"), 7)
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(field_of(&err).as_deref(), Some("conditions"));
    }

    #[tokio::test]
    async fn the_preview_without_matches_is_empty() {
        let w = World::new().await;
        let today = day("2026-01-05");
        w.work(today, "Code", (9, 0), (10, 0)).await;
        let input = rule_input("Slack", &w.project.id, &[(App, RuleOp::Is, "Slack")]);

        let preview = preview(&w.db, &w.settings, &input, today, 7).await.unwrap();

        assert_eq!(
            (preview.hits.len(), preview.total_ms, preview.days),
            (0, 0, 0)
        );
    }
}
