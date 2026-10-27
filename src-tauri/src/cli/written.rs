//! The text of the scripting replies: writes that are suggestions unless final, the listings with ids, and
//! undo (ADR 0028).

use chrono::TimeZone;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::reply::{clock, read, timer};
use crate::db::time::{Ms, format_hm};
use crate::ipc::IpcMessage;
use crate::recordings::cli::Recorded;
use crate::scripting::catalog::{ClientEntry, ProjectEntry, TagEntry};
use crate::scripting::listing::{Labels, RecordingEntry, SpanEntry, SuggestionEntry};
use crate::scripting::timer::{RecentTimerEntry, TimerState};
use crate::scripting::writes::{Deleted, DeletedKind, Written};

/// `09:15–09:45, 14:00–now`.
fn spans_text<Tz: TimeZone>(spans: impl IntoIterator<Item = (Ms, Option<Ms>)>, tz: &Tz) -> String {
    spans
        .into_iter()
        .map(|(start, end)| {
            let end = end.map_or_else(|| "now".to_string(), |end| clock(end, tz));
            format!("{}–{end}", clock(start, tz))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn entry_spans(spans: &[SpanEntry]) -> impl Iterator<Item = (Ms, Option<Ms>)> + '_ {
    spans.iter().map(|s| (s.start, s.end))
}

/// ` · note`, or nothing for an empty one.
fn note_part(note: &str) -> String {
    if note.is_empty() {
        String::new()
    } else {
        format!(" · {note}")
    }
}

/// Shown in place of a missing project.
const NO_PROJECT: &str = "no project";

/// `Acme · note #tag`.
fn labels_text(labels: &Labels) -> String {
    let tag = labels
        .tag
        .as_ref()
        .map_or_else(String::new, |t| format!(" #{t}"));
    let project = labels.project.as_deref().unwrap_or(NO_PROJECT);
    format!("{project}{}{tag}", note_part(&labels.note))
}

fn recorded_text<Tz: TimeZone>(recorded: &Recorded, now: Ms, tz: &Tz) -> String {
    let r = &recorded.recording;
    format!(
        "recorded {} ({}) for {}{}",
        spans_text(r.spans.iter().map(|s| (s.start, s.end)), tz),
        format_hm(r.duration(now)),
        recorded.project_name,
        note_part(&r.note)
    )
}

fn written_text<Tz: TimeZone>(written: &Written, now: Ms, tz: &Tz) -> String {
    match written {
        Written::Recording(recorded) => recorded_text(recorded, now, tz),
        Written::Suggestion(suggested) => {
            let s = &suggested.suggestion;
            format!(
                "suggested {} ({}) for {}{}; review it in the Day view (id {})",
                spans_text(s.spans.iter().map(|p| (p.start, Some(p.end))), tz),
                format_hm(s.duration()),
                suggested.project_name.as_deref().unwrap_or(NO_PROJECT),
                note_part(&s.note),
                s.id
            )
        }
    }
}

/// One line per entry of the listing `result`, or `empty` without any.
fn listing<T: DeserializeOwned>(
    result: &Value,
    empty: &str,
    line: impl Fn(&T) -> String,
) -> Result<String, String> {
    let entries = read::<Vec<T>>(result)?;
    Ok(if entries.is_empty() {
        empty.to_string()
    } else {
        entries.iter().map(line).collect::<Vec<_>>().join("\n")
    })
}

/// `undid Record 30m for Acme`, or `nothing` when there was nothing to undo or redo.
fn history(result: &Value, done: &str, nothing: &str) -> Result<String, String> {
    Ok(match read::<Option<String>>(result)? {
        Some(label) => format!("{done} {label}"),
        None => nothing.to_string(),
    })
}

fn archived(archived: bool) -> &'static str {
    if archived { " (archived)" } else { "" }
}

/// The text of a scripting reply; `None` for another message.
pub fn render<Tz: TimeZone>(
    message: &IpcMessage,
    result: &Value,
    now: Ms,
    tz: &Tz,
) -> Result<Option<String>, String> {
    Ok(Some(match message {
        IpcMessage::Record { .. } | IpcMessage::Edit { .. } => {
            written_text(&read::<Written>(result)?, now, tz)
        }
        IpcMessage::Delete { .. } => {
            let deleted = read::<Deleted>(result)?;
            let what = match deleted.kind {
                DeletedKind::Recording => "recording",
                DeletedKind::Suggestion => "suggestion",
            };
            format!("deleted the {what} {}", deleted.id)
        }
        IpcMessage::Approve { .. } => listing(result, "nothing approved", |r: &Recorded| {
            recorded_text(r, now, tz)
        })?,
        IpcMessage::Recordings { .. } => listing(result, "no recordings", |r: &RecordingEntry| {
            format!(
                "{} {} ({}) {}  {}",
                r.day,
                spans_text(entry_spans(&r.spans), tz),
                format_hm(r.duration_ms),
                labels_text(&r.labels),
                r.id
            )
        })?,
        IpcMessage::Suggestions { .. } => {
            listing(result, "no suggestions", |s: &SuggestionEntry| {
                format!(
                    "{} {} ({}) {} by {}  {}",
                    s.day,
                    spans_text(entry_spans(&s.spans), tz),
                    format_hm(s.duration_ms),
                    labels_text(&s.labels),
                    s.by,
                    s.id
                )
            })?
        }
        IpcMessage::Projects { .. } => listing(result, "no projects", |p: &ProjectEntry| {
            let client = p
                .client
                .as_ref()
                .map_or_else(String::new, |c| format!(" · {c}"));
            format!("{}{client}{}  {}", p.name, archived(p.archived), p.id)
        })?,
        IpcMessage::Clients { .. } => listing(result, "no clients", |c: &ClientEntry| {
            format!("{}{}  {}", c.name, archived(c.archived), c.id)
        })?,
        IpcMessage::Tags => listing(result, "no tags", |t: &TagEntry| {
            let emoji = t
                .emoji
                .as_ref()
                .map_or_else(String::new, |e| format!("{e} "));
            format!("{emoji}{}  {}", t.name, t.id)
        })?,
        IpcMessage::TimerRecent { .. } => {
            listing(result, "no recent timers", |t: &RecentTimerEntry| {
                let tag = t
                    .tag
                    .as_ref()
                    .map_or_else(String::new, |tag| format!(" #{tag}"));
                format!("{}{}{tag}  {}", t.project, note_part(&t.note), t.project_id)
            })?
        }
        IpcMessage::TimerStatus | IpcMessage::TimerUpdate { .. } => {
            format!("timer: {}", timer(&read::<TimerState>(result)?, now))
        }
        IpcMessage::Undo => history(result, "undid", "nothing to undo")?,
        IpcMessage::Redo => history(result, "redid", "nothing to redo")?,
        _ => return Ok(None),
    }))
}

#[cfg(test)]
mod tests {
    use chrono_tz::Europe::Vienna;
    use serde_json::{Value, json};

    use super::*;
    use crate::cli::reply::render;
    use crate::cli::testing::*;
    use crate::db::recordings::{Recording, RecordingSpan, Span};
    use crate::db::stored_suggestions::StoredSuggestion;
    use crate::recordings::Overlap;
    use crate::recordings::cli::RecordRequest;
    use crate::scripting::writes::{EditRequest, Suggested};

    const WRONG_SHAPE: &str = "The running TimeTracker sent an unexpected reply";

    fn render_at_now(message: &IpcMessage, result: &Value) -> Result<Option<String>, String> {
        render(message, result, now(), &Vienna)
    }

    fn text(message: &IpcMessage, result: &Value) -> String {
        render_at_now(message, result).unwrap().unwrap()
    }

    fn record_message() -> IpcMessage {
        IpcMessage::Record {
            request: RecordRequest {
                project: Some("acme".into()),
                spans: vec![Span { start: 0, end: 1 }],
                note: String::new(),
                tag: None,
                overlap: Overlap::Cancel,
                final_write: false,
                source: None,
            },
        }
    }

    fn edit_message() -> IpcMessage {
        IpcMessage::Edit {
            request: EditRequest {
                id: "abc".into(),
                project: None,
                note: None,
                tag: None,
                clear_tag: false,
                spans: None,
                final_write: true,
                overlap: Overlap::Cancel,
            },
        }
    }

    fn delete_message() -> IpcMessage {
        IpcMessage::Delete {
            id: "abc".into(),
            final_write: false,
        }
    }

    fn approve_message() -> IpcMessage {
        IpcMessage::Approve {
            ids: vec!["abc".into()],
        }
    }

    fn recorded(note: &str, spans: &[(&str, Option<&str>)]) -> Recorded {
        Recorded {
            recording: Recording {
                id: "r1".into(),
                project_id: "p1".into(),
                note: note.into(),
                tag_id: None,
                created_at: 0,
                updated_at: 0,
                spans: spans
                    .iter()
                    .enumerate()
                    .map(|(i, &(start, end))| RecordingSpan {
                        id: format!("s{i}"),
                        start: ms(start),
                        end: end.map(ms),
                    })
                    .collect(),
            },
            project_name: "Acme".into(),
        }
    }

    fn suggested(note: &str, spans: &[(&str, &str)]) -> Written {
        Written::Suggestion(Suggested {
            suggestion: StoredSuggestion {
                id: "sg1".into(),
                project_id: Some("p1".into()),
                note: note.into(),
                tag_id: None,
                source: "cli".into(),
                day: day("2026-10-21"),
                spans: spans
                    .iter()
                    .map(|&(start, end)| Span {
                        start: ms(start),
                        end: ms(end),
                    })
                    .collect(),
                created_at: 0,
                updated_at: 0,
            },
            project_name: Some("Internal".into()),
        })
    }

    fn suggested_without_project(note: &str, spans: &[(&str, &str)]) -> Written {
        let Written::Suggestion(with_project) = suggested(note, spans) else {
            unreachable!("suggested builds a suggestion");
        };
        Written::Suggestion(Suggested {
            suggestion: StoredSuggestion {
                project_id: None,
                ..with_project.suggestion
            },
            project_name: None,
        })
    }

    #[test]
    fn a_suggested_record_without_a_project_says_no_project() {
        let written = suggested_without_project(
            "note",
            &[("2026-10-21T06:00:00+02:00", "2026-10-21T06:30:00+02:00")],
        );

        assert_eq!(
            text(&record_message(), &serde_json::to_value(written).unwrap()),
            "suggested 06:00–06:30 (0:30) for no project · note; review it in the Day view (id sg1)"
        );
    }

    #[test]
    fn a_listed_suggestion_without_a_project_says_no_project() {
        let result = json!([{
            "id": "sg1", "kind": "stored", "by": "claude", "day": "2026-10-21",
            "durationMs": 1_800_000,
            "spans": [{
                "start": ms("2026-10-21T06:00:00+02:00"),
                "end": ms("2026-10-21T06:30:00+02:00"),
                "startIso": "2026-10-21T06:00:00+02:00",
                "endIso": "2026-10-21T06:30:00+02:00"
            }],
            "projectId": null, "project": null, "client": null,
            "tagId": null, "tag": null, "note": "note"
        }]);

        assert_eq!(
            text(
                &IpcMessage::Suggestions {
                    range: crate::reports::Range::Today
                },
                &result
            ),
            "2026-10-21 06:00–06:30 (0:30) no project · note by claude  sg1"
        );
    }

    #[test]
    fn a_suggested_record_names_the_times_the_project_the_note_and_the_id() {
        let written = suggested(
            "note",
            &[("2026-10-21T06:00:00+02:00", "2026-10-21T06:30:00+02:00")],
        );

        assert_eq!(
            text(&record_message(), &serde_json::to_value(written).unwrap()),
            "suggested 06:00–06:30 (0:30) for Internal · note; review it in the Day view (id sg1)"
        );
    }

    #[test]
    fn a_suggestion_with_several_spans_lists_them_and_adds_them_up() {
        let written = suggested(
            "",
            &[
                ("2026-10-21T09:00:00+02:00", "2026-10-21T10:00:00+02:00"),
                ("2026-10-21T11:00:00+02:00", "2026-10-21T12:15:00+02:00"),
            ],
        );

        assert_eq!(
            text(&record_message(), &serde_json::to_value(written).unwrap()),
            "suggested 09:00–10:00, 11:00–12:15 (2:15) for Internal; review it in the Day view (id sg1)"
        );
    }

    #[test]
    fn a_final_record_or_edit_prints_the_recording() {
        let written = Written::Recording(recorded(
            "fix login",
            &[(
                "2026-10-21T09:00:00+02:00",
                Some("2026-10-21T10:30:00+02:00"),
            )],
        ));
        let result = serde_json::to_value(written).unwrap();

        for message in [record_message(), edit_message()] {
            assert_eq!(
                text(&message, &result),
                "recorded 09:00–10:30 (1:30) for Acme · fix login",
                "{message:?}"
            );
        }
    }

    #[test]
    fn an_edited_suggestion_prints_like_a_new_one() {
        let written = suggested(
            "edited",
            &[("2026-10-21T06:00:00+02:00", "2026-10-21T07:00:00+02:00")],
        );

        assert_eq!(
            text(&edit_message(), &serde_json::to_value(written).unwrap()),
            "suggested 06:00–07:00 (1:00) for Internal · edited; review it in the Day view (id sg1)"
        );
    }

    #[test]
    fn delete_says_what_kind_of_thing_went() {
        for (kind, expected) in [
            ("suggestion", "deleted the suggestion abc"),
            ("recording", "deleted the recording abc"),
        ] {
            assert_eq!(
                text(&delete_message(), &json!({ "kind": kind, "id": "abc" })),
                expected
            );
        }
    }

    #[test]
    fn approve_prints_one_line_per_recorded_suggestion() {
        let first = recorded(
            "standup",
            &[(
                "2026-10-21T09:00:00+02:00",
                Some("2026-10-21T09:15:00+02:00"),
            )],
        );
        let second = recorded(
            "",
            &[
                (
                    "2026-10-21T10:00:00+02:00",
                    Some("2026-10-21T11:00:00+02:00"),
                ),
                (
                    "2026-10-21T13:00:00+02:00",
                    Some("2026-10-21T13:30:00+02:00"),
                ),
            ],
        );

        assert_eq!(
            text(
                &approve_message(),
                &serde_json::to_value([first, second]).unwrap()
            ),
            "recorded 09:00–09:15 (0:15) for Acme · standup\n\
             recorded 10:00–11:00, 13:00–13:30 (1:30) for Acme"
        );
    }

    #[test]
    fn an_empty_approval_says_nothing_was_approved() {
        assert_eq!(text(&approve_message(), &json!([])), "nothing approved");
    }

    #[test]
    fn recordings_are_listed_one_line_each_with_their_id() {
        let result = json!([
            {
                "id": "r1", "day": "2026-10-21", "running": false, "durationMs": 5_400_000,
                "spans": [{
                    "start": ms("2026-10-21T09:00:00+02:00"),
                    "end": ms("2026-10-21T10:30:00+02:00"),
                    "startIso": "2026-10-21T09:00:00+02:00",
                    "endIso": "2026-10-21T10:30:00+02:00"
                }],
                "projectId": "p1", "project": "Portal", "client": "Acme",
                "tagId": "t1", "tag": "development", "note": "fix login"
            },
            {
                "id": "r2", "day": "2026-10-21", "running": true, "durationMs": 1_800_000,
                "spans": [{
                    "start": ms("2026-10-21T11:30:00+02:00"),
                    "end": null,
                    "startIso": "2026-10-21T11:30:00+02:00",
                    "endIso": null
                }],
                "projectId": "p2", "project": "Podcast", "client": null,
                "tagId": null, "tag": null, "note": ""
            }
        ]);

        assert_eq!(
            text(
                &IpcMessage::Recordings {
                    range: crate::reports::Range::Today
                },
                &result
            ),
            "2026-10-21 09:00–10:30 (1:30) Portal · fix login #development  r1\n\
             2026-10-21 11:30–now (0:30) Podcast  r2"
        );
    }

    #[test]
    fn suggestions_are_listed_with_who_made_them_and_their_id() {
        let result = json!([{
            "id": "sg1", "kind": "stored", "by": "claude", "day": "2026-10-21",
            "durationMs": 1_800_000,
            "spans": [{
                "start": ms("2026-10-21T06:00:00+02:00"),
                "end": ms("2026-10-21T06:30:00+02:00"),
                "startIso": "2026-10-21T06:00:00+02:00",
                "endIso": "2026-10-21T06:30:00+02:00"
            }],
            "projectId": "p1", "project": "Internal", "client": null,
            "tagId": null, "tag": null, "note": "note"
        }]);

        assert_eq!(
            text(
                &IpcMessage::Suggestions {
                    range: crate::reports::Range::Today
                },
                &result
            ),
            "2026-10-21 06:00–06:30 (0:30) Internal · note by claude  sg1"
        );
    }

    #[test]
    fn projects_clients_and_tags_are_listed_with_their_ids() {
        let projects = json!([
            { "id": "p1", "name": "Portal", "clientId": "c1", "client": "Acme",
              "color": "teal", "shade": 4, "billable": true, "archived": false },
            { "id": "p2", "name": "Podcast", "clientId": null, "client": null,
              "color": "blue", "shade": 4, "billable": false, "archived": false },
            { "id": "p3", "name": "Old", "clientId": "c1", "client": "Acme",
              "color": "teal", "shade": 1, "billable": true, "archived": true },
        ]);
        let clients = json!([
            { "id": "c1", "name": "Acme", "archived": false },
            { "id": "c2", "name": "Globex", "archived": true },
        ]);
        let tags = json!([
            { "id": "t1", "name": "admin", "emoji": null },
            { "id": "t2", "name": "focus", "emoji": "🎯" },
        ]);

        assert_eq!(
            text(&IpcMessage::Projects { archived: true }, &projects),
            "Portal · Acme  p1\nPodcast  p2\nOld · Acme (archived)  p3"
        );
        assert_eq!(
            text(&IpcMessage::Clients { archived: true }, &clients),
            "Acme  c1\nGlobex (archived)  c2"
        );
        assert_eq!(text(&IpcMessage::Tags, &tags), "admin  t1\n🎯 focus  t2");
    }

    #[test]
    fn recent_timers_are_listed_with_note_tag_and_project_id() {
        let recent = json!([
            { "projectId": "p1", "project": "Portal", "client": "Acme", "tagId": "t1",
              "tag": "dev", "note": "standup", "lastUsedAt": 2_000 },
            { "projectId": "p2", "project": "Podcast", "client": null, "tagId": null,
              "tag": null, "note": "", "lastUsedAt": 1_000 },
            { "projectId": "p3", "project": "Admin", "client": null, "tagId": "t2",
              "tag": "focus", "note": "", "lastUsedAt": 500 },
        ]);

        assert_eq!(
            text(&IpcMessage::TimerRecent { limit: 8 }, &recent),
            "Portal · standup #dev  p1\nPodcast  p2\nAdmin #focus  p3"
        );
    }

    #[test]
    fn empty_listings_say_so() {
        let range = crate::reports::Range::Today;
        for (message, expected) in [
            (
                IpcMessage::Recordings {
                    range: range.clone(),
                },
                "no recordings",
            ),
            (IpcMessage::Suggestions { range }, "no suggestions"),
            (IpcMessage::Projects { archived: false }, "no projects"),
            (IpcMessage::Clients { archived: false }, "no clients"),
            (IpcMessage::Tags, "no tags"),
            (IpcMessage::TimerRecent { limit: 8 }, "no recent timers"),
        ] {
            assert_eq!(text(&message, &json!([])), expected, "{message:?}");
        }
    }

    #[test]
    fn undo_and_redo_name_what_they_changed_or_that_nothing_was_left() {
        for (message, label, expected) in [
            (
                IpcMessage::Undo,
                json!("Record 30m for Portal"),
                "undid Record 30m for Portal",
            ),
            (IpcMessage::Undo, Value::Null, "nothing to undo"),
            (
                IpcMessage::Redo,
                json!("Approve suggestion"),
                "redid Approve suggestion",
            ),
            (IpcMessage::Redo, Value::Null, "nothing to redo"),
        ] {
            assert_eq!(text(&message, &label), expected, "{message:?}");
        }
    }

    #[test]
    fn replies_of_the_wrong_shape_are_errors() {
        let range = crate::reports::Range::Today;
        for message in [
            record_message(),
            edit_message(),
            delete_message(),
            approve_message(),
            IpcMessage::Recordings {
                range: range.clone(),
            },
            IpcMessage::Suggestions { range },
            IpcMessage::Projects { archived: false },
            IpcMessage::Clients { archived: false },
            IpcMessage::Tags,
            IpcMessage::TimerRecent { limit: 8 },
            IpcMessage::Undo,
            IpcMessage::Redo,
        ] {
            let err = render_at_now(&message, &json!({ "nope": 1 })).unwrap_err();
            assert!(err.starts_with(WRONG_SHAPE), "{message:?}: {err}");
        }
    }

    #[test]
    fn a_listing_entry_missing_a_field_is_an_error() {
        let err =
            render_at_now(&IpcMessage::Tags, &json!([{ "id": "t1", "emoji": null }])).unwrap_err();

        assert!(err.starts_with(WRONG_SHAPE), "{err}");
    }
}
