//! `timetracker recordings` and `suggestions`: each entry with its id, its times as milliseconds and local
//! ISO text, and the names of its project, client and tag.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::day::{self, Names};
use crate::db::Db;
use crate::db::recordings::{self, Recording};
use crate::db::time::{Day, Ms, now_ms, to_local_iso};
use crate::error::{AppError, AppResult};
use crate::reports::{self, Range};
use crate::rules::{self, types::Suggestion};
use crate::state::AppState;
use crate::store::settings::Settings;

/// Suggestions are rebuilt per day, so a listing covers at most this many days.
const MAX_SUGGESTION_DAYS: i64 = 62;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpanEntry {
    pub start: Ms,
    /// `None` while the timer runs.
    pub end: Option<Ms>,
    pub start_iso: String,
    pub end_iso: Option<String>,
}

impl SpanEntry {
    fn new(start: Ms, end: Option<Ms>) -> Self {
        Self {
            start,
            end,
            start_iso: to_local_iso(start),
            end_iso: end.map(to_local_iso),
        }
    }
}

/// The names behind an entry's ids; archived and deleted ones included where they still resolve.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Labels {
    /// None for a stored suggestion without a project (ADR 0030).
    pub project_id: Option<String>,
    pub project: Option<String>,
    pub client: Option<String>,
    pub tag_id: Option<String>,
    pub tag: Option<String>,
    pub note: String,
}

