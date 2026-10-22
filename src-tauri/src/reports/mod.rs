//! Reports: recordings of a range, filtered, rounded per recording and grouped (ADR 0018). `text` renders
//! the result as the plain text the Reports screen copies and the CLI prints.

mod billable;
pub mod cli;
pub mod commands;
mod groups;
mod range;
mod rows;
pub mod saved;
#[cfg(test)]
mod testing;
mod text;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use billable::{Billable, billable, billable_text};
pub use groups::day_label;
pub(crate) use groups::week_label;
pub use range::{Range, resolve, shift};
pub use rows::rounding_step;
pub use text::text;

use crate::db::clients::Client;
use crate::db::projects::Project;
use crate::db::recordings::{Recording, Span};
use crate::db::tags::Tag;
use crate::db::time::{Day, Ms};
use crate::db::{Db, clients, projects, recordings, tags};
use crate::error::AppResult;
use crate::store::settings::{Group, Rounding};

const MINUTE: Ms = 60_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ReportFilter {
    pub range: Range,
    /// Empty lists keep everything; the lists narrow together.
    pub client_ids: Vec<String>,
    pub project_ids: Vec<String>,
    pub tag_ids: Vec<String>,
    pub group_by: Group,
    pub rounding: Rounding,
    pub include_archived: bool,
}

impl Default for ReportFilter {
    fn default() -> Self {
        Self {
            range: Range::Today,
            client_ids: Vec::new(),
            project_ids: Vec::new(),
            tag_ids: Vec::new(),
            group_by: Group::Project,
            rounding: Rounding::None,
            include_archived: false,
        }
    }
}

