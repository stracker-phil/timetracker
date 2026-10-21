//! Rules ("activity like this belongs to project X") and the suggested recordings they produce. Rules never
//! write recordings; accepting a suggestion is an ordinary, undoable create.

pub mod commands;
pub mod matching;
pub mod store;
pub mod suggest;
#[cfg(test)]
pub(crate) mod testing;
pub mod types;

use crate::day::transform::{breaks_continuity, continuity_ms};
use crate::day::{ActivityBlock, DayView, LaneKind, Names};
use crate::db::Db;
use crate::db::recordings::Recording;
use crate::db::time::{Day, Ms, now_ms};
use crate::error::{AppError, AppResult};
use crate::recordings::{Change, Overlap, RecordingInput, Recordings};
use crate::store::settings::Settings;
use matching::{Compiled, matches};
use types::{PreviewHit, RuleInput, RulePreview, Suggestion};

/// Keeps the preview's reply small; its total still counts every hit.
pub const PREVIEW_HITS: usize = 200;

/// The blocks rules look at: every lane but the summary.
fn candidates(view: &DayView) -> Vec<&ActivityBlock> {
    let mut blocks: Vec<&ActivityBlock> = view
        .lanes
        .iter()
        .filter(|lane| lane.kind != LaneKind::Summary)
        .flat_map(|lane| &lane.blocks)
        .collect();
    blocks.sort_by_key(|b| (b.start, b.end));
    blocks
}

/// The view's suggestions under `rules` (compiled, in order).
pub fn day_suggestions(
    view: &DayView,
    rules: &[Compiled],
    names: &Names,
    settings: &Settings,
) -> Vec<Suggestion> {
    if rules.is_empty() {
        return Vec::new();
    }
    let barriers: Vec<(Ms, Ms)> = view
        .away
        .iter()
        .filter(|a| breaks_continuity(a.reason))
        .map(|a| (a.start, a.end))
        .collect();
    suggest::suggest(
        &candidates(view),
        &view.recordings,
        &barriers,
        rules,
        names,
        continuity_ms(settings.timeline.continuity),
    )
}