impl Labels {
    fn new(names: &Names, project_id: Option<&str>, tag_id: Option<&str>, note: &str) -> Self {
        let project = project_id.and_then(|id| names.projects.get(id));
        Self {
            project_id: project_id.map(String::from),
            project: project_id
                .map(|id| project.map_or_else(|| id.to_string(), |p| p.name.clone())),
            client: project.and_then(|p| p.client_name.clone()),
            tag_id: tag_id.map(String::from),
            tag: tag_id
                .and_then(|id| names.tags.get(id))
                .map(|t| t.name.clone()),
            note: note.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingEntry {
    pub id: String,
    /// The day of its first span.
    pub day: Day,
    pub running: bool,
    pub duration_ms: Ms,
    pub spans: Vec<SpanEntry>,
    #[serde(flatten)]
    pub labels: Labels,
}

/// What made a suggestion: rules, or another app that stored it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SuggestionKind {
    Rule,
    Stored,
}

/// A suggestion as the Day view shows it; a stored one with all of its shown pieces as spans.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionEntry {
    /// The stored suggestion's id, or the rule suggestion's (`<rule id>:<start>:<end>`).
    pub id: String,
    pub kind: SuggestionKind,
    /// The rules' names or the source.
    pub by: String,
    pub day: Day,
    pub duration_ms: Ms,
    pub spans: Vec<SpanEntry>,
    #[serde(flatten)]
    pub labels: Labels,
}

fn recording_entry(recording: Recording, names: &Names, now: Ms) -> RecordingEntry {
    RecordingEntry {
        day: Day::of(recording.start()),
        running: recording.is_running(),
        duration_ms: recording.duration(now),
        spans: recording
            .spans
            .iter()
            .map(|s| SpanEntry::new(s.start, s.end))
            .collect(),
        labels: Labels::new(
            names,
            Some(&recording.project_id),
            recording.tag_id.as_deref(),
            &recording.note,
        ),
        id: recording.id,
    }
}

/// The recordings with time on the days `from` to `to`, by first start.
pub async fn recordings(db: &Db, from: Day, to: Day, now: Ms) -> AppResult<Vec<RecordingEntry>> {
    let names = day::names(db).await?;
    Ok(
        recordings::list_for_range(db, from.bounds().0, to.bounds().1)
            .await?
            .into_iter()
            .map(|r| recording_entry(r, &names, now))
            .collect(),
    )
}

fn suggestion_entry(
    first: &Suggestion,
    pieces: &[&Suggestion],
    names: &Names,
    day: Day,
) -> SuggestionEntry {
    let (id, kind) = match &first.stored_id {
        Some(stored_id) => (stored_id.clone(), SuggestionKind::Stored),
        None => (first.id.clone(), SuggestionKind::Rule),
    };
    SuggestionEntry {
        id,
        kind,
        by: first.by.clone(),
        day,
        duration_ms: pieces.iter().map(|s| s.duration_ms).sum(),
        spans: pieces
            .iter()
            .map(|s| SpanEntry::new(s.start, Some(s.end)))
            .collect(),
        labels: Labels::new(
            names,
            first.project_id.as_deref(),
            first.tag_id.as_deref(),
            &first.note,
        ),
    }
}

/// The day's suggestions as entries: each rule piece alone, the pieces of a stored one together, in the
/// order their first piece shows.
fn grouped(shown: &[Suggestion]) -> Vec<Vec<&Suggestion>> {
    let mut groups: Vec<Vec<&Suggestion>> = Vec::new();
    let mut stored: HashMap<&str, usize> = HashMap::new();
    for suggestion in shown {
        match suggestion.stored_id.as_deref() {
            Some(id) => match stored.get(id) {
                Some(&index) => groups[index].push(suggestion),
                None => {
                    stored.insert(id, groups.len());
                    groups.push(vec![suggestion]);
                }
            },
            None => groups.push(vec![suggestion]),
        }
    }
    groups
}

/// The suggestions the Day view shows on the days `from` to `to` (at most `MAX_SUGGESTION_DAYS`), by day
/// and start.
pub async fn suggestions(
    db: &Db,
    settings: &Settings,
    from: Day,
    to: Day,
) -> AppResult<Vec<SuggestionEntry>> {
    if (to.date() - from.date()).num_days() >= MAX_SUGGESTION_DAYS {
        return Err(AppError::InvalidInput(format!(
            "List at most {MAX_SUGGESTION_DAYS} days of suggestions at once"
        )));
    }
    let names = day::names(db).await?;
    let mut entries = Vec::new();
    for day in Day::range(&from, &to) {
        let shown = rules::suggestions_of(db, day, settings).await?;
        for pieces in grouped(&shown) {
            entries.push(suggestion_entry(pieces[0], &pieces, &names, day));
        }
    }
    Ok(entries)
}

/// `timetracker recordings RANGE` in the running app.
pub async fn recordings_in(app: &AppHandle, range: &Range) -> AppResult<Vec<RecordingEntry>> {
    let (from, to) = reports::resolve(range, Day::today())?;
    recordings(&app.state::<AppState>().data.db()?, from, to, now_ms()).await
}

/// `timetracker suggestions RANGE` in the running app.
pub async fn suggestions_in(app: &AppHandle, range: &Range) -> AppResult<Vec<SuggestionEntry>> {
    let (from, to) = reports::resolve(range, Day::today())?;
    let state = app.state::<AppState>();
    suggestions(&state.data.db()?, &state.settings.get(), from, to).await
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio::sync::Mutex;

    use super::*;
    use crate::db::projects::{self, Project, ProjectInput};
    use crate::db::recordings::NewRecording;
    use crate::db::stored_suggestions::StoredSuggestion;
    use crate::db::tags::{self, Tag};
    use crate::error::ErrorKind;
    use crate::recordings::undo::UndoStack;
    use crate::recordings::{RecordingInput, Recordings, Span, SuggestionInput};
    use crate::rules::testing::{on, plain_settings, rule_input};
    use crate::rules::types::{RuleField, RuleOp};
    use crate::store::settings::Continuity;
    use crate::test_support::{activity, client, project};

    const MIN: Ms = 60_000;

    fn day_of(text: &str) -> Day {
        Day::parse(text).unwrap()
    }

    fn now() -> Ms {
        on(day_of("2026-10-23"), 12, 0)
    }

    struct World {
        db: Db,
        portal: Project,
        podcast: Project,
        billing: Tag,
        undo: Mutex<UndoStack>,
    }

    impl World {
        /// Portal (client Acme) and Podcast, and a `billing` tag.
        async fn new() -> Self {
            let db = Db::open_memory().await.unwrap();
            let acme = client(&db, "Acme").await;
            let portal = projects::insert(
                &db,
                ProjectInput {
                    client_id: Some(acme.id),
                    name: "Portal".into(),
                    color: "blue".into(),
                    shade: None,
                    billable: true,
                },
            )
            .await
            .unwrap();
            let podcast = project(&db, "Podcast").await;
            let billing = tags::insert(&db, "billing").await.unwrap();
            Self {
                db,
                portal,
                podcast,
                billing,
                undo: Mutex::new(UndoStack::default()),
            }
        }

        fn service(&self) -> Recordings<'_> {
            Recordings {
                db: &self.db,
                undo: &self.undo,
                snap_minutes: 5,
            }
        }

        /// A recording of `of` with the given spans, `(from, to)` as `(hour, minute)` on `day`.
        async fn record(
            &self,
            of: &Project,
            day: Day,
            spans: &[((i64, i64), Option<(i64, i64)>)],
            note: &str,
            tag: Option<&Tag>,
        ) -> String {
            crate::db::recordings::insert(
                &self.db,
                NewRecording {
                    id: uuid::Uuid::new_v4().to_string(),
                    project_id: of.id.clone(),
                    spans: spans
                        .iter()
                        .map(|&(from, to)| {
                            (on(day, from.0, from.1), to.map(|to| on(day, to.0, to.1)))
                        })
                        .collect(),
                    note: note.into(),
                    tag_id: tag.map(|t| t.id.clone()),
                },
            )
            .await
            .unwrap()
            .id
        }

        async fn stored(
            &self,
            day: Day,
            spans: &[((i64, i64), (i64, i64))],
            source: &str,
        ) -> StoredSuggestion {
            let input = RecordingInput {
                project_id: self.portal.id.clone(),
                spans: spans
                    .iter()
                    .map(|&(from, to)| Span {
                        start: on(day, from.0, from.1),
                        end: on(day, to.0, to.1),
                    })
                    .collect(),
                note: "from planner".into(),
                tag_id: Some(self.billing.id.clone()),
            };
            self.service()
                .create_suggestion(input, source)
                .await
                .unwrap()
                .value
        }

        fn portal_labels(&self, note: &str, tagged: bool) -> Labels {
            Labels {
                project_id: Some(self.portal.id.clone()),
                project: Some("Portal".into()),
                client: Some("Acme".into()),
                tag_id: tagged.then(|| self.billing.id.clone()),
                tag: tagged.then(|| "billing".into()),
                note: note.into(),
            }
        }
    }

