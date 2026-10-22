//! Suggested recordings: each block goes to its first matching rule, a rule's blocks join into runs across
//! short gaps, and what recordings already cover or the owner dismissed is cut away.

use super::matching::{Compiled, matches};
use super::types::Suggestion;
use crate::day::{ActivityBlock, Names, RecordingBlock};
use crate::db::dismissed::Dismissal;
use crate::db::time::Ms;

/// Shorter remainders are not worth a suggestion.
const MIN_SUGGESTION_MS: Ms = 5 * 60_000;

type Span = (Ms, Ms);

/// Sorted, overlapping and touching spans merged.
fn union(mut spans: Vec<Span>) -> Vec<Span> {
    spans.sort_unstable();
    let mut merged: Vec<Span> = Vec::with_capacity(spans.len());
    for (start, end) in spans {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

/// `spans` without `cut`; both sorted and disjoint.
fn subtract(spans: &[Span], cut: &[Span]) -> Vec<Span> {
    let mut rest = Vec::new();
    for &(start, end) in spans {
        let mut from = start;
        for &(cut_start, cut_end) in cut {
            if cut_end <= from || cut_start >= end {
                continue;
            }
            if cut_start > from {
                rest.push((from, cut_start));
            }
            from = from.max(cut_end);
        }
        if from < end {
            rest.push((from, end));
        }
    }
    rest
}

/// A rule's blocks joined while the gap to the next is shorter than `bridge_ms` and no barrier lies in it.
fn runs(own: &[Span], barriers: &[Span], bridge_ms: Ms) -> Vec<Span> {
    let blocked = |from: Ms, to: Ms| {
        barriers
            .iter()
            .any(|&(start, end)| start < to && end > from)
    };
    let mut runs: Vec<Span> = Vec::new();
    for &(start, end) in own {
        match runs.last_mut() {
            Some(last)
                if start <= last.1 || (start - last.1 < bridge_ms && !blocked(last.1, start)) =>
            {
                last.1 = last.1.max(end);
            }
            _ => runs.push((start, end)),
        }
    }
    runs
}

/// Each block's span under its first matching rule, merged per rule (by rule index).
fn matched_spans(blocks: &[&ActivityBlock], rules: &[Compiled]) -> Vec<Vec<Span>> {
    let mut matched: Vec<Vec<Span>> = vec![Vec::new(); rules.len()];
    for block in blocks {
        if let Some(index) = rules.iter().position(|rule| matches(rule, block)) {
            matched[index].push((block.start, block.end));
        }
    }
    matched.into_iter().map(union).collect()
}

/// The activity time each rule matched (by rule index), before runs, recordings and dismissals.
pub(super) fn matched_ms(blocks: &[&ActivityBlock], rules: &[Compiled]) -> Vec<Ms> {
    matched_spans(blocks, rules)
        .iter()
        .map(|spans| spans.iter().map(|(start, end)| end - start).sum())
        .collect()
}

/// Each rule's share of the time: matched time first, then bridged gaps, earlier rules first in each pass.
fn claimed_pieces(own: &[Vec<Span>], bridges: &[Vec<Span>]) -> Vec<Vec<Span>> {
    let mut claimed: Vec<Span> = Vec::new();
    let mut pieces: Vec<Vec<Span>> = vec![Vec::new(); own.len()];
    for pass in [own, bridges] {
        for (index, spans) in pass.iter().enumerate() {
            let free = subtract(spans, &claimed);
            claimed = union(claimed.into_iter().chain(free.iter().copied()).collect());
            pieces[index].extend(free);
        }
    }
    pieces.into_iter().map(union).collect()
}

/// The suggestions of the day, by start. Matched time goes to its rule first; a run's bridged gaps only take
/// time no rule matched, so suggestions never overlap. A rule's dismissed time stays its own and is not
/// suggested again. Tags deleted since the rule was saved are left out.
pub fn suggest(
    blocks: &[&ActivityBlock],
    recordings: &[RecordingBlock],
    barriers: &[Span],
    rules: &[Compiled],
    names: &Names,
    bridge_ms: Ms,
    dismissed: &[Dismissal],
) -> Vec<Suggestion> {
    let own = matched_spans(blocks, rules);
    let bridges: Vec<Vec<Span>> = own
        .iter()
        .map(|spans| subtract(&runs(spans, barriers, bridge_ms), spans))
        .collect();
    let recorded = union(recordings.iter().map(|r| (r.start, r.end)).collect());

    let mut suggestions: Vec<Suggestion> = Vec::new();
    for (rule, pieces) in rules.iter().zip(claimed_pieces(&own, &bridges)) {
        let Some(project) = names.projects.get(&rule.project_id) else {
            continue;
        };
        let tag = rule.tag_id.as_ref().and_then(|id| names.tags.get(id));
        let tag_id = tag.and(rule.tag_id.clone());
        let rule_dismissed = dismissed
            .iter()
            .filter(|d| d.rule_id == rule.rule_id)
            .map(|d| (d.start, d.end));
        let cut = union(recorded.iter().copied().chain(rule_dismissed).collect());
        let spans = subtract(&pieces, &cut);
        suggestions.extend(
            spans
                .into_iter()
                .filter(|(start, end)| end - start >= MIN_SUGGESTION_MS)
                .map(|(start, end)| Suggestion {
                    id: format!("{}:{start}:{end}", rule.rule_id),
                    rule_id: rule.rule_id.clone(),
                    rule_name: rule.rule_name.clone(),
                    project_id: rule.project_id.clone(),
                    project_name: project.name.clone(),
                    client_name: project.client_name.clone(),
                    color: project.color.clone(),
                    shade: project.shade,
                    start,
                    end,
                    duration_ms: end - start,
                    note: rule.note.clone(),
                    tag_id: tag_id.clone(),
                    tag: tag.map(|t| t.name.clone()),
                    tag_emoji: tag.and_then(|t| t.emoji.clone()),
                    next_card: None,
                }),
        );
    }
    suggestions.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.id.cmp(&b.id)));
    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::time::Day;
    use crate::rules::matching::compile;
    use crate::rules::testing::*;
    use crate::rules::types::{Rule, RuleField, RuleOp};

    type Found = Vec<(&'static str, i64, i64)>;

    fn slack(start: i64, end: i64) -> ActivityBlock {
        block(start, end, "Slack", "")
    }

    fn code(start: i64, end: i64) -> ActivityBlock {
        block(start, end, "Code", "")
    }

    fn rules() -> Vec<Rule> {
        vec![
            Rule {
                position: 0,
                ..app_is("r1", "p1", "Slack")
            },
            Rule {
                position: 1,
                ..app_is("r2", "p2", "Code")
            },
        ]
    }

    /// `(rule id, start minute, end minute)` of each suggestion.
    fn suggested(
        blocks: &[ActivityBlock],
        recordings: &[RecordingBlock],
        barriers: &[(i64, i64)],
        bridge_min: i64,
    ) -> Vec<(String, i64, i64)> {
        let blocks: Vec<&ActivityBlock> = blocks.iter().collect();
        let barriers: Vec<(Ms, Ms)> = barriers.iter().map(|&(s, e)| (s * MIN, e * MIN)).collect();
        suggest(
            &blocks,
            recordings,
            &barriers,
            &compile(&rules()),
            &names(&["p1", "p2"], &[]),
            bridge_min * MIN,
            &[],
        )
        .into_iter()
        .map(|s| (s.rule_id, s.start / MIN, s.end / MIN))
        .collect()
    }

    fn expect(found: &Found) -> Vec<(String, i64, i64)> {
        found
            .iter()
            .map(|&(rule, start, end)| (rule.to_string(), start, end))
            .collect()
    }

    #[test]
    fn blocks_of_one_rule_join_into_runs_across_short_gaps() {
        struct Case {
            name: &'static str,
            blocks: Vec<ActivityBlock>,
            barriers: Vec<(i64, i64)>,
            bridge: i64,
            expected: Found,
        }
        let cases = [
            Case {
                name: "touching blocks join without a bridge",
                blocks: vec![slack(0, 10), slack(10, 20)],
                barriers: vec![],
                bridge: 0,
                expected: vec![("r1", 0, 20)],
            },
            Case {
                name: "overlapping blocks join",
                blocks: vec![slack(0, 20), slack(10, 30)],
                barriers: vec![],
                bridge: 0,
                expected: vec![("r1", 0, 30)],
            },
            Case {
                name: "a gap below the bridge joins",
                blocks: vec![slack(0, 10), slack(17, 30)],
                barriers: vec![],
                bridge: 8,
                expected: vec![("r1", 0, 30)],
            },
            Case {
                name: "a gap at the bridge splits",
                blocks: vec![slack(0, 10), slack(18, 30)],
                barriers: vec![],
                bridge: 8,
                expected: vec![("r1", 0, 10), ("r1", 18, 30)],
            },
            Case {
                name: "a gap above the bridge splits",
                blocks: vec![slack(0, 10), slack(30, 40)],
                barriers: vec![],
                bridge: 8,
                expected: vec![("r1", 0, 10), ("r1", 30, 40)],
            },
            Case {
                name: "no bridge keeps any gap",
                blocks: vec![slack(0, 10), slack(11, 30)],
                barriers: vec![],
                bridge: 0,
                expected: vec![("r1", 0, 10), ("r1", 11, 30)],
            },
            Case {
                name: "a sleep in the gap splits",
                blocks: vec![slack(0, 10), slack(15, 30)],
                barriers: vec![(12, 14)],
                bridge: 8,
                expected: vec![("r1", 0, 10), ("r1", 15, 30)],
            },
            Case {
                name: "a sleep outside the gap does not",
                blocks: vec![slack(0, 10), slack(15, 30)],
                barriers: vec![(40, 50), (0, 5)],
                bridge: 8,
                expected: vec![("r1", 0, 30)],
            },
            Case {
                name: "A-B-A with B unmatched is one run",
                blocks: vec![slack(0, 10), block(10, 14, "Mail", ""), slack(14, 30)],
                barriers: vec![],
                bridge: 8,
                expected: vec![("r1", 0, 30)],
            },
            Case {
                name: "A-B-A with B matched by another rule leaves B its time",
                blocks: vec![slack(0, 10), code(10, 18), slack(18, 30)],
                barriers: vec![],
                bridge: 16,
                expected: vec![("r1", 0, 10), ("r2", 10, 18), ("r1", 18, 30)],
            },
            Case {
                name: "the earlier rule wins overlapping time",
                blocks: vec![slack(0, 20), code(10, 30)],
                barriers: vec![],
                bridge: 0,
                expected: vec![("r1", 0, 20), ("r2", 20, 30)],
            },
        ];
        for case in cases {
            assert_eq!(
                suggested(&case.blocks, &[], &case.barriers, case.bridge),
                expect(&case.expected),
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn a_run_shorter_than_the_minimum_yields_nothing() {
        for (end, expected) in [(4, vec![]), (5, vec![("r1", 0, 5)])] {
            assert_eq!(
                suggested(&[slack(0, end)], &[], &[], 0),
                expect(&expected),
                "{end} min"
            );
        }
    }

    #[test]
    fn recordings_are_cut_out_of_a_run() {
        let cases: [(&str, Vec<RecordingBlock>, Found); 8] = [
            (
                "a recording in the middle yields two",
                vec![recorded(20, 40)],
                vec![("r1", 0, 20), ("r1", 40, 60)],
            ),
            (
                "several recordings leave the gaps",
                vec![recorded(10, 20), recorded(30, 50)],
                vec![("r1", 0, 10), ("r1", 20, 30), ("r1", 50, 60)],
            ),
            (
                "a remainder under the minimum at the end is dropped",
                vec![recorded(0, 57)],
                vec![],
            ),
            (
                "a remainder under the minimum at the start is dropped",
                vec![recorded(3, 60)],
                vec![],
            ),
            (
                "a remainder of exactly the minimum is kept",
                vec![recorded(5, 60)],
                vec![("r1", 0, 5)],
            ),
            (
                "a covering recording leaves nothing",
                vec![recorded(-10, 70)],
                vec![],
            ),
            (
                "a recording elsewhere changes nothing",
                vec![recorded(70, 80)],
                vec![("r1", 0, 60)],
            ),
            (
                "a touching recording changes nothing",
                vec![recorded(60, 70)],
                vec![("r1", 0, 60)],
            ),
        ];
        for (name, recordings, expected) in cases {
            assert_eq!(
                suggested(&[slack(0, 60)], &recordings, &[], 0),
                expect(&expected),
                "{name}"
            );
        }
    }

    #[test]
    fn recordings_are_cut_out_of_a_bridged_gap_too() {
        let found = suggested(&[slack(0, 10), slack(20, 30)], &[recorded(12, 18)], &[], 16);

        assert_eq!(found, expect(&vec![("r1", 0, 12), ("r1", 18, 30)]));
    }

    #[test]
    fn suggestions_are_sorted_by_start_across_rules() {
        let found = suggested(&[slack(40, 60), code(0, 10), slack(20, 30)], &[], &[], 0);

        assert_eq!(
            found,
            expect(&vec![("r2", 0, 10), ("r1", 20, 30), ("r1", 40, 60)])
        );
    }

    #[test]
    fn a_suggestion_carries_the_rules_project_note_and_tag() {
        let rule = Rule {
            note: "standup".into(),
            tag_id: Some("t1".into()),
            ..app_is("r1", "p1", "Slack")
        };
        let blocks = [slack(10, 20)];
        let refs: Vec<&ActivityBlock> = blocks.iter().collect();

        let found = suggest(
            &refs,
            &[],
            &[],
            &compile(&[rule]),
            &names(&["p1"], &["t1"]),
            0,
            &[],
        );

        assert_eq!(
            found,
            [Suggestion {
                id: format!("r1:{}:{}", 10 * MIN, 20 * MIN),
                rule_id: "r1".into(),
                rule_name: "Rule r1".into(),
                project_id: "p1".into(),
                project_name: "Project p1".into(),
                client_name: None,
                color: "blue".into(),
                shade: 4,
                start: 10 * MIN,
                end: 20 * MIN,
                duration_ms: 10 * MIN,
                note: "standup".into(),
                tag_id: Some("t1".into()),
                tag: Some("tag t1".into()),
                tag_emoji: None,
                next_card: None,
            }]
        );
    }

    #[test]
    fn a_suggestion_carries_the_emoji_of_the_rules_tag() {
        let rule = Rule {
            tag_id: Some("t1".into()),
            ..app_is("r1", "p1", "Slack")
        };
        let blocks = [slack(10, 20)];
        let refs: Vec<&ActivityBlock> = blocks.iter().collect();
        let mut names = names(&["p1"], &["t1"]);
        names.tags.get_mut("t1").unwrap().emoji = Some("🧪".into());

        let found = suggest(&refs, &[], &[], &compile(&[rule]), &names, 0, &[]);

        assert_eq!(found[0].tag.as_deref(), Some("tag t1"));
        assert_eq!(found[0].tag_emoji.as_deref(), Some("🧪"));
    }

    #[test]
    fn a_suggestion_drops_a_tag_that_no_longer_exists() {
        let rule = Rule {
            tag_id: Some("gone".into()),
            ..app_is("r1", "p1", "Slack")
        };
        let blocks = [slack(10, 20)];
        let refs: Vec<&ActivityBlock> = blocks.iter().collect();

        let found = suggest(
            &refs,
            &[],
            &[],
            &compile(&[rule]),
            &names(&["p1"], &["t1"]),
            0,
            &[],
        );

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].tag_id, None);
    }

    #[test]
    fn ids_are_the_same_on_every_build() {
        let blocks = [slack(0, 30), code(40, 60)];
        let refs: Vec<&ActivityBlock> = blocks.iter().collect();
        let build = || {
            suggest(
                &refs,
                &[],
                &[],
                &compile(&rules()),
                &names(&["p1", "p2"], &[]),
                0,
                &[],
            )
        };

        let ids: Vec<String> = build().into_iter().map(|s| s.id).collect();

        assert_eq!(
            ids,
            [
                format!("r1:{}:{}", 0, 30 * MIN),
                format!("r2:{}:{}", 40 * MIN, 60 * MIN)
            ]
        );
        assert_eq!(build(), build());
    }

    #[test]
    fn a_rule_whose_project_is_gone_suggests_nothing() {
        let blocks = [block(0, 30, "Mail", "")];
        let refs: Vec<&ActivityBlock> = blocks.iter().collect();
        let rules = [app_is("r3", "gone", "Mail")];

        let found = suggest(
            &refs,
            &[],
            &[],
            &compile(&rules),
            &names(&["p1"], &[]),
            0,
            &[],
        );

        assert!(found.is_empty());
    }

    /// `(rule id, start minute, end minute)` of each suggestion with `(rule id, start, end)` minutes dismissed.
    fn suggested_but_dismissed(
        blocks: &[ActivityBlock],
        dismissed: &[(&str, i64, i64)],
    ) -> Vec<(String, i64, i64)> {
        let blocks: Vec<&ActivityBlock> = blocks.iter().collect();
        let dismissed: Vec<Dismissal> = dismissed
            .iter()
            .map(|&(rule_id, start, end)| Dismissal {
                rule_id: rule_id.into(),
                day: Day::parse("2026-01-05").unwrap(),
                start: start * MIN,
                end: end * MIN,
            })
            .collect();
        suggest(
            &blocks,
            &[],
            &[],
            &compile(&rules()),
            &names(&["p1", "p2"], &[]),
            0,
            &dismissed,
        )
        .into_iter()
        .map(|s| (s.rule_id, s.start / MIN, s.end / MIN))
        .collect()
    }

    #[test]
    fn a_dismissed_span_is_not_suggested_again() {
        struct Case {
            name: &'static str,
            blocks: Vec<ActivityBlock>,
            dismissed: Vec<(&'static str, i64, i64)>,
            expected: Found,
        }
        let cases = [
            Case {
                name: "the rule's dismissed span is dropped and its other spans stay",
                blocks: vec![slack(0, 10), slack(30, 40)],
                dismissed: vec![("r1", 0, 10)],
                expected: vec![("r1", 30, 40)],
            },
            Case {
                name: "a dismissal in the middle of a run leaves both sides",
                blocks: vec![slack(0, 60)],
                dismissed: vec![("r1", 20, 40)],
                expected: vec![("r1", 0, 20), ("r1", 40, 60)],
            },
            Case {
                name: "several dismissals leave what is between them",
                blocks: vec![slack(0, 60)],
                dismissed: vec![("r1", 0, 10), ("r1", 50, 60)],
                expected: vec![("r1", 10, 50)],
            },
            Case {
                name: "another rule's dismissal changes nothing",
                blocks: vec![slack(0, 10)],
                dismissed: vec![("r2", 0, 10)],
                expected: vec![("r1", 0, 10)],
            },
            Case {
                name: "a dismissal next to the span changes nothing",
                blocks: vec![slack(0, 10)],
                dismissed: vec![("r1", 10, 20)],
                expected: vec![("r1", 0, 10)],
            },
            Case {
                name: "dismissed time stays its rule's and is not handed to the next rule",
                blocks: vec![slack(0, 10), code(5, 15)],
                dismissed: vec![("r1", 0, 10)],
                expected: vec![("r2", 10, 15)],
            },
        ];
        for case in cases {
            assert_eq!(
                suggested_but_dismissed(&case.blocks, &case.dismissed),
                expect(&case.expected),
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn what_a_dismissal_leaves_under_the_minimum_is_dropped() {
        for (dismissed_end, expected) in [(57, vec![]), (50, vec![("r1", 50, 60)])] {
            assert_eq!(
                suggested_but_dismissed(&[slack(0, 60)], &[("r1", 0, dismissed_end)]),
                expect(&expected),
                "dismissed until minute {dismissed_end}"
            );
        }
    }

    #[test]
    fn matched_time_counts_each_block_for_its_first_matching_rule_only() {
        let blocks = [
            slack(0, 10),
            slack(5, 20),
            code(30, 33),
            block(40, 50, "Mail", ""),
        ];
        let refs: Vec<&ActivityBlock> = blocks.iter().collect();
        let rules = [
            app_is("r1", "p1", "Slack"),
            rule("r2", "p2", &[(RuleField::App, RuleOp::Contains, "o")]),
            app_is("r3", "p1", "Code"),
        ];

        let matched = matched_ms(&refs, &compile(&rules));

        assert_eq!(matched, [20 * MIN, 3 * MIN, 0]);
    }

    #[test]
    fn private_blocks_and_conditionless_rules_suggest_nothing() {
        let blocks = [private(slack(0, 30)), block(0, 30, "Mail", "")];
        let refs: Vec<&ActivityBlock> = blocks.iter().collect();
        let rules = [app_is("r1", "p1", "Slack"), rule("r2", "p1", &[])];

        let found = suggest(
            &refs,
            &[],
            &[],
            &compile(&rules),
            &names(&["p1"], &[]),
            0,
            &[],
        );

        assert!(found.is_empty());
    }
}
