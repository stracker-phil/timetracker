//! Fixtures for the rules tests: activity blocks, rules, names and a settings value that leaves blocks raw.

use std::collections::HashMap;

use super::store;
use super::types::{Condition, ConditionInput, Rule, RuleField, RuleInput, RuleOp};
use crate::day::{ActivityBlock, Names, ProjectRef, RecordingBlock};
use crate::db::Db;
use crate::db::time::{Day, Ms};
use crate::error::AppError;
use crate::store::settings::{Continuity, Noise, Settings};

pub const MIN: Ms = 60_000;
pub const HOUR: Ms = 60 * MIN;

pub type Spec<'a> = (RuleField, RuleOp, &'a str);

/// A tracker block of `app` from minute `start` to minute `end` of the day, without url or path.
pub fn block(start: i64, end: i64, app: &str, title: &str) -> ActivityBlock {
    ActivityBlock {
        id: format!("a{start}"),
        start: start * MIN,
        end: end * MIN,
        app: app.into(),
        bundle_id: None,
        app_key: app.into(),
        title: title.into(),
        label: app.into(),
        url: None,
        path: None,
        source: "tracker".into(),
        covered: false,
        members: vec![start],
        col: 0,
        cols: 1,
        private: false,
    }
}

pub fn with_url(block: ActivityBlock, url: &str) -> ActivityBlock {
    ActivityBlock {
        url: Some(url.into()),
        ..block
    }
}

pub fn with_path(block: ActivityBlock, path: &str) -> ActivityBlock {
    ActivityBlock {
        path: Some(path.into()),
        ..block
    }
}

pub fn private(block: ActivityBlock) -> ActivityBlock {
    ActivityBlock {
        private: true,
        ..block
    }
}

/// An enabled rule at position 0 named `Rule <id>`.
pub fn rule(id: &str, project_id: &str, conditions: &[Spec]) -> Rule {
    Rule {
        id: id.into(),
        name: format!("Rule {id}"),
        project_id: project_id.into(),
        position: 0,
        enabled: true,
        note: String::new(),
        tag_id: None,
        conditions: conditions
            .iter()
            .enumerate()
            .map(|(i, &(field, op, value))| Condition {
                id: format!("{id}-c{i}"),
                field,
                op,
                value: value.into(),
            })
            .collect(),
    }
}

pub fn app_is(id: &str, project_id: &str, app: &str) -> Rule {
    rule(id, project_id, &[(RuleField::App, RuleOp::Is, app)])
}

/// Projects `p1` and `p2` style ids given, named `Project <id>` in blue; tags named `tag <id>`.
pub fn names(projects: &[&str], tags: &[&str]) -> Names {
    Names {
        projects: projects
            .iter()
            .map(|id| {
                let project = ProjectRef {
                    name: format!("Project {id}"),
                    client_name: None,
                    color: "blue".into(),
                    shade: 4,
                };
                (id.to_string(), project)
            })
            .collect::<HashMap<_, _>>(),
        tags: tags
            .iter()
            .map(|id| (id.to_string(), format!("tag {id}")))
            .collect(),
    }
}

/// A recording of project `p1` from minute `start` to minute `end`.
pub fn recorded(start: i64, end: i64) -> RecordingBlock {
    RecordingBlock {
        id: format!("rec{start}"),
        span_id: format!("rec{start}-span"),
        span_count: 1,
        start: start * MIN,
        end: end * MIN,
        project_id: "p1".into(),
        project_name: "Project p1".into(),
        client_name: None,
        color: "blue".into(),
        shade: 4,
        note: String::new(),
        tag: None,
        running: false,
        clipped_start: false,
        clipped_end: false,
    }
}

pub fn rule_input(name: &str, project_id: &str, conditions: &[Spec]) -> RuleInput {
    RuleInput {
        name: name.into(),
        project_id: project_id.into(),
        enabled: true,
        note: String::new(),
        tag_id: None,
        conditions: conditions
            .iter()
            .map(|&(field, op, value)| ConditionInput {
                field,
                op,
                value: value.into(),
            })
            .collect(),
    }
}

/// Saves a new rule on the database.
pub async fn save_rule(db: &Db, name: &str, project_id: &str, conditions: &[Spec<'_>]) -> Rule {
    store::save(db, None, &rule_input(name, project_id, conditions))
        .await
        .expect("save rule")
}

/// `(field, op, value)` of each condition of the rule.
pub fn specs(rule: &Rule) -> Vec<(RuleField, RuleOp, String)> {
    rule.conditions
        .iter()
        .map(|c| (c.field, c.op, c.value.clone()))
        .collect()
}

/// Settings that leave blocks as recorded: no summary lane, no grouping, no noise filter.
pub fn plain_settings(continuity: Continuity) -> Settings {
    let mut settings = Settings::default();
    settings.timeline.summary_lane = false;
    settings.timeline.group_by_app = false;
    settings.timeline.noise = Noise::Off;
    settings.timeline.continuity = continuity;
    settings
}

/// `hour:minute` local time of `day`.
pub fn on(day: Day, hour: i64, minute: i64) -> Ms {
    day.bounds().0 + hour * HOUR + minute * MIN
}

/// The form field of an `InvalidField` error.
pub fn field_of(err: &AppError) -> Option<String> {
    err.field().map(|f| f.path.to_string())
}