    #[tokio::test]
    async fn recordings_are_listed_by_first_start_with_ids_spans_and_names() {
        let w = World::new().await;
        let today = day_of("2026-10-20");
        let late = w
            .record(
                &w.portal,
                today,
                &[((13, 0), Some((14, 0)))],
                "review",
                Some(&w.billing),
            )
            .await;
        let early = w
            .record(
                &w.podcast,
                today,
                &[((8, 0), Some((8, 25)))],
                "outline",
                None,
            )
            .await;

        let entries = recordings(&w.db, today, today, now()).await.unwrap();

        assert_eq!(
            entries,
            [
                RecordingEntry {
                    id: early,
                    day: today,
                    running: false,
                    duration_ms: 25 * MIN,
                    spans: vec![SpanEntry::new(on(today, 8, 0), Some(on(today, 8, 25)))],
                    labels: Labels {
                        project_id: Some(w.podcast.id.clone()),
                        project: Some("Podcast".into()),
                        client: None,
                        tag_id: None,
                        tag: None,
                        note: "outline".into(),
                    },
                },
                RecordingEntry {
                    id: late,
                    day: today,
                    running: false,
                    duration_ms: 60 * MIN,
                    spans: vec![SpanEntry::new(on(today, 13, 0), Some(on(today, 14, 0)))],
                    labels: w.portal_labels("review", true),
                },
            ]
        );
    }

