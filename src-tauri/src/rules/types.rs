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
    pub tag_id: Option<String>,
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
    pub tag_id: Option<String>,
    pub conditions: Vec<ConditionInput>,
}

/// A recording proposed for time no recording covers yet, by rules or stored by another app; it becomes one
/// only once approved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Suggestion {
    /// `<rule id>:<start>:<end>`, or `stored:<stored id>:<start>:<end>`: the same after a reload.
    pub id: String,
    /// The rules whose touching pieces it joins, in position order; they share project, note and tag. Empty
    /// for a stored one.
    pub rule_ids: Vec<String>,
    /// The stored suggestion it shows a piece of.
    pub stored_id: Option<String>,
    /// Who suggests it: the rules' names, comma separated, or the stored suggestion's source.
    pub by: String,
    /// None for a stored one waiting for the owner to pick it (ADR 0030); not approvable until then.
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub client_name: Option<String>,
    /// The project's accent, `graphite` without one.
    pub color: String,
    pub shade: u8,
    pub start: Ms,
    pub end: Ms,
    pub duration_ms: Ms,
    pub note: String,
    pub tag_id: Option<String>,
    /// The tag's name.
    pub tag: Option<String>,
    pub tag_emoji: Option<String>,
    /// The first timesheet card that starts after it, which its card goes before; none puts it last.
    pub next_card: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct PreviewHit {
    pub block: ActivityBlock,
    pub day: Day,
    /// `Wed, Oct 21`, the short day title.
    pub day_label: String,
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

/// An active rule's day: the activity it matched and its suggestions waiting for review.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct RuleDay {
    pub rule_id: String,
    /// Matched activity, the first matching rule taking a block.
    pub matched_ms: Ms,
    pub pending: u32,
    pub pending_ms: Ms,
}
