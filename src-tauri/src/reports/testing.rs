//! Report fixtures: projects, clients and tags of two clients, recordings built from local times.

use super::{MINUTE, Range, ReportData, ReportFilter, ReportResult, build};
use crate::db::clients::Client;
use crate::db::projects::Project;
use crate::db::recordings::{Recording, RecordingSpan};
use crate::db::tags::Tag;
use crate::db::time::{Day, Ms};
use crate::store::settings::{DayTitle, Group};

pub const D: &str = "2026-10-20";
/// The day the fixtures' reports are titled on.
pub const TODAY: &str = "2026-10-22";

pub fn day(s: &str) -> Day {
    Day::parse(s).unwrap()
}

pub fn minutes(m: i64) -> Ms {
    m * MINUTE
}

/// The local time `h:m` on the day `d`; kept clear of DST changes.
pub fn at(d: &str, h: i64, m: i64) -> Ms {
    day(d).bounds().0 + minutes(h * 60 + m)
}

pub fn now() -> Ms {
    at("2026-12-31", 12, 0)
}

pub fn rec(
    id: &str,
    project: &str,
    start: Ms,
    end: Option<Ms>,
    note: &str,
    tag_id: Option<&str>,
) -> Recording {
    spanned(id, project, &[(start, end)], note, tag_id)
}

pub fn spanned(
    id: &str,
    project: &str,
    spans: &[(Ms, Option<Ms>)],
    note: &str,
    tag_id: Option<&str>,
) -> Recording {
    Recording {
        id: id.into(),
        project_id: project.into(),
        note: note.into(),
        tag_id: tag_id.map(String::from),
        created_at: 0,
        updated_at: 0,
        spans: spans
            .iter()
            .enumerate()
            .map(|(i, &(start, end))| RecordingSpan {
                id: format!("{id}-s{i}"),
                start,
                end,
            })
            .collect(),
    }
}

/// `id` with closed spans, each given as `(day, start hour, start minute, end hour, end minute)`.
pub fn in_spans(id: &str, project: &str, parts: &[(&str, i64, i64, i64, i64)]) -> Recording {
    let spans: Vec<(Ms, Option<Ms>)> = parts
        .iter()
        .map(|&(d, sh, sm, eh, em)| (at(d, sh, sm), Some(at(d, eh, em))))
        .collect();
    spanned(id, project, &spans, "", None)
}

/// `id` on project `project` for `len` minutes from 09:00 on `d`.
pub fn on(id: &str, project: &str, d: &str, len: i64) -> Recording {
    let start = at(d, 9, 0);
    rec(id, project, start, Some(start + minutes(len)), "", None)
}

pub fn with_note(mut r: Recording, note: &str) -> Recording {
    r.note = note.into();
    r
}

pub fn with_tag(mut r: Recording, tag_id: &str) -> Recording {
    r.tag_id = Some(tag_id.into());
    r
}

pub fn project(id: &str, name: &str, client_id: Option<&str>, color: &str, shade: u8) -> Project {
    Project {
        id: id.into(),
        client_id: client_id.map(String::from),
        name: name.into(),
        color: color.into(),
        shade,
        billable: false,
        archived_at: None,
        created_at: 0,
    }
}

pub fn data(recordings: Vec<Recording>) -> ReportData {
    data_billing(recordings, &[])
}

/// `data` where the projects with the given ids are billable.
pub fn data_billing(recordings: Vec<Recording>, billable: &[&str]) -> ReportData {
    let mut data = data_of(recordings);
    for project in &mut data.projects {
        project.billable = billable.contains(&project.id.as_str());
    }
    data
}

pub fn data_of(recordings: Vec<Recording>) -> ReportData {
    let named = |id: &str, name: &str| Client {
        id: id.into(),
        name: name.into(),
        color: "blue".into(),
        archived_at: None,
        created_at: 0,
    };
    let mut legacy = project("p-old", "Legacy", Some("c-acme"), "blue", 3);
    legacy.archived_at = Some(1);
    ReportData {
        recordings,
        projects: vec![
            project("p-portal", "Portal", Some("c-acme"), "blue", 4),
            project("p-api", "API", Some("c-acme"), "blue", 1),
            project("p-site", "Site", Some("c-beta"), "green", 7),
            project("p-solo", "Aardvark", None, "orange", 2),
            legacy,
        ],
        clients: vec![named("c-acme", "Acme"), named("c-beta", "Beta")],
        tags: vec![
            Tag {
                id: "t-admin".into(),
                name: "admin".into(),
                emoji: None,
                created_at: 0,
            },
            Tag {
                id: "t-dev".into(),
                name: "development".into(),
                emoji: None,
                created_at: 0,
            },
            Tag {
                id: "t-sup".into(),
                name: "support".into(),
                emoji: None,
                created_at: 0,
            },
        ],
    }
}

pub fn report(
    filter: &ReportFilter,
    from: &str,
    to: &str,
    recordings: Vec<Recording>,
) -> ReportResult {
    build(
        filter,
        (day(from), day(to)),
        &data(recordings),
        now(),
        DayTitle::Short,
        day(TODAY),
    )
}

pub fn by(group_by: Group) -> ReportFilter {
    ReportFilter {
        group_by,
        ..ReportFilter::default()
    }
}

pub fn labels(result: &ReportResult) -> Vec<&str> {
    result.groups.iter().map(|g| g.label.as_str()).collect()
}

pub fn keys(result: &ReportResult) -> Vec<&str> {
    result.groups.iter().map(|g| g.key.as_str()).collect()
}

pub fn durations(result: &ReportResult) -> Vec<i64> {
    result
        .groups
        .iter()
        .map(|g| g.duration_ms / MINUTE)
        .collect()
}

pub fn billing_report(
    filter: &ReportFilter,
    billable: &[&str],
    recordings: Vec<Recording>,
) -> ReportResult {
    build(
        filter,
        (day(D), day(D)),
        &data_billing(recordings, billable),
        now(),
        DayTitle::Short,
        day(TODAY),
    )
}

pub fn custom(from: &str, to: &str) -> Range {
    Range::Custom {
        from: day(from),
        to: day(to),
    }
}
