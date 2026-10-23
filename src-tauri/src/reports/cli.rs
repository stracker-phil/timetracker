//! `timetracker report`: the filter with clients, projects and tags by name, resolved in the app.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::{Range, ReportFilter, ReportResult, billable_text, run, text};
use crate::db::time::{Day, now_ms};
use crate::db::{Db, clients, projects, tags};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::store::settings::{Group, Rounding};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportRequest {
    pub range: Range,
    pub group_by: Group,
    pub rounding: Rounding,
    /// Ids, names or unique starts of names.
    pub clients: Vec<String>,
    pub projects: Vec<String>,
    pub tags: Vec<String>,
}

/// What the CLI prints without `--json`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReportOutput {
    /// The copied text, then the billable line.
    #[default]
    Text,
    /// The notes, one per line (`--notes`).
    Notes,
}

impl ReportOutput {
    pub fn is_text(&self) -> bool {
        *self == Self::Text
    }
}

/// The result with the text the CLI prints without `--json`; on the wire the period (`from`, `to`,
/// `title`) comes first and the long `text` last, so the JSON reads from the top.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reported {
    #[serde(flatten)]
    pub result: serde_json::Value,
    pub text: String,
}

/// `result`'s fields with `first` in front, the rest in their order.
fn leading(result: serde_json::Value, first: &[&str]) -> serde_json::Value {
    let serde_json::Value::Object(mut fields) = result else {
        return result;
    };
    let mut ordered = serde_json::Map::new();
    for key in first {
        if let Some(value) = fields.shift_remove(*key) {
            ordered.insert((*key).to_string(), value);
        }
    }
    ordered.extend(fields);
    serde_json::Value::Object(ordered)
}

pub async fn report(
    app: &AppHandle,
    request: ReportRequest,
    output: ReportOutput,
) -> AppResult<Reported> {
    let db = app.state::<AppState>().data.db()?;
    let filter = filter_of(&db, request).await?;
    reported(run(&db, &filter, Day::today(), now_ms()).await?, output)
}

fn reported(result: ReportResult, output: ReportOutput) -> AppResult<Reported> {
    let mut json = serde_json::to_value(&result)?;
    json["groups"] = serde_json::to_value(&result.groups)?;
    let text = match output {
        ReportOutput::Notes => result.notes.text,
        ReportOutput::Text => {
            let mut text = text(&result);
            if let Some(billable) = billable_text(&result.billable) {
                text.push_str(&format!("Billable: {billable}\n"));
            }
            text
        }
    };
    Ok(Reported {
        result: leading(json, &["from", "to", "title"]),
        text,
    })
}

/// The request with its names resolved to ids.
pub async fn filter_of(db: &Db, request: ReportRequest) -> AppResult<ReportFilter> {
    let mut project_ids = Vec::new();
    for name in &request.projects {
        project_ids.push(projects::resolve(db, name).await?.id);
    }
    let include_archived = true;
    let all_clients = clients::list(db, include_archived).await?;
    let all_tags = tags::list(db).await?;
    Ok(ReportFilter {
        range: request.range,
        group_by: request.group_by,
        rounding: request.rounding,
        client_ids: resolve_all(&request.clients, "client", &all_clients, |c| {
            (&c.id, &c.name)
        })?,
        project_ids,
        tag_ids: resolve_all(&request.tags, "tag", &all_tags, |t| (&t.id, &t.name))?,
        include_archived: false,
    })
}

