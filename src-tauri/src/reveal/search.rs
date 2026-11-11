//! The palette's `r:` rows: the recordings that match the typed text, and the suggestions of the last days.

use std::collections::HashSet;

use chrono::Days;
use tauri::State;

use super::{EntryKind, EntryRef, FoundEntry};
use crate::calendar::titles::day_title;
use crate::day::{self, Names};
use crate::db::Db;
use crate::db::recordings::{self, Recording};
use crate::db::time::Day;
use crate::error::AppResult;
use crate::rules::stored::{NO_PROJECT_COLOR, NO_PROJECT_SHADE};
use crate::rules::{self, types::Suggestion};
use crate::state::AppState;
use crate::store::settings::{DayTitle, Settings};

/// The most recordings one search returns.
const RECORDING_LIMIT: u32 = 30;
/// Suggestions are rebuilt per day, so the palette offers those of today and the days before it only.
const SUGGESTION_DAYS: u64 = 7;

fn recording_row(recording: &Recording, names: &Names, today: Day) -> FoundEntry {
    let day = Day::of(recording.start());
    let (project, client, color, shade) = match names.projects.get(&recording.project_id) {
        Some(p) => (
            p.name.clone(),
            p.client_name.clone(),
            p.color.clone(),
            p.shade,
        ),
        None => (
            recording.project_id.clone(),
            None,
            NO_PROJECT_COLOR.into(),
            NO_PROJECT_SHADE,
        ),
    };
    FoundEntry {
        entry: EntryRef {
            day,
            kind: EntryKind::Recording,
            id: recording.id.clone(),
        },
        day_label: day_title(day, DayTitle::Short, today),
        start: recording.start(),
        end: recording.end(),
        project: Some(project),
        client,
        color,
        shade,
        note: recording.note.clone(),
        tag: recording
            .tag_id
            .as_ref()
            .and_then(|id| names.tags.get(id))
            .map(|t| t.name.clone()),
        by: None,
    }
}

fn suggestion_row(suggestion: Suggestion, day: Day, day_label: String) -> FoundEntry {
    FoundEntry {
        entry: EntryRef {
            day,
            kind: EntryKind::Suggestion,
            id: suggestion.id,
        },
        day_label,
        start: suggestion.start,
        end: Some(suggestion.end),
        project: suggestion.project_name,
        client: suggestion.client_name,
        color: suggestion.color,
        shade: suggestion.shade,
        note: suggestion.note,
        tag: suggestion.tag,
        by: Some(suggestion.by),
    }
}

/// The recordings whose note, project, client or tag holds `text` (all for an empty one), latest first.
pub async fn recordings_matching(db: &Db, text: &str, today: Day) -> AppResult<Vec<FoundEntry>> {
    let names = day::names(db).await?;
    Ok(recordings::search(db, text.trim(), RECORDING_LIMIT)
        .await?
        .iter()
        .map(|r| recording_row(r, &names, today))
        .collect())
}

/// The palette's suggestions from `today` back over `SUGGESTION_DAYS` days, latest day first; a stored
/// suggestion once, as its first piece.
pub async fn recent_suggestions(
    db: &Db,
    settings: &Settings,
    today: Day,
) -> AppResult<Vec<FoundEntry>> {
    let mut rows = Vec::new();
    for back in 0..SUGGESTION_DAYS {
        let day = Day::from_date(today.date() - Days::new(back));
        let day_label = day_title(day, DayTitle::Short, today);
        let mut listed_stored = HashSet::new();
        for suggestion in rules::suggestions_of(db, day, settings).await? {
            if let Some(id) = &suggestion.stored_id
                && !listed_stored.insert(id.clone())
            {
                continue;
            }
            rows.push(suggestion_row(suggestion, day, day_label.clone()));
        }
    }
    Ok(rows)
}

#[tauri::command]
pub async fn reveal_recordings(
    state: State<'_, AppState>,
    text: String,
) -> AppResult<Vec<FoundEntry>> {
    recordings_matching(&state.data.db()?, &text, Day::today()).await
}

#[tauri::command]
pub async fn reveal_suggestions(state: State<'_, AppState>) -> AppResult<Vec<FoundEntry>> {
    recent_suggestions(&state.data.db()?, &state.settings.get(), Day::today()).await
}

#[cfg(test)]
mod tests {
    use tokio::sync::Mutex;

    use super::*;
    use crate::db::projects::{self, Project, ProjectInput};
    use crate::db::recordings::NewRecording;
    use crate::db::tags::{self, Tag};
    use crate::db::time::Ms;
    use crate::recordings::undo::UndoStack;
    use crate::recordings::{Recordings, Span, SuggestionInput};
    use crate::rules::testing::{on, plain_settings, save_rule};
    use crate::rules::types::{RuleField, RuleOp};
    use crate::store::settings::Continuity;
    use crate::test_support::{activity, client};