pub async fn suggestions_of(db: &Db, day: Day, settings: &Settings) -> AppResult<Vec<Suggestion>> {
    Ok(crate::day::commands::load(db, day, settings, now_ms())
        .await?
        .suggestions)
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
        let view = crate::day::commands::view_of(db, day, settings, now).await?;
        let mut found: Vec<&ActivityBlock> = candidates(&view)
            .into_iter()
            .filter(|b| matches(&rule, b))
            .collect();
        if !found.is_empty() {
            hit_days += 1;
        }
        total_ms += found.iter().map(|b| b.end - b.start).sum::<Ms>();
        found.reverse();
        hits.extend(found.into_iter().map(|block| PreviewHit {
            block: block.clone(),
            day,
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

fn input_of(suggestion: Suggestion) -> RecordingInput {
    RecordingInput {
        project_id: suggestion.project_id,
        start: suggestion.start,
        end: suggestion.end,
        note: suggestion.note,
        tag_id: suggestion.tag_id,
    }
}

/// Records the suggestion `id` of `day` (`not_found` once it is gone); refused with `overlap` when a
/// recording took part of its time.
pub async fn accept(
    service: &Recordings<'_>,
    settings: &Settings,
    day: Day,
    id: &str,
) -> AppResult<Change<Recording>> {
    let suggestion = suggestions_of(service.db, day, settings)
        .await?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| AppError::NotFound("The suggestion is gone".into()))?;
    service.create(input_of(suggestion), Overlap::Cancel).await
}

/// Records every suggestion of the rule on `day` as one undoable change.
pub async fn accept_rule(
    service: &Recordings<'_>,
    settings: &Settings,
    day: Day,
    rule_id: &str,
) -> AppResult<Change<Vec<Recording>>> {
    let found: Vec<Suggestion> = suggestions_of(service.db, day, settings)
        .await?
        .into_iter()
        .filter(|s| s.rule_id == rule_id)
        .collect();
    let Some(first) = found.first() else {
        return Err(AppError::NotFound(
            "The rule has no suggestions on this day".into(),
        ));
    };
    let label = format!("Accept suggestions of {}", first.rule_name);
    service
        .create_all(found.into_iter().map(input_of).collect(), label)
        .await
}

#[cfg(test)]
mod tests {
    use tokio::sync::Mutex;

    use super::testing::*;
    use super::types::RuleField::*;
    use super::types::{Rule, RuleOp};
    use super::*;
    use crate::db::projects::Project;
    use crate::db::recordings as stored;
    use crate::db::tags;
    use crate::error::ErrorKind;
    use crate::recordings::undo::UndoStack;
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

        async fn work(&self, day: Day, app: &str, from: (i64, i64), to: (i64, i64)) {
            let (start, end) = (on(day, from.0, from.1), on(day, to.0, to.1));
            activity(&self.db, start, Some(end), app, "").await;
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

        let change = accept(&w.service(), &w.settings, today, &suggestion.id)
            .await
            .unwrap();

        let recording = change.value;
        assert_eq!(
            (
                recording.project_id.as_str(),
                recording.start,
                recording.end,
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
        assert_eq!(suggestion.rule_id, rule.id);
        assert_eq!(w.recordings().await, [recording]);
        assert!(change.days.contains(&today));
    }

    #[tokio::test]
    async fn an_accepted_suggestion_is_gone_and_a_stale_id_is_not_found() {
        let w = World::new().await;
        let today = day("2026-01-05");
        w.app_rule("Slack").await;
        w.work(today, "Slack", (9, 0), (10, 0)).await;
        let id = w.suggestions(today).await.remove(0).id;
        accept(&w.service(), &w.settings, today, &id).await.unwrap();

        let again = accept(&w.service(), &w.settings, today, &id)
            .await
            .unwrap_err();
        let unknown = accept(&w.service(), &w.settings, today, "nope")
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
        accept(&w.service(), &w.settings, today, &id).await.unwrap();

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

        let change = accept_rule(&w.service(), &w.settings, today, &slack.id)
            .await
            .unwrap();

        let spans: Vec<(Ms, Option<Ms>)> = change.value.iter().map(|r| (r.start, r.end)).collect();
        assert_eq!(
            spans,
            [
                (on(today, 9, 0), Some(on(today, 10, 0))),
                (on(today, 13, 0), Some(on(today, 14, 0))),
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

        assert_eq!(undone.value.as_deref(), Some("Accept suggestions of Slack"));
        assert!(w.recordings().await.is_empty());
        assert_eq!(w.suggestions(today).await.len(), 3);
    }

    #[tokio::test]
    async fn accepting_a_rule_without_suggestions_is_not_found() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let slack = w.app_rule("Slack").await;

        let none = accept_rule(&w.service(), &w.settings, today, &slack.id)
            .await
            .unwrap_err();
        let unknown = accept_rule(&w.service(), &w.settings, today, "nope")
            .await
            .unwrap_err();

        assert_eq!(none.kind(), ErrorKind::NotFound);
        assert_eq!(unknown.kind(), ErrorKind::NotFound);
        assert!(w.recordings().await.is_empty());
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
        assert_eq!(preview.total_ms, (60 + 30 + 45 + 60) * MIN);
        assert_eq!(preview.days, 3);
    }

    #[tokio::test]
    async fn the_preview_lists_at_most_two_hundred_hits_but_counts_all_the_time() {
        let w = World::new().await;
        let today = day("2026-01-05");
        let midnight = on(today, 0, 0);
        for i in 0..250 {
            let start = midnight + i * 2 * MIN;
            let title = format!("window {i}");
            activity(&w.db, start, Some(start + MIN), "Slack", &title).await;
        }
        let input = rule_input("Slack", &w.project.id, &[(App, RuleOp::Is, "Slack")]);

        let preview = preview(&w.db, &w.settings, &input, today, 7).await.unwrap();

        assert_eq!(preview.hits.len(), PREVIEW_HITS);
        assert_eq!(preview.hits[0].block.start, midnight + 249 * 2 * MIN);
        assert_eq!(preview.hits[199].block.start, midnight + 50 * 2 * MIN);
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
