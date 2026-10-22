//! What the editor shows for the spans of a new or changed recording: the activity they cover and the
//! project to propose for it.

use std::collections::HashMap;
use std::hash::Hash;

use serde::Serialize;
use ts_rs::TS;

use super::{MINUTE_MS, Span};
use crate::day::ActivityBlock;
use crate::db::recordings::Recording;
use crate::db::time::{Day, Ms};
use crate::db::{Db, activities, projects, recordings};
use crate::error::AppResult;
use crate::rules::matching::{Compiled, compile, first_match};
use crate::store::settings::Settings;

/// How far back the last recording of the same app is looked for.
const LOOKBACK_MS: Ms = 14 * 24 * 60 * MINUTE_MS;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct RecordingProposal {
    /// The project of the last recording that covered the same app, else of the rule that matches most of
    /// the activity; `None` without either.
    pub project_id: Option<String>,
    /// The activity blocks sharing time with the spans, by start.
    pub activity: Vec<CoveredActivity>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct CoveredActivity {
    /// The block's id on the Day view.
    pub id: String,
    pub label: String,
    pub app: String,
    pub bundle_id: Option<String>,
    pub start: Ms,
    pub end: Ms,
    /// The part of it inside the spans.
    pub inside_ms: i64,
    pub private: bool,
}

/// The time `[start, end)` shares with `spans`.
fn inside(start: Ms, end: Ms, spans: &[Span]) -> i64 {
    spans
        .iter()
        .map(|s| (end.min(s.end) - start.max(s.start)).max(0))
        .sum()
}

/// The key with the most time summed over `timed`; equal times go to the smallest key.
fn most_time<K: Ord + Hash>(timed: impl IntoIterator<Item = (K, i64)>) -> Option<K> {
    let mut sums: HashMap<K, i64> = HashMap::new();
    for (key, ms) in timed {
        *sums.entry(key).or_default() += ms;
    }
    sums.into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|(key, _)| key)
}

/// The blocks of `blocks` inside `spans`, by start.
pub fn covered(blocks: &[&ActivityBlock], spans: &[Span]) -> Vec<CoveredActivity> {
    let mut found: Vec<CoveredActivity> = blocks
        .iter()
        .filter_map(|b| {
            let inside_ms = inside(b.start, b.end, spans);
            (inside_ms > 0).then(|| CoveredActivity {
                id: b.id.clone(),
                label: b.label.clone(),
                app: b.app.clone(),
                bundle_id: b.bundle_id.clone(),
                start: b.start,
                end: b.end,
                inside_ms,
                private: b.private,
            })
        })
        .collect();
    found.sort_by_key(|a| (a.start, a.end));
    found
}

/// The app with the most time inside the spans; private blocks do not count.
pub fn main_app(activity: &[CoveredActivity]) -> Option<String> {
    most_time(
        activity
            .iter()
            .filter(|a| !a.private)
            .map(|a| (a.app.as_str(), a.inside_ms)),
    )
    .map(str::to_string)
}

/// The project of the latest span in `earlier` that shares time with one of `seen`.
pub fn last_covering(earlier: &[Recording], seen: &[Span]) -> Option<String> {
    earlier
        .iter()
        .flat_map(|r| r.spans.iter().map(move |s| (r, s)))
        .filter_map(|(r, s)| s.end.map(|end| (r, s.start, end)))
        .filter(|&(_, start, end)| seen.iter().any(|a| a.start < end && a.end > start))
        .max_by_key(|&(_, start, _)| start)
        .map(|(r, _, _)| r.project_id.clone())
}

/// The project of the rule that matches the most time inside the spans.
fn rule_project(blocks: &[&ActivityBlock], spans: &[Span], rules: &[Compiled]) -> Option<String> {
    most_time(blocks.iter().filter_map(|block| {
        let inside_ms = inside(block.start, block.end, spans);
        if inside_ms == 0 {
            return None;
        }
        first_match(rules, block).map(|rule| (rule.project_id.as_str(), inside_ms))
    }))
    .map(str::to_string)
}

