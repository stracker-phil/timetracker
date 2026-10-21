//! `timetracker report`: the filter with clients, projects and tags by name, resolved in the app.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::{Range, ReportFilter, ReportResult, run, text};
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

/// The result with its text, which the CLI prints without `--json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reported {
    pub text: String,
    #[serde(flatten)]
    pub result: serde_json::Value,
}

pub async fn report(app: &AppHandle, request: ReportRequest) -> AppResult<Reported> {
    let db = app.state::<AppState>().data.db()?;
    let filter = filter_of(&db, request).await?;
    let result: ReportResult = run(&db, &filter, Day::today(), now_ms()).await?;
    Ok(Reported {
        text: text(&result),
        result: serde_json::to_value(result)?,
    })
}

async fn filter_of(db: &Db, request: ReportRequest) -> AppResult<ReportFilter> {
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

    fn tag(id: &str, name: &str) -> Tag {
        Tag {
            id: id.into(),
            name: name.into(),
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
}
