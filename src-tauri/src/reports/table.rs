//! The Reports screen's table: clients, their projects and the projects' recordings, each with totals.

use serde::Serialize;
use ts_rs::TS;

use super::notes::{CopiedNotes, copied_notes};
use super::{ReportRow, index_of, project_label};

pub(super) const NO_CLIENT_KEY: &str = "none";
pub(super) const NO_CLIENT_LABEL: &str = "No client";

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ReportClient {
    /// The client's id, or `none` for projects without a client.
    pub key: String,
    pub label: String,
    /// The client's accent; none without a client.
    pub color: Option<String>,
    pub duration_ms: i64,
    pub rounded_ms: i64,
    /// By name.
    pub projects: Vec<ReportProject>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ReportProject {
    pub id: String,
    pub name: String,
    /// `Client / Project`, or the name without a client.
    pub label: String,
    pub color: String,
    pub shade: u8,
    pub billable: bool,
    pub duration_ms: i64,
    pub rounded_ms: i64,
    /// What "Copy notes" puts in the clipboard.
    pub notes: CopiedNotes,
    /// By start.
    pub rows: Vec<ReportRow>,
}

/// `rows` (sorted by start) by client and project; clients and projects by name, projects without a client
/// last.
pub(super) fn table(rows: &[ReportRow]) -> Vec<ReportClient> {
    finish(grouped(rows))
}

fn grouped(rows: &[ReportRow]) -> Vec<ReportClient> {
    let mut clients: Vec<ReportClient> = Vec::new();
    for row in rows {
        let key = row.client_id.as_deref().unwrap_or(NO_CLIENT_KEY);
        let client = index_of(&mut clients, |c| c.key == key, || client_of(row, key));
        let client = &mut clients[client];
        client.duration_ms += row.duration_ms;
        client.rounded_ms += row.rounded_ms;
        let project = index_of(
            &mut client.projects,
            |p| p.id == row.project_id,
            || project_of(row),
        );
        let project = &mut client.projects[project];
        project.duration_ms += row.duration_ms;
        project.rounded_ms += row.rounded_ms;
        project.rows.push(row.clone());
    }
    clients
}

/// Sorts clients and projects and gives each project its notes.
fn finish(mut clients: Vec<ReportClient>) -> Vec<ReportClient> {
    for client in &mut clients {
        client
            .projects
            .sort_by_cached_key(|p| (p.name.to_lowercase(), p.id.clone()));
        for project in &mut client.projects {
            project.notes = copied_notes(&project.rows);
        }
    }
    clients.sort_by_cached_key(|c| {
        (
            c.key == NO_CLIENT_KEY,
            c.label.to_lowercase(),
            c.key.clone(),
        )
    });
    clients
}

fn client_of(row: &ReportRow, key: &str) -> ReportClient {
    ReportClient {
        key: key.to_string(),
        label: row.client.clone().unwrap_or_else(|| NO_CLIENT_LABEL.into()),
        color: row.client_color.clone(),
        duration_ms: 0,
        rounded_ms: 0,
        projects: Vec::new(),
    }
}

fn project_of(row: &ReportRow) -> ReportProject {
    ReportProject {
        id: row.project_id.clone(),
        name: row.project.clone(),
        label: project_label(row),
        color: row.color.clone(),
        shade: row.shade,
        billable: row.billable,
        duration_ms: 0,
        rounded_ms: 0,
        notes: CopiedNotes::default(),
        rows: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::recordings::Recording;
    use crate::reports::ReportFilter;
    use crate::reports::testing::*;
    use crate::store::settings::Rounding;

    fn clients_of(recordings: Vec<Recording>) -> Vec<ReportClient> {
        report(&ReportFilter::default(), D, D, recordings).clients
    }

    fn mixed() -> Vec<Recording> {
        vec![
            on("r1", "p-solo", D, 10),
            on("r2", "p-site", D, 20),
            on("r3", "p-portal", D, 30),
            on("r4", "p-api", D, 40),
        ]
    }

    #[test]
    fn clients_are_sorted_by_name_with_no_client_last() {
        let clients = clients_of(mixed());

        let keys: Vec<&str> = clients.iter().map(|c| c.key.as_str()).collect();
        let labels: Vec<&str> = clients.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(keys, ["c-acme", "c-beta", NO_CLIENT_KEY]);
        assert_eq!(labels, ["Acme", "Beta", NO_CLIENT_LABEL]);
        assert_eq!(clients[0].color.as_deref(), Some("blue"));
        assert_eq!(clients[2].color, None);
    }

    #[test]
    fn projects_are_sorted_by_name_within_their_client() {
        let clients = clients_of(mixed());

        let names: Vec<&str> = clients[0]
            .projects
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        let labels: Vec<&str> = clients[0]
            .projects
            .iter()
            .map(|p| p.label.as_str())
            .collect();
        assert_eq!(names, ["API", "Portal"]);
        assert_eq!(labels, ["Acme / API", "Acme / Portal"]);
        assert_eq!(clients[2].projects[0].label, "Aardvark");
    }

    #[test]
    fn client_and_project_totals_are_the_sums_of_their_rows() {
        let filter = ReportFilter {
            rounding: Rounding::Up15,
            ..ReportFilter::default()
        };
        let result = report(&filter, D, D, mixed());

        let acme = &result.clients[0];
        assert_eq!(
            (acme.duration_ms, acme.rounded_ms),
            (minutes(70), minutes(45 + 30))
        );
        let api = &acme.projects[0];
        assert_eq!(
            (api.duration_ms, api.rounded_ms),
            (minutes(40), minutes(45))
        );
        let portal = &acme.projects[1];
        assert_eq!(
            (portal.duration_ms, portal.rounded_ms),
            (minutes(30), minutes(30))
        );
    }

    #[test]
    fn a_projects_rows_are_ordered_by_start() {
        let late = rec(
            "late",
            "p-portal",
            at(D, 15, 0),
            Some(at(D, 16, 0)),
            "",
            None,
        );
        let early = rec(
            "early",
            "p-portal",
            at(D, 8, 0),
            Some(at(D, 8, 30)),
            "",
            None,
        );

        let clients = clients_of(vec![late, early]);

        let ids: Vec<&str> = clients[0].projects[0]
            .rows
            .iter()
            .map(|r| r.id.as_str())
            .collect();
        assert_eq!(ids, ["early", "late"]);
    }

    #[test]
    fn a_recording_with_two_spans_is_one_row() {
        let clients = clients_of(vec![in_spans(
            "r1",
            "p-portal",
            &[(D, 9, 0, 10, 0), (D, 11, 0, 11, 30)],
        )]);

        let project = &clients[0].projects[0];
        assert_eq!(project.rows.len(), 1);
        assert_eq!(project.rows[0].spans.len(), 2);
        assert_eq!(project.duration_ms, minutes(90));
    }

    #[test]
    fn a_projects_notes_hold_only_its_own_notes() {
        let clients = clients_of(vec![
            with_note(on("r1", "p-portal", D, 10), "portal work"),
            with_note(on("r2", "p-api", D, 10), "api work"),
            on("r3", "p-site", D, 10),
        ]);

        let notes = |client: usize, project: usize| clients[client].projects[project].notes.clone();
        assert_eq!(
            notes(0, 0),
            CopiedNotes {
                text: "api work".into(),
                count: 1
            }
        );
        assert_eq!(
            notes(0, 1),
            CopiedNotes {
                text: "portal work".into(),
                count: 1
            }
        );
        assert_eq!(notes(1, 0), CopiedNotes::default());
    }

    #[test]
    fn a_report_without_recordings_has_no_clients() {
        assert_eq!(clients_of(vec![]), vec![]);
    }
}