/// The project of the latest recording in the lookback before `first` that covered `app`'s activity.
async fn earlier_project(db: &Db, app: &str, first: Ms) -> AppResult<Option<String>> {
    let from = first - LOOKBACK_MS;
    let seen: Vec<Span> = activities::list_for_range(db, from, first)
        .await?
        .into_iter()
        .filter(|row| row.app == app && !row.private)
        .map(|row| Span {
            start: row.start,
            end: row.end.unwrap_or(first),
        })
        .collect();
    let earlier = recordings::list_for_range(db, from, first).await?;
    Ok(last_covering(&earlier, &seen))
}

pub async fn propose(
    db: &Db,
    spans: &[Span],
    settings: &Settings,
    now: Ms,
) -> AppResult<RecordingProposal> {
    let (Some(first), Some(last)) = (
        spans.iter().map(|s| s.start).min(),
        spans.iter().map(|s| s.end).max(),
    ) else {
        return Ok(RecordingProposal {
            project_id: None,
            activity: Vec::new(),
        });
    };
    let mut views = Vec::new();
    for day in Day::range(&Day::of(first), &Day::of(last - 1)) {
        views.push(crate::day::view_of(db, day, settings, now).await?);
    }
    let blocks: Vec<&ActivityBlock> = views.iter().flat_map(crate::rules::candidates).collect();
    let activity = covered(&blocks, spans);
    let open: Vec<String> = projects::list(db, false)
        .await?
        .into_iter()
        .map(|p| p.id)
        .collect();
    let usable = |id: Option<String>| id.filter(|id| open.contains(id));

    let mut project_id = None;
    if let Some(app) = main_app(&activity) {
        project_id = usable(earlier_project(db, &app, first).await?);
    }
    if project_id.is_none() {
        let rules = compile(&crate::rules::store::active(db).await?);
        project_id = usable(rule_project(&blocks, spans, &rules));
    }
    Ok(RecordingProposal {
        project_id,
        activity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::recordings::RecordingSpan;
    use crate::recordings::testing::{HOUR, MIN, World, at, row, span};
    use crate::rules::testing::{block, plain_settings, private, save_rule};
    use crate::rules::types::{RuleField, RuleOp};
    use crate::store::settings::Continuity;
    use crate::test_support::activity;

    fn minutes(start: i64, end: i64) -> Span {
        span(start * MIN, end * MIN)
    }

    fn covered_block(id: &str, app: &str, start: i64, end: i64, inside: i64) -> CoveredActivity {
        CoveredActivity {
            id: id.into(),
            label: app.into(),
            app: app.into(),
            bundle_id: None,
            start: start * MIN,
            end: end * MIN,
            inside_ms: inside * MIN,
            private: false,
        }
    }

    fn used(app: &str, inside: i64) -> CoveredActivity {
        covered_block("x", app, 0, inside, inside)
    }

    #[test]
    fn covered_keeps_only_blocks_sharing_time_with_the_spans_and_sorts_them_by_start() {
        let late = block(40, 50, "Mail", "");
        let far = block(100, 110, "Slack", "");
        let early = block(0, 10, "Slack", "");
        let touching = block(45, 60, "Slack", "");

        let found = covered(&[&late, &far, &early, &touching], &[minutes(5, 45)]);

        assert_eq!(
            found,
            [
                covered_block("a0", "Slack", 0, 10, 5),
                covered_block("a40", "Mail", 40, 50, 5),
            ]
        );
    }

    #[test]
    fn covered_adds_up_the_time_a_block_shares_with_every_span() {
        let long = block(0, 60, "Slack", "");

        let found = covered(
            &[&long],
            &[minutes(10, 20), minutes(30, 35), minutes(70, 80)],
        );

        assert_eq!(found, [covered_block("a0", "Slack", 0, 60, 15)]);
    }

    #[test]
    fn covered_of_no_spans_is_empty() {
        let any = block(0, 10, "Slack", "");

        assert_eq!(covered(&[&any], &[]), Vec::<CoveredActivity>::new());
    }

    #[test]
    fn covered_keeps_the_private_flag() {
        let hidden = private(block(0, 10, "Slack", ""));

        let found = covered(&[&hidden], &[minutes(0, 10)]);

        assert_eq!(
            found,
            [CoveredActivity {
                private: true,
                ..covered_block("a0", "Slack", 0, 10, 10)
            }]
        );
    }

    #[test]
    fn main_app_is_the_app_with_the_most_time_summed_over_its_blocks() {
        let activity = [used("Slack", 20), used("Mail", 30), used("Slack", 20)];

        assert_eq!(main_app(&activity).as_deref(), Some("Slack"));
    }

    #[test]
    fn main_app_ignores_private_blocks() {
        let activity = [
            CoveredActivity {
                private: true,
                ..used("Slack", 50)
            },
            used("Mail", 10),
        ];

        assert_eq!(main_app(&activity).as_deref(), Some("Mail"));
    }

    #[test]
    fn main_app_is_none_without_activity_or_with_only_private_activity() {
        let hidden = CoveredActivity {
            private: true,
            ..used("Slack", 50)
        };

        assert_eq!(main_app(&[]), None);
        assert_eq!(main_app(&[hidden]), None);
    }

    #[test]
    fn main_app_of_equal_times_is_the_first_by_name() {
        let activity = [used("Mail", 10), used("Slack", 10), used("Figma", 10)];

        assert_eq!(main_app(&activity).as_deref(), Some("Figma"));
    }

    fn recorded_for(project: &str, start: i64, end: i64) -> Recording {
        Recording {
            project_id: project.into(),
            ..row(&format!("{project}-{start}"), start * MIN, end * MIN)
        }
    }

    #[test]
    fn last_covering_is_the_project_of_the_latest_span_sharing_time_with_the_activity() {
        let recordings = [
            recorded_for("old", 0, 30),
            recorded_for("late", 60, 90),
            recorded_for("mid", 30, 60),
        ];

        let found = last_covering(&recordings, &[span(10 * MIN, 70 * MIN)]);

        assert_eq!(found.as_deref(), Some("late"));
    }

    #[test]
    fn last_covering_looks_at_every_interval_of_the_activity() {
        let recordings = [recorded_for("a", 0, 30), recorded_for("b", 100, 130)];

        let found = last_covering(
            &recordings,
            &[span(10 * MIN, 20 * MIN), span(200 * MIN, 210 * MIN)],
        );

        assert_eq!(found.as_deref(), Some("a"));
    }

    #[test]
    fn last_covering_ignores_open_spans() {
        let running = Recording {
            spans: vec![RecordingSpan::new(50 * MIN, None)],
            ..recorded_for("running", 0, 1)
        };
        let recordings = [recorded_for("done", 0, 30), running];

        let found = last_covering(&recordings, &[span(10 * MIN, 70 * MIN)]);

        assert_eq!(found.as_deref(), Some("done"));
    }

    #[test]
    fn last_covering_is_none_without_overlap_and_touching_does_not_count() {
        let recordings = [recorded_for("a", 0, 30), recorded_for("b", 60, 90)];

        assert_eq!(
            last_covering(&recordings, &[span(30 * MIN, 60 * MIN)]),
            None
        );
        assert_eq!(
            last_covering(&recordings, &[span(200 * MIN, 210 * MIN)]),
            None
        );
        assert_eq!(last_covering(&recordings, &[]), None);
        assert_eq!(last_covering(&[], &[span(0, 10 * MIN)]), None);
    }

    fn settings() -> Settings {
        plain_settings(Continuity::Off)
    }

    async fn propose_for(w: &World, start: Ms, end: Ms) -> RecordingProposal {
        propose(&w.db, &[span(start, end)], &settings(), at(20, 0))
            .await
            .unwrap()
    }

    async fn slack_before_and_during(w: &World) {
        activity(&w.db, at(8, 0), Some(at(9, 0)), "Slack", "").await;
        activity(&w.db, at(10, 0), Some(at(11, 0)), "Slack", "").await;
    }

    fn apps(proposal: &RecordingProposal) -> Vec<(&str, i64)> {
        proposal
            .activity
            .iter()
            .map(|a| (a.app.as_str(), a.inside_ms))
            .collect()
    }

    #[tokio::test]
    async fn propose_gives_the_project_of_an_earlier_recording_over_the_same_app() {
        let w = World::new().await;
        slack_before_and_during(&w).await;
        w.create(at(8, 0), at(9, 0)).await;
        save_rule(
            &w.db,
            "Slack work",
            &w.other.id,
            &[(RuleField::App, RuleOp::Is, "Slack")],
        )
        .await;

        let proposal = propose_for(&w, at(10, 0), at(11, 0)).await;

        assert_eq!(proposal.project_id.as_deref(), Some(w.project.id.as_str()));
        assert_eq!(apps(&proposal), [("Slack", HOUR)]);
    }

    #[tokio::test]
    async fn propose_falls_back_to_the_rule_project_without_an_earlier_recording() {
        let w = World::new().await;
        slack_before_and_during(&w).await;
        save_rule(
            &w.db,
            "Slack work",
            &w.other.id,
            &[(RuleField::App, RuleOp::Is, "Slack")],
        )
        .await;

        let proposal = propose_for(&w, at(10, 0), at(11, 0)).await;

        assert_eq!(proposal.project_id.as_deref(), Some(w.other.id.as_str()));
    }

    #[tokio::test]
    async fn propose_gives_no_project_without_a_recording_or_rule_but_still_lists_the_activity() {
        let w = World::new().await;
        slack_before_and_during(&w).await;

        let proposal = propose_for(&w, at(10, 0), at(11, 0)).await;

        assert_eq!(proposal.project_id, None);
        assert_eq!(apps(&proposal), [("Slack", HOUR)]);
    }

    #[tokio::test]
    async fn propose_ignores_earlier_recordings_over_another_app() {
        let w = World::new().await;
        activity(&w.db, at(8, 0), Some(at(9, 0)), "Mail", "").await;
        activity(&w.db, at(10, 0), Some(at(11, 0)), "Slack", "").await;
        w.create(at(8, 0), at(9, 0)).await;

        let proposal = propose_for(&w, at(10, 0), at(11, 0)).await;

        assert_eq!(proposal.project_id, None);
    }

    #[tokio::test]
    async fn propose_never_gives_an_archived_project() {
        let w = World::new().await;
        slack_before_and_during(&w).await;
        w.create(at(8, 0), at(9, 0)).await;
        projects::set_archived(&w.db, &w.project.id, true)
            .await
            .unwrap();

        let proposal = propose_for(&w, at(10, 0), at(11, 0)).await;

        assert_eq!(proposal.project_id, None);
    }

    #[tokio::test]
    async fn propose_uses_the_rule_project_when_the_earlier_project_is_archived() {
        let w = World::new().await;
        slack_before_and_during(&w).await;
        w.create(at(8, 0), at(9, 0)).await;
        save_rule(
            &w.db,
            "Slack work",
            &w.other.id,
            &[(RuleField::App, RuleOp::Is, "Slack")],
        )
        .await;
        projects::set_archived(&w.db, &w.project.id, true)
            .await
            .unwrap();

        let proposal = propose_for(&w, at(10, 0), at(11, 0)).await;

        assert_eq!(proposal.project_id.as_deref(), Some(w.other.id.as_str()));
    }

    #[tokio::test]
    async fn propose_for_no_spans_is_empty() {
        let w = World::new().await;
        slack_before_and_during(&w).await;

        let proposal = propose(&w.db, &[], &settings(), at(20, 0)).await.unwrap();

        assert_eq!(
            proposal,
            RecordingProposal {
                project_id: None,
                activity: Vec::new()
            }
        );
    }
}
