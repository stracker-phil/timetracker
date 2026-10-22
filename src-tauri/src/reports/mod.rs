//! Reports: recordings of a range, filtered, rounded per recording and grouped (ADR 0018), with the totals,
//! donuts and client/project table the Reports screen shows and each project's notes to copy. `text`
//! renders the result as the plain text the Reports screen copies and the CLI prints.

mod billable;
mod charts;
pub mod cli;
pub mod commands;
mod groups;
mod notes;
mod period;
mod range;
mod rows;
pub mod saved;
mod table;
#[cfg(test)]
mod testing;
mod text;

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use billable::{Billable, billable, billable_text};
pub use charts::ReportCharts;
pub use groups::day_label;
pub(crate) use groups::week_label;
pub use notes::CopiedNotes;
pub use period::{PeriodTotals, period_totals};
pub use range::{Range, resolve, shift};
pub use rows::rounding_step;
use table::ReportClient;
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
    #[serde(skip)]
    #[ts(skip)]
    pub client_id: Option<String>,
    #[serde(skip)]
    #[ts(skip)]
    pub client_color: Option<String>,
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
    /// By `filter.groupBy`, for the text and MCP; left out on the wire, where `clients` carries the rows
    /// (the CLI's `--json` adds them back).
    #[serde(skip)]
    #[ts(skip)]
    pub groups: Vec<ReportGroup>,
    pub total_ms: i64,
    pub total_rounded_ms: i64,
    /// Of the rounded times, so it adds up with `total_rounded_ms`.
    pub billable: Billable,
    /// Distinct recordings and projects.
    pub recording_count: u32,
    pub project_count: u32,
    /// The rounded time of recordings without a tag.
    pub untagged_rounded_ms: i64,
    pub charts: ReportCharts,
    /// The Reports screen's table: by client, then project.
    pub clients: Vec<ReportClient>,
    /// Every note of the report, as "Copy notes" copies a project's.
    pub notes: CopiedNotes,
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
    let rounded_where = |keep: fn(&ReportRow) -> bool| -> i64 {
        rows.iter().filter(|r| keep(r)).map(|r| r.rounded_ms).sum()
    };
    let total_rounded_ms = rows.iter().map(|r| r.rounded_ms).sum();
    let distinct =
        |key: fn(&ReportRow) -> &str| rows.iter().map(key).collect::<HashSet<_>>().len() as u32;
    ReportResult {
        filter: filter.clone(),
        from,
        to,
        groups: groups::groups(filter, &rows),
        total_ms: rows.iter().map(|r| r.duration_ms).sum(),
        total_rounded_ms,
        billable: Billable::of(total_rounded_ms, rounded_where(|r| r.billable)),
        recording_count: distinct(|r| &r.id),
        project_count: distinct(|r| &r.project_id),
        untagged_rounded_ms: rounded_where(|r| r.tag_id.is_none()),
        charts: charts::charts(&rows, &data.tags),
        clients: table::table(&rows),
        notes: notes::copied_notes(&rows),
    }
}

/// The index of the first item matching `is`, after appending `new()` when none does.
fn index_of<T>(items: &mut Vec<T>, is: impl Fn(&T) -> bool, new: impl FnOnce() -> T) -> usize {
    items.iter().position(is).unwrap_or_else(|| {
        items.push(new());
        items.len() - 1
    })
}

/// `Client / Project`, or the project's name without a client.
fn project_label(row: &ReportRow) -> String {
    match &row.client {
        Some(client) => format!("{client} / {}", row.project),
        None => row.project.clone(),
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
        assert!(wire.get("groups").is_none());
        assert_eq!(
            wire["clients"][0]["projects"][0]["rows"][0],
            json!({
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
    fn the_result_notes_hold_every_note_while_a_project_holds_its_own() {
        let result = report(
            &ReportFilter::default(),
            D,
            D,
            vec![
                with_note(
                    rec("r1", "p-portal", at(D, 9, 0), Some(at(D, 9, 30)), "", None),
                    "login",
                ),
                with_note(
                    rec("r2", "p-site", at(D, 10, 0), Some(at(D, 10, 30)), "", None),
                    "launch",
                ),
                with_note(
                    rec(
                        "r3",
                        "p-portal",
                        at(D, 11, 0),
                        Some(at(D, 11, 30)),
                        "",
                        None,
                    ),
                    "logout",
                ),
            ],
        );

        assert_eq!(
            result.notes,
            CopiedNotes {
                text: "login\nlaunch\nlogout".into(),
                count: 3
            }
        );
        let project_notes: Vec<(&str, &str)> = result
            .clients
            .iter()
            .flat_map(|c| &c.projects)
            .map(|p| (p.id.as_str(), p.notes.text.as_str()))
            .collect();
        assert_eq!(
            project_notes,
            [("p-portal", "login\nlogout"), ("p-site", "launch")]
        );
    }

    #[test]
    fn a_recording_over_several_days_counts_once() {
        let result = report(
            &ReportFilter::default(),
            D,
            "2026-10-21",
            vec![
                in_spans(
                    "r1",
                    "p-portal",
                    &[(D, 9, 0, 10, 0), ("2026-10-21", 9, 0, 10, 0)],
                ),
                on("r2", "p-site", D, 30),
                on("r3", "p-site", "2026-10-21", 30),
            ],
        );

        assert_eq!(result.recording_count, 3);
        assert_eq!(result.project_count, 2);
    }

    #[test]
    fn untagged_time_is_the_rounded_time_of_recordings_without_a_tag() {
        let filter = ReportFilter {
            rounding: Rounding::Up15,
            ..ReportFilter::default()
        };

        let result = report(
            &filter,
            D,
            D,
            vec![
                with_tag(on("r1", "p-portal", D, 10), "t-admin"),
                on("r2", "p-portal", D, 20),
            ],
        );

        assert_eq!(result.untagged_rounded_ms, minutes(30));
        assert_eq!(result.total_rounded_ms, minutes(45));
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
                recording_count: 0,
                project_count: 0,
                untagged_rounded_ms: 0,
                charts: ReportCharts::default(),
                clients: vec![],
                notes: CopiedNotes::default(),
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
