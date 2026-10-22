//! The notes a project (or a whole report) offers to copy into a company dashboard.

use std::collections::HashSet;

use serde::Serialize;
use ts_rs::TS;

use super::ReportRow;

/// Notes as copied text: one per line, without a trailing line break.
#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct CopiedNotes {
    pub text: String,
    pub count: u32,
}

/// The notes of `rows` (sorted by start) in that order: empty notes skipped, repeated notes kept, and a
/// recording's note once even when it spans several days.
pub(super) fn copied_notes<'a>(rows: impl IntoIterator<Item = &'a ReportRow>) -> CopiedNotes {
    let mut seen = HashSet::new();
    let lines: Vec<&str> = rows
        .into_iter()
        .filter(|row| !row.note.is_empty() && seen.insert(row.id.as_str()))
        .map(|row| row.note.as_str())
        .collect();
    CopiedNotes {
        text: lines.join("\n"),
        count: lines.len() as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::recordings::Recording;
    use crate::reports::ReportFilter;
    use crate::reports::testing::*;

    const NEXT: &str = "2026-10-21";

    fn at_nine(id: &str, project: &str, d: &str, minute: i64, note: &str) -> Recording {
        let start = at(d, 9, minute);
        with_note(
            rec(id, project, start, Some(start + minutes(10)), "", None),
            note,
        )
    }

    fn notes_of(recordings: Vec<Recording>) -> CopiedNotes {
        report(&ReportFilter::default(), D, NEXT, recordings).notes
    }

    #[test]
    fn notes_follow_the_start_order_across_projects() {
        let notes = notes_of(vec![
            at_nine("r3", "p-site", D, 40, "third"),
            at_nine("r1", "p-portal", D, 0, "first"),
            at_nine("r2", "p-api", D, 20, "second"),
        ]);

        assert_eq!(
            notes,
            CopiedNotes {
                text: "first\nsecond\nthird".into(),
                count: 3,
            }
        );
    }

    #[test]
    fn empty_notes_are_skipped_and_repeated_notes_are_kept_in_order() {
        let notes = notes_of(vec![
            at_nine("r1", "p-portal", D, 0, "standup"),
            at_nine("r2", "p-portal", D, 10, ""),
            at_nine("r3", "p-api", D, 20, "standup"),
            at_nine("r4", "p-api", D, 30, "review"),
        ]);

        assert_eq!(
            notes,
            CopiedNotes {
                text: "standup\nstandup\nreview".into(),
                count: 3,
            }
        );
    }

    #[test]
    fn a_recording_over_several_days_gives_its_note_once() {
        let spanning = with_note(
            in_spans("r1", "p-portal", &[(D, 9, 0, 10, 0), (NEXT, 9, 0, 10, 0)]),
            "long task",
        );

        let notes = notes_of(vec![spanning, at_nine("r2", "p-api", D, 30, "other")]);

        assert_eq!(
            notes,
            CopiedNotes {
                text: "long task\nother".into(),
                count: 2,
            }
        );
    }

    #[test]
    fn no_notes_give_empty_text_and_a_zero_count() {
        let notes = notes_of(vec![at_nine("r1", "p-portal", D, 0, "")]);

        assert_eq!(notes, CopiedNotes::default());
    }

    #[test]
    fn copied_notes_serialize_with_text_and_count() {
        let notes = CopiedNotes {
            text: "a\nb".into(),
            count: 2,
        };

        assert_eq!(
            serde_json::to_value(&notes).unwrap(),
            serde_json::json!({ "text": "a\nb", "count": 2 })
        );
    }
}
