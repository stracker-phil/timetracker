//! The project the editor proposes for the spans of a new recording, from the activity they cover.

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

/// The app with the most time inside the spans; private blocks do not count.
pub fn main_app(blocks: &[&ActivityBlock], spans: &[Span]) -> Option<String> {
    most_time(
        blocks
            .iter()
            .filter(|b| !b.private)
            .map(|b| (b.app.as_str(), inside(b.start, b.end, spans)))
            .filter(|&(_, ms)| ms > 0),
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
        return Ok(RecordingProposal { project_id: None });
    };
    let mut views = Vec::new();
    for day in Day::range(&Day::of(first), &Day::of(last - 1)) {
        views.push(crate::day::view_of(db, day, settings, now).await?);
    }
    let blocks: Vec<&ActivityBlock> = views.iter().flat_map(crate::rules::candidates).collect();
    let open: Vec<String> = projects::list(db, false)
        .await?
        .into_iter()
        .map(|p| p.id)
        .collect();
    let usable = |id: Option<String>| id.filter(|id| open.contains(id));

    let mut project_id = None;
    if let Some(app) = main_app(&blocks, spans) {
        project_id = usable(earlier_project(db, &app, first).await?);
    }
    if project_id.is_none() {
        let rules = compile(&crate::rules::store::active(db).await?);
        project_id = usable(rule_project(&blocks, spans, &rules));
    }
    Ok(RecordingProposal { project_id })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::recordings::RecordingSpan;
    use crate::recordings::testing::{MIN, World, at, row, span};
    use crate::rules::testing::{block, plain_settings, private, save_rule};
    use crate::rules::types::{RuleField, RuleOp};
    use crate::store::settings::Continuity;
    use crate::test_support::activity;

    fn minutes(start: i64, end: i64) -> Span {
        span(start * MIN, end * MIN)
    }

    #[test]
    fn main_app_is_the_app_with_the_most_time_inside_the_spans() {
        let slack_a = block(0, 20, "Slack", "");
        let mail = block(20, 70, "Mail", "");
        let slack_b = block(70, 120, "Slack", "");

        let found = main_app(
            &[&slack_a, &mail, &slack_b],
            &[minutes(10, 30), minutes(80, 110)],
        );

        assert_eq!(found.as_deref(), Some("Slack"));
    }

    #[test]
    fn main_app_counts_only_the_time_inside_the_spans() {
        let long = block(0, 100, "Mail", "");
        let short = block(40, 60, "Slack", "");
        let found = main_app(&[&long, &short], &[minutes(0, 10), minutes(40, 60)]);

        assert_eq!(found.as_deref(), Some("Mail"));
    }

    #[test]
    fn main_app_ignores_private_blocks() {
        let hidden = private(block(0, 50, "Slack", ""));
        let shown = block(0, 10, "Mail", "");

        let found = main_app(&[&hidden, &shown], &[minutes(0, 50)]);

        assert_eq!(found.as_deref(), Some("Mail"));
        assert_eq!(main_app(&[&hidden], &[minutes(0, 50)]), None);
    }

    #[test]
    fn main_app_skips_blocks_without_time_inside_the_spans() {
        let outside = block(100, 200, "Slack", "");
        let touching = block(50, 60, "Figma", "");
        let inside = block(0, 50, "Mail", "");

        let found = main_app(&[&outside, &touching, &inside], &[minutes(0, 50)]);

        assert_eq!(found.as_deref(), Some("Mail"));
        assert_eq!(main_app(&[&outside, &touching], &[minutes(0, 50)]), None);
        assert_eq!(main_app(&[&inside], &[]), None);
    }

    #[test]
    fn main_app_of_equal_times_is_the_smallest_name() {
        let mail = block(0, 10, "Mail", "");
        let slack = block(10, 20, "Slack", "");
        let figma = block(20, 30, "Figma", "");

        let found = main_app(&[&mail, &slack, &figma], &[minutes(0, 30)]);

        assert_eq!(found.as_deref(), Some("Figma"));
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
    async fn propose_gives_no_project_without_a_recording_or_rule() {
        let w = World::new().await;
        slack_before_and_during(&w).await;

        let proposal = propose_for(&w, at(10, 0), at(11, 0)).await;

        assert_eq!(proposal.project_id, None);
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

        assert_eq!(proposal, RecordingProposal { project_id: None });
    }
}