    #[tokio::test]
    async fn a_recording_with_several_spans_adds_them_up() {
        let w = World::new().await;
        let today = day_of("2026-10-20");
        w.record(
            &w.portal,
            today,
            &[((9, 0), Some((10, 0))), ((14, 0), Some((14, 30)))],
            "",
            None,
        )
        .await;

        let entries = recordings(&w.db, today, today, now()).await.unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].duration_ms, 90 * MIN);
        assert_eq!(
            entries[0].spans,
            [
                SpanEntry::new(on(today, 9, 0), Some(on(today, 10, 0))),
                SpanEntry::new(on(today, 14, 0), Some(on(today, 14, 30))),
            ]
        );
    }

    #[tokio::test]
    async fn a_running_timer_is_listed_with_an_open_span_and_its_time_so_far() {
        let w = World::new().await;
        let today = day_of("2026-10-23");
        w.record(&w.portal, today, &[((11, 0), None)], "call", None)
            .await;

        let entries = recordings(&w.db, today, today, now()).await.unwrap();

        assert_eq!(entries.len(), 1);
        assert!(entries[0].running);
        assert_eq!(entries[0].duration_ms, 60 * MIN);
        assert_eq!(entries[0].spans, [SpanEntry::new(on(today, 11, 0), None)]);
    }

    #[tokio::test]
    async fn recordings_outside_the_days_are_left_out() {
        let w = World::new().await;
        let (monday, tuesday) = (day_of("2026-10-19"), day_of("2026-10-20"));
        w.record(&w.portal, monday, &[((9, 0), Some((10, 0)))], "", None)
            .await;
        let kept = w
            .record(&w.portal, tuesday, &[((9, 0), Some((10, 0)))], "", None)
            .await;

        let entries = recordings(&w.db, tuesday, tuesday, now()).await.unwrap();

        assert_eq!(
            entries.into_iter().map(|e| e.id).collect::<Vec<_>>(),
            [kept]
        );
    }

    #[tokio::test]
    async fn a_stored_suggestion_is_one_entry_with_all_its_pieces() {
        let w = World::new().await;
        let today = day_of("2026-01-05");
        let saved = w.stored(today, &[((9, 0), (12, 0))], "planner").await;
        w.record(&w.podcast, today, &[((10, 0), Some((11, 0)))], "", None)
            .await;
        let settings = plain_settings(Continuity::Off);

        let entries = suggestions(&w.db, &settings, today, today).await.unwrap();

        assert_eq!(
            entries,
            [SuggestionEntry {
                id: saved.id,
                kind: SuggestionKind::Stored,
                by: "planner".into(),
                day: today,
                duration_ms: 120 * MIN,
                spans: vec![
                    SpanEntry::new(on(today, 9, 0), Some(on(today, 10, 0))),
                    SpanEntry::new(on(today, 11, 0), Some(on(today, 12, 0))),
                ],
                labels: w.portal_labels("from planner", true),
            }]
        );
    }

    #[tokio::test]
    async fn a_stored_suggestion_without_a_project_is_listed_with_no_project_labels() {
        let w = World::new().await;
        let today = day_of("2026-01-05");
        let input = SuggestionInput {
            project_id: None,
            spans: vec![Span {
                start: on(today, 9, 0),
                end: on(today, 10, 0),
            }],
            note: "to sort".into(),
            tag_id: None,
        };
        let saved = w
            .service()
            .create_suggestion(input, "planner")
            .await
            .unwrap()
            .value;
        let settings = plain_settings(Continuity::Off);

        let entries = suggestions(&w.db, &settings, today, today).await.unwrap();

        assert_eq!(
            entries,
            [SuggestionEntry {
                id: saved.id,
                kind: SuggestionKind::Stored,
                by: "planner".into(),
                day: today,
                duration_ms: 60 * MIN,
                spans: vec![SpanEntry::new(on(today, 9, 0), Some(on(today, 10, 0)))],
                labels: Labels {
                    project_id: None,
                    project: None,
                    client: None,
                    tag_id: None,
                    tag: None,
                    note: "to sort".into(),
                },
            }]
        );
        let json = serde_json::to_value(&entries[0]).unwrap();
        assert_eq!(
            (&json["projectId"], &json["project"], &json["client"]),
            (
                &serde_json::Value::Null,
                &serde_json::Value::Null,
                &serde_json::Value::Null
            )
        );
    }

    #[tokio::test]
    async fn a_rule_suggestion_is_listed_under_its_piece_id_and_the_rules_name() {
        let w = World::new().await;
        let today = day_of("2026-01-05");
        let rule = crate::rules::store::save(
            &w.db,
            None,
            &rule_input(
                "Slack rule",
                &w.portal.id,
                &[(RuleField::App, RuleOp::Is, "Slack")],
            ),
        )
        .await
        .unwrap();
        activity(&w.db, on(today, 9, 0), Some(on(today, 10, 0)), "Slack", "").await;
        let settings = plain_settings(Continuity::Off);

        let entries = suggestions(&w.db, &settings, today, today).await.unwrap();

        assert_eq!(
            entries,
            [SuggestionEntry {
                id: format!("{}:{}:{}", rule.id, on(today, 9, 0), on(today, 10, 0)),
                kind: SuggestionKind::Rule,
                by: "Slack rule".into(),
                day: today,
                duration_ms: 60 * MIN,
                spans: vec![SpanEntry::new(on(today, 9, 0), Some(on(today, 10, 0)))],
                labels: w.portal_labels("", false),
            }]
        );
    }

    #[tokio::test]
    async fn suggestions_of_several_days_come_in_day_order() {
        let w = World::new().await;
        let (first, second) = (day_of("2026-01-05"), day_of("2026-01-06"));
        let later = w.stored(second, &[((9, 0), (10, 0))], "cli").await;
        let earlier = w.stored(first, &[((15, 0), (16, 0))], "cli").await;
        let settings = plain_settings(Continuity::Off);

        let entries = suggestions(&w.db, &settings, first, second).await.unwrap();

        assert_eq!(
            entries
                .iter()
                .map(|e| (e.id.as_str(), e.day))
                .collect::<Vec<_>>(),
            [(earlier.id.as_str(), first), (later.id.as_str(), second)]
        );
    }

    #[tokio::test]
    async fn no_suggestions_make_an_empty_list() {
        let w = World::new().await;
        let today = day_of("2026-01-05");
        let settings = plain_settings(Continuity::Off);

        assert!(
            suggestions(&w.db, &settings, today, today)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn suggestions_are_listed_for_at_most_62_days_at_once() {
        let w = World::new().await;
        let from = day_of("2026-01-01");
        let settings = plain_settings(Continuity::Off);
        let last = (0..62).fold(from, |day, _| day.next());

        let err = suggestions(&w.db, &settings, from, last).await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(
            err.to_string(),
            "List at most 62 days of suggestions at once"
        );
        assert!(
            suggestions(&w.db, &settings, from, last.prev())
                .await
                .is_ok()
        );
    }

    #[test]
    fn entries_have_camel_case_fields_with_the_labels_beside_them() {
        let labels = Labels {
            project_id: Some("p1".into()),
            project: Some("Portal".into()),
            client: None,
            tag_id: Some("t1".into()),
            tag: Some("billing".into()),
            note: "n".into(),
        };
        let recording = RecordingEntry {
            id: "r1".into(),
            day: day_of("2026-10-20"),
            running: true,
            duration_ms: 1_500,
            spans: vec![SpanEntry {
                start: 1_000,
                end: None,
                start_iso: "a".into(),
                end_iso: None,
            }],
            labels: labels.clone(),
        };
        let suggestion = SuggestionEntry {
            id: "sg1".into(),
            kind: SuggestionKind::Rule,
            by: "Slack rule".into(),
            day: day_of("2026-10-20"),
            duration_ms: 1_500,
            spans: vec![],
            labels,
        };

        assert_eq!(
            serde_json::to_value(&recording).unwrap(),
            json!({
                "id": "r1", "day": "2026-10-20", "running": true, "durationMs": 1500,
                "spans": [{ "start": 1000, "end": null, "startIso": "a", "endIso": null }],
                "projectId": "p1", "project": "Portal", "client": null,
                "tagId": "t1", "tag": "billing", "note": "n"
            })
        );
        assert_eq!(
            serde_json::to_value(&suggestion).unwrap(),
            json!({
                "id": "sg1", "kind": "rule", "by": "Slack rule", "day": "2026-10-20",
                "durationMs": 1500, "spans": [],
                "projectId": "p1", "project": "Portal", "client": null,
                "tagId": "t1", "tag": "billing", "note": "n"
            })
        );
    }
}
