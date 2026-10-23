//! Stored suggestions on the Day view (ADR 0028): each span without the time recordings, or a suggestion
//! stored before it, already take. A piece the cut leaves shorter than `MIN_SUGGESTION_MS` is dropped; a
//! whole span stays at any length.

use super::suggest::{MIN_SUGGESTION_MS, Span, subtract, union};
use super::types::Suggestion;
use crate::day::Names;
use crate::db::stored_suggestions::StoredSuggestion;

/// The pieces of `stored` (by creation, the earlier keeping shared time) outside `recorded`, by start; a
/// suggestion whose project is gone is left out.
pub fn pieces(stored: &[StoredSuggestion], recorded: &[Span], names: &Names) -> Vec<Suggestion> {
    let mut by_creation: Vec<&StoredSuggestion> = stored.iter().collect();
    by_creation.sort_by_key(|s| (s.created_at, s.id.clone()));
    let mut taken = union(recorded.to_vec());
    let mut found = Vec::new();
    for suggestion in by_creation {
        let Some(project) = names.projects.get(&suggestion.project_id) else {
            continue;
        };
        let tag_id = suggestion
            .tag_id
            .clone()
            .filter(|id| names.tags.contains_key(id));
        let tag = tag_id.as_ref().and_then(|id| names.tags.get(id));
        for span in &suggestion.spans {
            let whole = (span.start, span.end);
            let free = subtract(&[whole], &taken);
            for (start, end) in free {
                let is_whole = (start, end) == whole;
                if !is_whole && end - start < MIN_SUGGESTION_MS {
                    continue;
                }
                found.push(Suggestion {
                    id: format!("stored:{}:{start}:{end}", suggestion.id),
                    rule_ids: Vec::new(),
                    stored_id: Some(suggestion.id.clone()),
                    by: suggestion.source.clone(),
                    project_id: suggestion.project_id.clone(),
                    project_name: project.name.clone(),
                    client_name: project.client_name.clone(),
                    color: project.color.clone(),
                    shade: project.shade,
                    start,
                    end,
                    duration_ms: end - start,
                    note: suggestion.note.clone(),
                    tag_id: tag_id.clone(),
                    tag: tag.map(|t| t.name.clone()),
                    tag_emoji: tag.and_then(|t| t.emoji.clone()),
                    next_card: None,
                });
            }
        }
        let own = suggestion.spans.iter().map(|s| (s.start, s.end));
        taken = union(taken.into_iter().chain(own).collect());
    }
    found.sort_by_key(|s| s.start);
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::recordings::Span as StoredSpan;
    use crate::db::time::{Day, Ms};
    use crate::rules::testing::{MIN, names, recorded};

    fn stored(id: &str, created_at: Ms, spans: &[(i64, i64)]) -> StoredSuggestion {
        StoredSuggestion {
            id: id.into(),
            project_id: "p1".into(),
            note: "review".into(),
            tag_id: None,
            source: "claude".into(),
            day: Day::parse("2026-01-05").unwrap(),
            spans: spans
                .iter()
                .map(|&(start, end)| StoredSpan {
                    start: start * MIN,
                    end: end * MIN,
                })
                .collect(),
            created_at,
            updated_at: created_at,
        }
    }

    /// `(start minute, end minute)` of each piece.
    fn minutes(found: &[Suggestion]) -> Vec<(i64, i64)> {
        found.iter().map(|s| (s.start / MIN, s.end / MIN)).collect()
    }

    /// `(stored id, start minute, end minute)` of each piece.
    fn owners_and_minutes(found: &[Suggestion]) -> Vec<(&str, i64, i64)> {
        found
            .iter()
            .map(|s| (s.stored_id.as_deref().unwrap(), s.start / MIN, s.end / MIN))
            .collect()
    }

    fn pieces_of(stored: &[StoredSuggestion], recorded: &[(Ms, Ms)]) -> Vec<Suggestion> {
        pieces(stored, recorded, &names(&["p1", "p2"], &["t1"]))
    }

    #[test]
    fn a_piece_carries_the_projects_note_tag_and_source_of_its_suggestion() {
        let with_tag = StoredSuggestion {
            tag_id: Some("t1".into()),
            ..stored("s1", 1, &[(10, 30)])
        };

        let found = pieces_of(&[with_tag], &[]);

        assert_eq!(
            found,
            [Suggestion {
                id: format!("stored:s1:{}:{}", 10 * MIN, 30 * MIN),
                rule_ids: Vec::new(),
                stored_id: Some("s1".into()),
                by: "claude".into(),
                project_id: "p1".into(),
                project_name: "Project p1".into(),
                client_name: None,
                color: "blue".into(),
                shade: 4,
                start: 10 * MIN,
                end: 30 * MIN,
                duration_ms: 20 * MIN,
                note: "review".into(),
                tag_id: Some("t1".into()),
                tag: Some("tag t1".into()),
                tag_emoji: None,
                next_card: None,
            }]
        );
    }

    #[test]
    fn recorded_time_is_cut_out_of_a_span() {
        let cases: [(&str, Vec<(Ms, Ms)>, Vec<(i64, i64)>); 6] = [
            (
                "a recording in the middle yields two pieces",
                vec![recorded(20, 40)],
                vec![(0, 20), (40, 60)],
            ),
            (
                "several recordings leave the gaps",
                vec![recorded(10, 20), recorded(30, 50)],
                vec![(0, 10), (20, 30), (50, 60)],
            ),
            (
                "a covering recording leaves nothing",
                vec![recorded(-10, 70)],
                vec![],
            ),
            (
                "a recording elsewhere changes nothing",
                vec![recorded(70, 80)],
                vec![(0, 60)],
            ),
            (
                "a touching recording changes nothing",
                vec![recorded(60, 70)],
                vec![(0, 60)],
            ),
            ("no recording changes nothing", vec![], vec![(0, 60)]),
        ];
        for (name, recordings, expected) in cases {
            let found = pieces_of(&[stored("s1", 1, &[(0, 60)])], &recordings);

            assert_eq!(minutes(&found), expected, "{name}");
        }
    }

    #[test]
    fn each_piece_has_its_own_id_from_its_times() {
        let found = pieces_of(&[stored("s1", 1, &[(0, 60)])], &[recorded(20, 40)]);

        let ids: Vec<&str> = found.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                format!("stored:s1:0:{}", 20 * MIN),
                format!("stored:s1:{}:{}", 40 * MIN, 60 * MIN),
            ]
        );
        assert!(found.iter().all(|s| s.stored_id.as_deref() == Some("s1")));
    }

    #[test]
    fn a_piece_the_cut_leaves_under_the_minimum_is_dropped() {
        for (recorded_from, expected) in [(3, vec![]), (4, vec![]), (5, vec![(0, 5)])] {
            let found = pieces_of(
                &[stored("s1", 1, &[(0, 60)])],
                &[recorded(recorded_from, 60)],
            );

            assert_eq!(
                minutes(&found),
                expected,
                "recorded from minute {recorded_from}"
            );
        }
    }

    #[test]
    fn a_whole_span_is_kept_at_any_length() {
        for (end, expected) in [(1, vec![(0, 1)]), (4, vec![(0, 4)]), (5, vec![(0, 5)])] {
            let found = pieces_of(&[stored("s1", 1, &[(0, end)])], &[recorded(100, 110)]);

            assert_eq!(minutes(&found), expected, "{end} min");
        }
    }

    #[test]
    fn every_span_of_a_suggestion_is_a_piece_by_start() {
        let found = pieces_of(&[stored("s1", 1, &[(0, 10), (30, 40)])], &[recorded(5, 8)]);

        assert_eq!(minutes(&found), [(0, 5), (30, 40)]);
    }

    #[test]
    fn the_earlier_stored_suggestion_keeps_the_time_it_shares() {
        let first = stored("a", 1, &[(0, 30)]);
        let second = stored("b", 2, &[(20, 50)]);

        for (order, input) in [
            ("oldest first", [first.clone(), second.clone()]),
            ("newest first", [second, first]),
        ] {
            let found = pieces_of(&input, &[]);

            assert_eq!(
                owners_and_minutes(&found),
                [("a", 0, 30), ("b", 30, 50)],
                "{order}"
            );
        }
    }

    #[test]
    fn suggestions_created_at_the_same_time_are_ordered_by_id() {
        let found = pieces_of(
            &[stored("b", 5, &[(0, 30)]), stored("a", 5, &[(0, 30)])],
            &[],
        );

        assert_eq!(owners_and_minutes(&found), [("a", 0, 30)]);
    }

    #[test]
    fn a_remainder_under_the_minimum_after_an_earlier_suggestion_is_dropped() {
        let found = pieces_of(
            &[stored("a", 1, &[(0, 28)]), stored("b", 2, &[(20, 30)])],
            &[],
        );

        assert_eq!(owners_and_minutes(&found), [("a", 0, 28)]);
    }

    #[test]
    fn pieces_of_different_suggestions_are_sorted_by_start() {
        let found = pieces_of(
            &[
                stored("late", 1, &[(40, 60)]),
                stored("early", 2, &[(0, 10)]),
            ],
            &[],
        );

        assert_eq!(minutes(&found), [(0, 10), (40, 60)]);
    }

    #[test]
    fn a_suggestion_of_an_unknown_project_is_left_out_and_takes_no_time() {
        let gone = StoredSuggestion {
            project_id: "gone".into(),
            ..stored("a", 1, &[(0, 30)])
        };

        let found = pieces_of(&[gone, stored("b", 2, &[(0, 30)])], &[]);

        assert_eq!(owners_and_minutes(&found), [("b", 0, 30)]);
    }

    #[test]
    fn a_deleted_tag_is_dropped_from_the_piece() {
        let tagged = StoredSuggestion {
            tag_id: Some("gone".into()),
            ..stored("s1", 1, &[(0, 30)])
        };

        let found = pieces_of(&[tagged], &[]);

        assert_eq!(
            (&found[0].tag_id, &found[0].tag, &found[0].tag_emoji),
            (&None, &None, &None)
        );
    }

    #[test]
    fn no_suggestions_make_no_pieces() {
        assert!(pieces_of(&[], &[recorded(0, 10)]).is_empty());
    }
}