    fn day_of(text: &str) -> Day {
        Day::parse(text).unwrap()
    }

    fn today() -> Day {
        day_of("2026-10-23")
    }

    fn days_before(day: Day, back: u64) -> Day {
        Day::from_date(day.date() - Days::new(back))
    }

    struct World {
        db: Db,
        portal: Project,
        podcast: Project,
        billing: Tag,
        undo: Mutex<UndoStack>,
    }

    impl World {
        /// "Website relaunch" (client "Example Co") and "Podcast", and a `billing` tag.
        async fn new() -> Self {
            let db = Db::open_memory().await.unwrap();
            let example = client(&db, "Example Co").await;
            let portal = projects::insert(
                &db,
                ProjectInput {
                    client_id: Some(example.id),
                    name: "Website relaunch".into(),
                    color: "blue".into(),
                    shade: None,
                    billable: true,
                },
            )
            .await
            .unwrap();
            let podcast = crate::test_support::project(&db, "Podcast").await;
            let billing = tags::insert(&db, "billing").await.unwrap();
            Self {
                db,
                portal,
                podcast,
                billing,
                undo: Mutex::new(UndoStack::default()),
            }
        }

        async fn record(
            &self,
            of: &Project,
            spans: &[(Ms, Option<Ms>)],
            note: &str,
            tag: Option<&Tag>,
        ) -> Recording {
            recordings::insert(
                &self.db,
                NewRecording {
                    id: uuid::Uuid::new_v4().to_string(),
                    project_id: of.id.clone(),
                    spans: spans.to_vec(),
                    note: note.into(),
                    tag_id: tag.map(|t| t.id.clone()),
                },
            )
            .await
            .unwrap()
        }

        /// A stored suggestion for the portal project with one piece per `(from, to)` hour on `day`.
        async fn stored(&self, day: Day, hours: &[(i64, i64)], source: &str) -> String {
            let input = SuggestionInput {
                project_id: Some(self.portal.id.clone()),
                spans: hours
                    .iter()
                    .map(|&(from, to)| Span {
                        start: on(day, from, 0),
                        end: on(day, to, 0),
                    })
                    .collect(),
                note: "from planner".into(),
                tag_id: Some(self.billing.id.clone()),
            };
            Recordings {
                db: &self.db,
                undo: &self.undo,
                snap_minutes: 5,
            }
            .create_suggestion(input, source)
            .await
            .unwrap()
            .value
            .id
        }
    }

    fn found_ids(rows: &[FoundEntry]) -> Vec<&str> {
        rows.iter().map(|r| r.entry.id.as_str()).collect()
    }

    #[tokio::test]
    async fn a_recording_row_carries_its_place_project_client_colors_note_and_tag() {
        let w = World::new().await;
        let day = day_of("2026-10-21");
        let rec = w
            .record(
                &w.portal,
                &[(on(day, 9, 0), Some(on(day, 10, 0)))],
                "review the layout",
                Some(&w.billing),
            )
            .await;

        let rows = recordings_matching(&w.db, "layout", today()).await.unwrap();

        assert_eq!(
            rows,
            [FoundEntry {
                entry: EntryRef {
                    day,
                    kind: EntryKind::Recording,
                    id: rec.id,
                },
                day_label: day_title(day, DayTitle::Short, today()),
                start: on(day, 9, 0),
                end: Some(on(day, 10, 0)),
                project: Some("Website relaunch".into()),
                client: Some("Example Co".into()),
                color: w.portal.color.clone(),
                shade: w.portal.shade,
                note: "review the layout".into(),
                tag: Some("billing".into()),
                by: None,
            }]
        );
    }

