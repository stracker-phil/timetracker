use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::day::ActivityBlock;
use crate::db::time::{Day, Ms};

/// What part of an activity block a condition reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum RuleField {
    App,
    Title,
    Url,
    /// The host of the URL without a leading `www.`.
    Domain,
    Path,
    Source,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum RuleOp {
    Contains,
    Is,
    BeginsWith,
    /// A glob anchored at both ends: `*` any run of characters, `?` one.
    Pattern,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Condition {
    pub id: String,
    pub field: RuleField,
    pub op: RuleOp,
    pub value: String,
}

/// Activity a rule's conditions all match belongs to its project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub project_id: String,
    /// The first matching rule by position wins.
    pub position: i64,
    pub enabled: bool,
    pub note: String,
    pub tag_ids: Vec<String>,
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ConditionInput {
    pub field: RuleField,
    pub op: RuleOp,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct RuleInput {
    pub name: String,
    pub project_id: String,
    pub enabled: bool,
    pub note: String,
    pub tag_ids: Vec<String>,
    pub conditions: Vec<ConditionInput>,
}

/// A recording a rule proposes for time no recording covers yet.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Suggestion {
    /// `<rule id>:<start>:<end>`: the same after a reload.
    pub id: String,
    pub rule_id: String,
    pub rule_name: String,
    pub project_id: String,
    pub project_name: String,
    pub color: String,
    pub shade: u8,
    pub start: Ms,
    pub end: Ms,
    pub note: String,
    pub tag_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct PreviewHit {
    pub block: ActivityBlock,
    pub day: Day,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct RulePreview {
    /// Most recent first, at most `PREVIEW_HITS`.
    pub hits: Vec<PreviewHit>,
    /// Every matching block's time, also those beyond the listed hits.
    pub total_ms: i64,
    /// The days with at least one hit.
    pub days: i64,
}