/// Each of `wanted` as an id: an exact id, else the one item with that name (ignoring case), else the one
/// whose name starts with it. Several matches are `invalid_input` listing them.
fn resolve_all<T>(
    wanted: &[String],
    kind: &str,
    items: &[T],
    id_name: impl Fn(&T) -> (&String, &String),
) -> AppResult<Vec<String>> {
    wanted
        .iter()
        .map(|text| {
            let text = text.trim();
            let lower = text.to_lowercase();
            let matching = |matches: &dyn Fn(&str) -> bool| -> Vec<(&String, &String)> {
                items
                    .iter()
                    .map(&id_name)
                    .filter(|(_, name)| matches(&name.to_lowercase()))
                    .collect()
            };
            if let Some((id, _)) = items
                .iter()
                .map(&id_name)
                .find(|(id, _)| id.as_str() == text)
            {
                return Ok(id.clone());
            }
            let exact = matching(&|name| name == lower);
            let candidates = if exact.is_empty() {
                matching(&|name| name.starts_with(&lower))
            } else {
                exact
            };
            match candidates.as_slice() {
                [] => Err(AppError::NotFound(format!("No {kind} matches `{text}`"))),
                [(id, _)] => Ok((*id).clone()),
                several => Err(AppError::InvalidInput(format!(
                    "`{text}` matches several {kind}s: {}",
                    several
                        .iter()
                        .map(|(_, n)| n.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::db::tags::Tag;
    use crate::error::ErrorKind;
    use crate::reports::{Billable, ReportGroup};

    fn tag(id: &str, name: &str) -> Tag {
        Tag {
            id: id.into(),
            name: name.into(),
            emoji: None,
            created_at: 0,
        }
    }

    fn tags() -> Vec<Tag> {
        vec![
            tag("t-admin", "admin"),
            tag("t-dev", "development"),
            tag("t-ext", "external meeting"),
            tag("t-int", "internal meeting"),
            tag("t-meet", "meet"),
        ]
    }

    fn resolve(wanted: &[&str]) -> AppResult<Vec<String>> {
        let wanted: Vec<String> = wanted.iter().map(|w| w.to_string()).collect();
        resolve_all(&wanted, "tag", &tags(), |t| (&t.id, &t.name))
    }

    #[test]
    fn names_resolve_to_ids_ignoring_case_and_surrounding_space() {
        assert_eq!(
            resolve(&["Admin", "  development "]).unwrap(),
            ["t-admin", "t-dev"]
        );
    }

    #[test]
    fn an_exact_id_resolves_to_itself() {
        assert_eq!(resolve(&["t-ext"]).unwrap(), ["t-ext"]);
    }

    #[test]
    fn a_unique_start_of_a_name_resolves() {
        assert_eq!(resolve(&["dev", "ext"]).unwrap(), ["t-dev", "t-ext"]);
    }

    #[test]
    fn an_exact_name_wins_over_longer_names_that_start_with_it() {
        let tags = vec![tag("a", "meet"), tag("b", "meeting")];
        let wanted = vec!["meet".to_string()];
        assert_eq!(
            resolve_all(&wanted, "tag", &tags, |t| (&t.id, &t.name)).unwrap(),
            ["a"]
        );
    }

    #[test]
    fn nothing_to_resolve_gives_no_ids() {
        assert_eq!(resolve(&[]).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn an_unknown_name_is_not_found() {
        let err = resolve(&["nothing"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(err.to_string(), "No tag matches `nothing`");
    }

    #[test]
    fn an_ambiguous_start_is_invalid_input_listing_the_candidates() {
        let tags = vec![
            tag("a", "external meeting"),
            tag("b", "external review"),
            tag("c", "admin"),
        ];
        let wanted = vec!["ext".to_string()];

        let err = resolve_all(&wanted, "tag", &tags, |t| (&t.id, &t.name)).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(
            err.to_string(),
            "`ext` matches several tags: external meeting, external review"
        );
    }

    #[test]
    fn the_request_has_camel_case_fields() {
        let request = ReportRequest {
            range: Range::ThisMonth,
            group_by: Group::Week,
            rounding: Rounding::Nearest30,
            clients: vec!["acme".into()],
            projects: vec![],
            tags: vec![],
        };
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            json!({
                "range": { "kind": "thisMonth" },
                "groupBy": "week",
                "rounding": "nearest30",
                "clients": ["acme"],
                "projects": [],
                "tags": [],
            })
        );
    }

    fn result_with(recorded_minutes: i64, billable_minutes: i64) -> ReportResult {
        let ms = |m: i64| m * 60_000;
        let day = Day::parse("2026-10-20").unwrap();
        let groups = if recorded_minutes > 0 {
            vec![ReportGroup {
                key: "Portal".into(),
                label: "Portal".into(),
                color: None,
                shade: None,
                billable: None,
                duration_ms: ms(recorded_minutes),
                rounded_ms: ms(recorded_minutes),
                notes: vec![],
                rows: vec![],
            }]
        } else {
            vec![]
        };
        ReportResult {
            filter: ReportFilter::default(),
            from: day,
            to: day,
            title: "Tuesday, October 20, 2026".into(),
            groups,
            total_ms: ms(recorded_minutes),
            total_rounded_ms: ms(recorded_minutes),
            billable: Billable::of(ms(recorded_minutes), ms(billable_minutes)),
            recording_count: 0,
            project_count: 0,
            untagged_rounded_ms: 0,
            charts: crate::reports::ReportCharts::default(),
            clients: vec![],
            notes: crate::reports::CopiedNotes::default(),
        }
    }

    #[test]
    fn the_report_text_ends_with_the_billable_line_when_time_is_recorded() {
        let reported = reported(result_with(480, 384), ReportOutput::Text).unwrap();

        assert_eq!(
            reported.text,
            "Tue 2026-10-20\n\
             \n\
             Portal  8:00 h\n\
             \n\
             Total  8:00 h\n\
             Billable: 6:24 of 8:00 (80 %)\n"
        );
    }

    #[test]
    fn the_cli_json_carries_the_groups_in_both_outputs() {
        for output in [ReportOutput::Text, ReportOutput::Notes] {
            let reported = reported(result_with(480, 384), output).unwrap();

            assert_eq!(
                reported.result["title"], "Tuesday, October 20, 2026",
                "{output:?}"
            );
            assert_eq!(reported.result["groups"][0]["key"], "Portal", "{output:?}");
            assert_eq!(reported.result["groups"][0]["roundedMs"], 28_800_000);
        }
    }

    #[test]
    fn the_notes_reply_is_the_notes_text_without_report_or_billable_lines() {
        let mut result = result_with(480, 384);
        result.notes = crate::reports::CopiedNotes {
            text: "fix login\nreview".into(),
            count: 2,
        };

        let reported = reported(result, ReportOutput::Notes).unwrap();

        assert_eq!(reported.text, "fix login\nreview");
        assert_eq!(
            reported.result["notes"],
            json!({ "text": "fix login\nreview", "count": 2 })
        );
    }

    #[test]
    fn the_report_text_has_no_billable_line_without_recorded_time() {
        let reported = reported(result_with(0, 0), ReportOutput::Text).unwrap();

        assert_eq!(reported.text, "Tue 2026-10-20\n\nNo recordings.\n");
    }

    #[test]
    fn the_json_result_carries_the_billable_share() {
        let with_time = reported(result_with(480, 384), ReportOutput::Text).unwrap();
        let without = reported(result_with(0, 0), ReportOutput::Text).unwrap();

        assert_eq!(
            with_time.result["billable"],
            json!({ "recordedMs": 28_800_000, "billableMs": 23_040_000, "percent": 80 })
        );
        assert_eq!(
            without.result["billable"],
            json!({ "recordedMs": 0, "billableMs": 0, "percent": null })
        );
    }

    #[test]
    fn the_reply_puts_the_text_beside_the_result_fields() {
        let reported = Reported {
            text: "Total  0:00 h\n".into(),
            result: json!({ "totalMs": 0, "groups": [] }),
        };
        let wire = serde_json::to_value(&reported).unwrap();
        assert_eq!(
            wire,
            json!({ "text": "Total  0:00 h\n", "totalMs": 0, "groups": [] })
        );
        assert_eq!(serde_json::from_value::<Reported>(wire).unwrap(), reported);
    }

    #[test]
    fn the_reply_writes_the_text_after_the_result_fields() {
        let reported = Reported {
            text: "Total  0:00 h\n".into(),
            result: json!({ "totalMs": 0, "groups": [] }),
        };

        assert_eq!(
            serde_json::to_string(&reported).unwrap(),
            r#"{"totalMs":0,"groups":[],"text":"Total  0:00 h\n"}"#
        );
    }

    fn keys_of(reported: &Reported) -> Vec<String> {
        serde_json::to_value(reported)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect()
    }

    #[test]
    fn the_cli_json_starts_with_the_period_and_ends_with_the_text() {
        for output in [ReportOutput::Text, ReportOutput::Notes] {
            let keys = keys_of(&reported(result_with(480, 384), output).unwrap());

            assert_eq!(keys[..3], ["from", "to", "title"], "{output:?}");
            assert_eq!(keys.last().map(String::as_str), Some("text"), "{output:?}");
        }
    }

    #[test]
    fn leading_moves_the_named_fields_to_the_front_and_keeps_the_rest_in_order() {
        let result = json!({ "b": 1, "title": "T", "a": 2, "from": "F" });

        let ordered = leading(result, &["from", "to", "title"]);

        let keys: Vec<&String> = ordered.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["from", "title", "b", "a"]);
        assert_eq!(ordered["from"], "F");
    }

    #[test]
    fn leading_leaves_a_result_that_is_not_an_object_alone() {
        assert_eq!(leading(json!([1, 2]), &["from"]), json!([1, 2]));
    }
}