    #[tokio::test]
    async fn a_running_recording_has_no_end_and_a_project_without_client_has_none() {
        let w = World::new().await;
        let day = day_of("2026-10-23");
        let rec = w
            .record(&w.podcast, &[(on(day, 11, 0), None)], "", None)
            .await;

        let rows = recordings_matching(&w.db, "podcast", today())
            .await
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].entry.id, rec.id);
        assert_eq!(rows[0].end, None);
        assert_eq!((rows[0].client.clone(), rows[0].tag.clone()), (None, None));
    }

    #[tokio::test]
    async fn a_recording_sits_on_the_day_of_its_first_span() {
        let w = World::new().await;
        let (first, second) = (day_of("2026-10-20"), day_of("2026-10-21"));
        let rec = w
            .record(
                &w.podcast,
                &[
                    (on(first, 23, 0), Some(on(first, 23, 30))),
                    (on(second, 9, 0), Some(on(second, 10, 0))),
                ],
                "",
                None,
            )
            .await;

        let rows = recordings_matching(&w.db, "", today()).await.unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].entry.id, rec.id);
        assert_eq!(rows[0].entry.day, first);
        assert_eq!(rows[0].start, on(first, 23, 0));
    }

    #[tokio::test]
    async fn the_typed_text_is_trimmed_and_matches_the_latest_recordings_first() {
        let w = World::new().await;
        let (monday, tuesday) = (day_of("2026-10-19"), day_of("2026-10-20"));
        let older = w
            .record(
                &w.portal,
                &[(on(monday, 9, 0), Some(on(monday, 10, 0)))],
                "",
                None,
            )
            .await;
        let newer = w
            .record(
                &w.portal,
                &[(on(tuesday, 9, 0), Some(on(tuesday, 10, 0)))],
                "",
                None,
            )
            .await;
        w.record(
            &w.podcast,
            &[(on(tuesday, 11, 0), Some(on(tuesday, 12, 0)))],
            "",
            None,
        )
        .await;

        let rows = recordings_matching(&w.db, "  example co ", today())
            .await
            .unwrap();

        assert_eq!(found_ids(&rows), [newer.id.as_str(), older.id.as_str()]);
        assert!(
            recordings_matching(&w.db, "invoice", today())
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_search_returns_at_most_thirty_recordings() {
        let w = World::new().await;
        let day = day_of("2026-10-20");
        for minute in 0..31 {
            let start = on(day, 8, 0) + minute * 60_000;
            w.record(&w.podcast, &[(start, Some(start + 30_000))], "", None)
                .await;
        }

        let rows = recordings_matching(&w.db, "", today()).await.unwrap();

        assert_eq!(rows.len(), 30);
        assert_eq!(rows[0].start, on(day, 8, 30));
    }

    #[tokio::test]
    async fn a_stored_suggestion_with_several_pieces_is_one_row_as_its_first_piece() {
        let w = World::new().await;
        let day = today();
        let stored = w.stored(day, &[(9, 10), (11, 12)], "planner").await;
        let settings = plain_settings(Continuity::Off);

        let rows = recent_suggestions(&w.db, &settings, today()).await.unwrap();

        assert_eq!(
            rows,
            [FoundEntry {
                entry: EntryRef {
                    day,
                    kind: EntryKind::Suggestion,
                    id: format!("stored:{stored}:{}:{}", on(day, 9, 0), on(day, 10, 0)),
                },
                day_label: day_title(day, DayTitle::Short, today()),
                start: on(day, 9, 0),
                end: Some(on(day, 10, 0)),
                project: Some("Website relaunch".into()),
                client: Some("Example Co".into()),
                color: w.portal.color.clone(),
                shade: w.portal.shade,
                note: "from planner".into(),
                tag: Some("billing".into()),
                by: Some("planner".into()),
            }]
        );
    }

    #[tokio::test]
    async fn a_rule_suggestion_is_named_by_its_rule() {
        let w = World::new().await;
        let day = today();
        let rule = save_rule(
            &w.db,
            "Slack rule",
            &w.portal.id,
            &[(RuleField::App, RuleOp::Is, "Slack")],
        )
        .await;
        activity(&w.db, on(day, 9, 0), Some(on(day, 10, 0)), "Slack", "").await;
        let settings = plain_settings(Continuity::Off);

        let rows = recent_suggestions(&w.db, &settings, today()).await.unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].entry,
            EntryRef {
                day,
                kind: EntryKind::Suggestion,
                id: format!("{}:{}:{}", rule.id, on(day, 9, 0), on(day, 10, 0)),
            }
        );
        assert_eq!(rows[0].by, Some("Slack rule".into()));
        assert_eq!(rows[0].project, Some("Website relaunch".into()));
    }

    #[tokio::test]
    async fn suggestions_of_the_last_seven_days_come_latest_day_first_and_older_ones_are_left_out()
    {
        let w = World::new().await;
        let today = today();
        let mut expected = Vec::new();
        for back in [6, 0, 3] {
            let id = w.stored(days_before(today, back), &[(9, 10)], "cli").await;
            expected.push((back, id));
        }
        w.stored(days_before(today, 7), &[(9, 10)], "cli").await;
        expected.sort();
        let settings = plain_settings(Continuity::Off);

        let rows = recent_suggestions(&w.db, &settings, today).await.unwrap();

        assert_eq!(
            rows.iter().map(|r| r.entry.day).collect::<Vec<_>>(),
            [0, 3, 6].map(|back| days_before(today, back))
        );
        for (row, (back, id)) in rows.iter().zip(&expected) {
            let day = days_before(today, *back);
            assert_eq!(
                row.entry.id,
                format!("stored:{id}:{}:{}", on(day, 9, 0), on(day, 10, 0))
            );
        }
    }

    #[tokio::test]
    async fn without_suggestions_there_are_no_rows() {
        let w = World::new().await;
        let settings = plain_settings(Continuity::Off);

        assert!(
            recent_suggestions(&w.db, &settings, today())
                .await
                .unwrap()
                .is_empty()
        );
    }
}