/// A recording's part in one day of the range: its spans of that day.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ReportRow {
    pub id: String,
    pub day: Day,
    /// The start of the first span and the end of the last one.
    pub start: Ms,
    pub end: Ms,
    /// The day's parts of its spans, by start.
    pub spans: Vec<Span>,
    pub duration_ms: i64,
    pub rounded_ms: i64,
    pub project_id: String,
    pub project: String,
    pub client: Option<String>,
    pub color: String,
    pub shade: u8,
    pub billable: bool,
    pub tag: Option<String>,
    #[serde(skip)]
    #[ts(skip)]
    pub tag_id: Option<String>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ReportNote {
    pub text: String,
    /// The days the note occurs on, ascending.
    pub days: Vec<Day>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ReportGroup {
    pub key: String,
    pub label: String,
    /// The project's color, shade and billable flag when grouped by project.
    pub color: Option<String>,
    pub shade: Option<u8>,
    pub billable: Option<bool>,
    pub duration_ms: i64,
    pub rounded_ms: i64,
    /// Distinct notes in the order they first occur.
    pub notes: Vec<ReportNote>,
    /// By start.
    pub rows: Vec<ReportRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ReportResult {
    pub filter: ReportFilter,
    pub from: Day,
    pub to: Day,
    pub groups: Vec<ReportGroup>,
    pub total_ms: i64,
    pub total_rounded_ms: i64,
    /// Of the rounded times, so it adds up with `total_rounded_ms`.
    pub billable: Billable,
}

/// What a report reads besides the recordings.
pub struct ReportData {
    pub recordings: Vec<Recording>,
    pub projects: Vec<Project>,
    pub clients: Vec<Client>,
    pub tags: Vec<Tag>,
}

pub async fn run(db: &Db, filter: &ReportFilter, today: Day, now: Ms) -> AppResult<ReportResult> {
    let (from, to) = resolve(&filter.range, today)?;
    let data = ReportData {
        recordings: recordings::list_for_range(db, from.bounds().0, to.bounds().1).await?,
        projects: projects::list(db, true).await?,
        clients: clients::list(db, true).await?,
        tags: tags::list(db).await?,
    };
    Ok(build(filter, (from, to), &data, now))
}

/// The report of `data` over the days `from..=to`; a running recording ends at `now`.
pub fn build(
    filter: &ReportFilter,
    (from, to): (Day, Day),
    data: &ReportData,
    now: Ms,
) -> ReportResult {
    let rows = rows::rows(filter, (from, to), data, now);
    let groups = groups::groups(filter, &rows);
    let total_rounded_ms = rows.iter().map(|r| r.rounded_ms).sum();
    let billable_ms = rows
        .iter()
        .filter(|r| r.billable)
        .map(|r| r.rounded_ms)
        .sum();
    ReportResult {
        filter: filter.clone(),
        from,
        to,
        total_ms: rows.iter().map(|r| r.duration_ms).sum(),
        total_rounded_ms,
        billable: Billable::of(total_rounded_ms, billable_ms),
        groups,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::db::projects::ProjectInput;
    use crate::db::recordings::NewRecording;
    use crate::error::ErrorKind;
    use crate::reports::testing::*;
    use crate::test_support::client;

    #[test]
    fn the_default_filter_reports_today_by_project_without_rounding() {
        assert_eq!(
            serde_json::to_value(ReportFilter::default()).unwrap(),
            json!({
                "range": { "kind": "today" },
                "clientIds": [],
                "projectIds": [],
                "tagIds": [],
                "groupBy": "project",
                "rounding": "none",
                "includeArchived": false,
            })
        );
    }

    #[test]
    fn a_result_serializes_with_camel_case_fields() {
        let result = report(
            &ReportFilter::default(),
            D,
            D,
            vec![with_note(on("r1", "p-portal", D, 90), "fix login")],
        );
        let wire = serde_json::to_value(&result).unwrap();
        let start = at(D, 9, 0);

        assert_eq!(wire["from"], D);
        assert_eq!(wire["to"], D);
        assert_eq!(wire["totalMs"], minutes(90));
        assert_eq!(wire["totalRoundedMs"], minutes(90));
        assert_eq!(
            wire["billable"],
            json!({ "recordedMs": minutes(90), "billableMs": 0, "percent": 0 })
        );
        assert!(wire.get("tagOverlap").is_none());
        assert_eq!(
            wire["groups"][0],
            json!({
                "key": "p-portal",
                "label": "Acme / Portal",
                "color": "blue",
                "shade": 4,
                "billable": false,
                "durationMs": minutes(90),
                "roundedMs": minutes(90),
                "notes": [{ "text": "fix login", "days": [D] }],
                "rows": [{
                    "id": "r1",
                    "day": D,
                    "start": start,
                    "end": start + minutes(90),
                    "spans": [{ "start": start, "end": start + minutes(90) }],
                    "durationMs": minutes(90),
                    "roundedMs": minutes(90),
                    "projectId": "p-portal",
                    "project": "Portal",
                    "client": "Acme",
                    "color": "blue",
                    "shade": 4,
                    "billable": false,
                    "tag": null,
                    "note": "fix login",
                }],
            })
        );
    }

    #[test]
    fn groups_and_total_sum_their_recordings() {
        let result = report(
            &by(Group::Project),
            D,
            D,
            vec![
                rec("r1", "p-portal", at(D, 9, 0), Some(at(D, 10, 0)), "", None),
                rec(
                    "r2",
                    "p-portal",
                    at(D, 11, 0),
                    Some(at(D, 11, 30)),
                    "",
                    None,
                ),
                rec("r3", "p-site", at(D, 13, 0), Some(at(D, 13, 45)), "", None),
            ],
        );

        assert_eq!(durations(&result), [90, 45]);
        assert_eq!(result.total_ms, minutes(135));
        assert_eq!(result.total_rounded_ms, minutes(135));
        assert_eq!(result.groups[0].rows.len(), 2);
    }

    #[test]
    fn a_range_without_recordings_is_an_empty_report() {
        let filter = by(Group::Tag);

        let result = report(&filter, D, "2026-10-26", vec![]);

        assert_eq!(
            result,
            ReportResult {
                filter,
                from: day(D),
                to: day("2026-10-26"),
                groups: vec![],
                total_ms: 0,
                total_rounded_ms: 0,
                billable: Billable::of(0, 0),
            }
        );
    }

    #[tokio::test]
    async fn run_reports_the_spans_of_a_stored_recording() {
        let db = Db::open_memory().await.unwrap();
        let p = crate::test_support::project(&db, "Solo").await;
        recordings::insert(
            &db,
            NewRecording {
                id: "r1".into(),
                project_id: p.id.clone(),
                spans: vec![
                    (at("2026-10-21", 14, 25), Some(at("2026-10-21", 15, 0))),
                    (at("2026-10-21", 16, 5), Some(at("2026-10-21", 20, 15))),
                ],
                note: "WEB-214".into(),
                tag_id: None,
            },
        )
        .await
        .unwrap();

        let result = run(&db, &ReportFilter::default(), day("2026-10-21"), now())
            .await
            .unwrap();

        let rows = &result.groups[0].rows;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].spans.len(), 2);
        assert_eq!(rows[0].duration_ms, minutes(285));
        assert_eq!(result.total_ms, minutes(285));
    }

    #[tokio::test]
    async fn run_reads_the_database_and_resolves_the_range_against_today() {
        let db = Db::open_memory().await.unwrap();
        let acme = client(&db, "Acme").await;
        let portal = projects::insert(
            &db,
            ProjectInput {
                client_id: Some(acme.id),
                name: "Portal".into(),
                color: String::new(),
                shade: None,
                billable: false,
            },
        )
        .await
        .unwrap();
        let admin = tags::insert(&db, "admin").await.unwrap();
        let start = at("2026-10-21", 9, 0);
        recordings::insert(
            &db,
            NewRecording {
                id: "r1".into(),
                project_id: portal.id.clone(),
                spans: vec![(start, Some(start + minutes(90)))],
                note: " fix login ".into(),
                tag_id: Some(admin.id.clone()),
            },
        )
        .await
        .unwrap();
        recordings::insert(
            &db,
            NewRecording {
                id: "r2".into(),
                project_id: portal.id.clone(),
                spans: vec![(at("2026-10-20", 9, 0), Some(at("2026-10-20", 10, 0)))],
                note: String::new(),
                tag_id: None,
            },
        )
        .await
        .unwrap();

        let result = run(&db, &ReportFilter::default(), day("2026-10-21"), now())
            .await
            .unwrap();

        assert_eq!(
            (result.from, result.to),
            (day("2026-10-21"), day("2026-10-21"))
        );
        assert_eq!(keys(&result), [portal.id.as_str()]);
        assert_eq!(labels(&result), ["Acme / Portal"]);
        assert_eq!(result.total_ms, minutes(90));
        assert_eq!(result.groups[0].rows[0].tag.as_deref(), Some("admin"));
        assert_eq!(result.groups[0].notes[0].text, "fix login");
    }

    #[tokio::test]
    async fn run_ends_a_running_recording_at_now() {
        let db = Db::open_memory().await.unwrap();
        let p = crate::test_support::project(&db, "Solo").await;
        crate::test_support::recording(&db, &p.id, at("2026-10-21", 9, 0), None).await;

        let result = run(
            &db,
            &ReportFilter::default(),
            day("2026-10-21"),
            at("2026-10-21", 10, 15),
        )
        .await
        .unwrap();

        assert_eq!(result.total_ms, minutes(75));
    }

    #[tokio::test]
    async fn run_includes_archived_projects_only_when_asked() {
        let db = Db::open_memory().await.unwrap();
        let p = crate::test_support::project(&db, "Old").await;
        crate::test_support::recording(
            &db,
            &p.id,
            at("2026-10-21", 9, 0),
            Some(at("2026-10-21", 10, 0)),
        )
        .await;
        projects::set_archived(&db, &p.id, true).await.unwrap();

        let hidden = run(&db, &ReportFilter::default(), day("2026-10-21"), now())
            .await
            .unwrap();
        let shown = run(
            &db,
            &ReportFilter {
                include_archived: true,
                ..ReportFilter::default()
            },
            day("2026-10-21"),
            now(),
        )
        .await
        .unwrap();

        assert!(hidden.groups.is_empty());
        assert_eq!(shown.total_ms, minutes(60));
    }

    #[tokio::test]
    async fn run_rejects_a_reversed_custom_range() {
        let db = Db::open_memory().await.unwrap();
        let filter = ReportFilter {
            range: Range::Custom {
                from: day("2026-10-07"),
                to: day("2026-10-01"),
            },
            ..ReportFilter::default()
        };

        let err = run(&db, &filter, day("2026-10-21"), now())
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }
}
