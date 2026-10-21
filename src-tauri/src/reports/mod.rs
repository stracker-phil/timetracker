//! Reports: recordings of a range, filtered, rounded per recording and grouped (ADR 0018). `text` renders
//! the result as the plain text the Reports screen copies and the CLI prints.

pub mod cli;
pub mod commands;
pub mod saved;
mod text;

use std::collections::HashMap;

use chrono::{Datelike, Days, Months};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use text::text;

use crate::db::clients::Client;
use crate::db::projects::Project;
use crate::db::recordings::Recording;
use crate::db::tags::Tag;
use crate::db::time::{Day, Ms, clip};
use crate::db::{Db, clients, projects, recordings, tags};
use crate::error::{AppError, AppResult};
use crate::store::settings::{Group, Rounding};

const MINUTE: Ms = 60_000;
const NO_TAG_KEY: &str = "untagged";
const NO_TAG_LABEL: &str = "No tag";

/// Presets stay relative, so a saved "this month" report shows the current month.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum Range {
    Today,
    Yesterday,
    ThisWeek,
    LastWeek,
    ThisMonth,
    LastMonth,
    Custom { from: Day, to: Day },
}

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

/// A recording's part in one day of the range.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ReportRow {
    pub id: String,
    pub day: Day,
    pub start: Ms,
    pub end: Ms,
    pub duration_ms: i64,
    pub rounded_ms: i64,
    pub project_id: String,
    pub project: String,
    pub client: Option<String>,
    pub color: String,
    pub shade: u8,
    /// Sorted by name.
    pub tags: Vec<String>,
    /// The ids of `tags`, in their order.
    #[serde(skip)]
    #[ts(skip)]
    pub tag_ids: Vec<String>,
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
    /// The project's color and shade when grouped by project.
    pub color: Option<String>,
    pub shade: Option<u8>,
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
    /// Every row once, also when tags overlap.
    pub total_ms: i64,
    pub total_rounded_ms: i64,
    /// Grouped by tag and a recording has several tags, so it counts in each of their groups.
    pub tag_overlap: bool,
}

/// The first and last day of `range`; weeks start on Monday.
pub fn resolve(range: &Range, today: Day) -> AppResult<(Day, Day)> {
    let (from, to) = match *range {
        Range::Today => (today, today),
        Range::Yesterday => (today.prev(), today.prev()),
        Range::ThisWeek => {
            let start = today.week_start();
            (start, add_days(start, 6))
        }
        Range::LastWeek => {
            let start = add_days(today.week_start(), -7);
            (start, add_days(start, 6))
        }
        Range::ThisMonth => month(today.month_start()),
        Range::LastMonth => month(today.month_start().prev().month_start()),
        Range::Custom { from, to } => (from, to),
    };
    if from > to {
        return Err(AppError::InvalidInput(format!(
            "The range starts after it ends: {from} – {to}"
        )));
    }
    Ok((from, to))
}

fn add_days(day: Day, days: i64) -> Day {
    let date = if days >= 0 {
        day.date() + Days::new(days.unsigned_abs())
    } else {
        day.date() - Days::new(days.unsigned_abs())
    };
    Day::from_date(date)
}

fn month(start: Day) -> (Day, Day) {
    let next = start.date() + Months::new(1);
    let last = Day::from_date(next).prev();
    (start, last)
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
    let rows = rows(filter, (from, to), data, now);
    let groups = groups(filter, &rows);
    let counted: usize = groups.iter().map(|g| g.rows.len()).sum();
    ReportResult {
        filter: filter.clone(),
        from,
        to,
        tag_overlap: counted > rows.len(),
        total_ms: rows.iter().map(|r| r.duration_ms).sum(),
        total_rounded_ms: rows.iter().map(|r| r.rounded_ms).sum(),
        groups,
    }
}

fn wanted(filter: &ReportFilter, project: &Project, recording: &Recording) -> bool {
    let kept = |ids: &[String], id: &String| ids.is_empty() || ids.contains(id);
    kept(&filter.project_ids, &project.id)
        && (filter.client_ids.is_empty()
            || project
                .client_id
                .as_ref()
                .is_some_and(|c| filter.client_ids.contains(c)))
        && (filter.tag_ids.is_empty()
            || recording.tag_ids.iter().any(|t| filter.tag_ids.contains(t)))
        && (filter.include_archived || project.archived_at.is_none())
}

/// One row per recording and day: a recording over midnight counts on both days, as in the Day view.
fn rows(
    filter: &ReportFilter,
    (from, to): (Day, Day),
    data: &ReportData,
    now: Ms,
) -> Vec<ReportRow> {
    let projects: HashMap<&str, &Project> =
        data.projects.iter().map(|p| (p.id.as_str(), p)).collect();
    let clients: HashMap<&str, &Client> = data.clients.iter().map(|c| (c.id.as_str(), c)).collect();
    let range = (from.bounds().0, to.bounds().1);
    let mut rows = Vec::new();
    for recording in &data.recordings {
        let Some(project) = projects.get(recording.project_id.as_str()) else {
            continue;
        };
        if !wanted(filter, project, recording) {
            continue;
        }
        let Some((start, end)) = clip(recording.start, recording.end, range, now) else {
            continue;
        };
        let mut tags: Vec<&Tag> = data
            .tags
            .iter()
            .filter(|t| recording.tag_ids.contains(&t.id))
            .collect();
        tags.sort_by_key(|t| t.name.to_lowercase());
        let mut day = Day::of(start);
        while day.bounds().0 < end {
            let (part_start, part_end) = (start.max(day.bounds().0), end.min(day.bounds().1));
            rows.push(ReportRow {
                id: recording.id.clone(),
                day,
                start: part_start,
                end: part_end,
                duration_ms: part_end - part_start,
                rounded_ms: rounded(part_end - part_start, filter.rounding),
                project_id: project.id.clone(),
                project: project.name.clone(),
                client: project
                    .client_id
                    .as_deref()
                    .and_then(|id| clients.get(id))
                    .map(|c| c.name.clone()),
                color: project.color.clone(),
                shade: project.shade,
                tags: tags.iter().map(|t| t.name.clone()).collect(),
                tag_ids: tags.iter().map(|t| t.id.clone()).collect(),
                note: recording.note.trim().to_string(),
            });
            day = day.next();
        }
    }
    rows.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.id.cmp(&b.id)));
    rows
}

/// The rounding step in minutes and whether it rounds up; `None` without rounding.
pub fn rounding_step(rounding: Rounding) -> Option<(i64, bool)> {
    Some(match rounding {
        Rounding::None => return None,
        Rounding::Nearest5 => (5, false),
        Rounding::Nearest6 => (6, false),
        Rounding::Nearest10 => (10, false),
        Rounding::Nearest15 => (15, false),
        Rounding::Nearest30 => (30, false),
        Rounding::Up5 => (5, true),
        Rounding::Up6 => (6, true),
        Rounding::Up10 => (10, true),
        Rounding::Up15 => (15, true),
        Rounding::Up30 => (30, true),
    })
}

/// To the step (nearest: half up; or up); a recording longer than zero keeps at least one step.
fn rounded(duration: Ms, rounding: Rounding) -> Ms {
    let Some((minutes, up)) = rounding_step(rounding) else {
        return duration;
    };
    if duration <= 0 {
        return 0;
    }
    let step = minutes * MINUTE;
    let steps = if up {
        (duration + step - 1) / step
    } else {
        (duration + step / 2) / step
    };
    steps.max(1) * step
}

struct GroupKey {
    key: String,
    label: String,
    /// Groups sort by it: whether the group goes last, then by name or date.
    order: (bool, String, String),
    color: Option<(String, u8)>,
}

/// A tag filter also limits the tag groups to the chosen tags.
fn shown(filter: &ReportFilter, key: &GroupKey) -> bool {
    filter.group_by != Group::Tag || filter.tag_ids.is_empty() || filter.tag_ids.contains(&key.key)
}

fn groups(filter: &ReportFilter, rows: &[ReportRow]) -> Vec<ReportGroup> {
    let mut groups: Vec<(GroupKey, Vec<ReportRow>)> = Vec::new();
    for row in rows {
        for key in keys(filter.group_by, row)
            .into_iter()
            .filter(|k| shown(filter, k))
        {
            match groups.iter_mut().find(|(k, _)| k.key == key.key) {
                Some((_, rows)) => rows.push(row.clone()),
                None => groups.push((key, vec![row.clone()])),
            }
        }
    }
    groups.sort_by(|(a, _), (b, _)| a.order.cmp(&b.order).then_with(|| a.key.cmp(&b.key)));
    groups
        .into_iter()
        .map(|(key, rows)| ReportGroup {
            duration_ms: rows.iter().map(|r| r.duration_ms).sum(),
            rounded_ms: rows.iter().map(|r| r.rounded_ms).sum(),
            notes: notes(&rows),
            color: key.color.as_ref().map(|c| c.0.clone()),
            shade: key.color.map(|c| c.1),
            key: key.key,
            label: key.label,
            rows,
        })
        .collect()
}

/// The groups a row counts in: one, or one per tag when grouped by tag.
fn keys(group_by: Group, row: &ReportRow) -> Vec<GroupKey> {
    match group_by {
        Group::Project => {
            let label = match &row.client {
                Some(client) => format!("{client} / {}", row.project),
                None => row.project.clone(),
            };
            vec![GroupKey {
                key: row.project_id.clone(),
                order: (
                    row.client.is_none(),
                    row.client.as_deref().unwrap_or_default().to_lowercase(),
                    row.project.to_lowercase(),
                ),
                label,
                color: Some((row.color.clone(), row.shade)),
            }]
        }
        Group::Tag if row.tags.is_empty() => vec![GroupKey {
            key: NO_TAG_KEY.into(),
            label: NO_TAG_LABEL.into(),
            order: (true, String::new(), String::new()),
            color: None,
        }],
        Group::Tag => row
            .tags
            .iter()
            .zip(&row.tag_ids)
            .map(|(name, id)| GroupKey {
                key: id.clone(),
                label: name.clone(),
                order: (false, name.to_lowercase(), String::new()),
                color: None,
            })
            .collect(),
        Group::Day => vec![GroupKey {
            key: row.day.to_string(),
            label: day_label(row.day),
            order: (false, row.day.to_string(), String::new()),
            color: None,
        }],
        Group::Week => {
            let start = row.day.week_start();
            vec![GroupKey {
                key: start.to_string(),
                label: week_label(start),
                order: (false, start.to_string(), String::new()),
                color: None,
            }]
        }
        Group::Month => {
            let start = row.day.month_start();
            vec![GroupKey {
                key: start.date().format("%Y-%m").to_string(),
                label: start.date().format("%B %Y").to_string(),
                order: (false, start.to_string(), String::new()),
                color: None,
            }]
        }
    }
}

/// `Mon 2026-10-20`.
pub fn day_label(day: Day) -> String {
    day.date().format("%a %Y-%m-%d").to_string()
}

/// `Week 43 · 20 – 26 Oct 2026`; the month and year repeat only where the week crosses them.
fn week_label(start: Day) -> String {
    let (first, last) = (start.date(), add_days(start, 6).date());
    let span = if first.year() != last.year() {
        format!(
            "{} – {}",
            first.format("%-d %b %Y"),
            last.format("%-d %b %Y")
        )
    } else if first.month() != last.month() {
        format!("{} – {}", first.format("%-d %b"), last.format("%-d %b %Y"))
    } else {
        format!("{} – {}", first.format("%-d"), last.format("%-d %b %Y"))
    };
    format!("Week {} · {span}", first.iso_week().week())
}

fn notes(rows: &[ReportRow]) -> Vec<ReportNote> {
    let mut notes: Vec<ReportNote> = Vec::new();
    for row in rows.iter().filter(|r| !r.note.is_empty()) {
        match notes.iter_mut().find(|n| n.text == row.note) {
            Some(note) if !note.days.contains(&row.day) => note.days.push(row.day),
            Some(_) => {}
            None => notes.push(ReportNote {
                text: row.note.clone(),
                days: vec![row.day],
            }),
        }
    }
    notes
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::db::projects::ProjectInput;
    use crate::db::recordings::NewRecording;
    use crate::error::ErrorKind;
    use crate::test_support::client;

    const D: &str = "2026-10-20";

    fn day(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    fn minutes(m: i64) -> Ms {
        m * MINUTE
    }

    /// The local time `h:m` on the day `d`; kept clear of DST changes.
    fn at(d: &str, h: i64, m: i64) -> Ms {
        day(d).bounds().0 + minutes(h * 60 + m)
    }

    fn now() -> Ms {
        at("2026-12-31", 12, 0)
    }

    fn rec(
        id: &str,
        project: &str,
        start: Ms,
        end: Option<Ms>,
        note: &str,
        tag_ids: &[&str],
    ) -> Recording {
        Recording {
            id: id.into(),
            project_id: project.into(),
            start,
            end,
            note: note.into(),
            tag_ids: tag_ids.iter().map(|t| t.to_string()).collect(),
            created_at: 0,
            updated_at: 0,
        }
    }

    /// `id` on project `project` for `len` minutes from 09:00 on `d`.
    fn on(id: &str, project: &str, d: &str, len: i64) -> Recording {
        let start = at(d, 9, 0);
        rec(id, project, start, Some(start + minutes(len)), "", &[])
    }

    fn with_note(mut r: Recording, note: &str) -> Recording {
        r.note = note.into();
        r
    }

    fn with_tags(mut r: Recording, tag_ids: &[&str]) -> Recording {
        r.tag_ids = tag_ids.iter().map(|t| t.to_string()).collect();
        r
    }

    fn project(id: &str, name: &str, client_id: Option<&str>, color: &str, shade: u8) -> Project {
        Project {
            id: id.into(),
            client_id: client_id.map(String::from),
            name: name.into(),
            color: color.into(),
            shade,
            archived_at: None,
            created_at: 0,
        }
    }

    fn data(recordings: Vec<Recording>) -> ReportData {
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
                    created_at: 0,
                },
                Tag {
                    id: "t-dev".into(),
                    name: "development".into(),
                    created_at: 0,
                },
                Tag {
                    id: "t-sup".into(),
                    name: "support".into(),
                    created_at: 0,
                },
            ],
        }
    }

    fn report(
        filter: &ReportFilter,
        from: &str,
        to: &str,
        recordings: Vec<Recording>,
    ) -> ReportResult {
        build(filter, (day(from), day(to)), &data(recordings), now())
    }

    fn by(group_by: Group) -> ReportFilter {
        ReportFilter {
            group_by,
            ..ReportFilter::default()
        }
    }

    fn labels(result: &ReportResult) -> Vec<&str> {
        result.groups.iter().map(|g| g.label.as_str()).collect()
    }

    fn keys(result: &ReportResult) -> Vec<&str> {
        result.groups.iter().map(|g| g.key.as_str()).collect()
    }

    fn durations(result: &ReportResult) -> Vec<i64> {
        result
            .groups
            .iter()
            .map(|g| g.duration_ms / MINUTE)
            .collect()
    }

    #[test]
    fn presets_resolve_against_a_wednesday() {
        let today = day("2026-10-21");
        for (range, from, to) in [
            (Range::Today, "2026-10-21", "2026-10-21"),
            (Range::Yesterday, "2026-10-20", "2026-10-20"),
            (Range::ThisWeek, "2026-10-19", "2026-10-25"),
            (Range::LastWeek, "2026-10-12", "2026-10-18"),
            (Range::ThisMonth, "2026-10-01", "2026-10-31"),
            (Range::LastMonth, "2026-09-01", "2026-09-30"),
        ] {
            assert_eq!(
                resolve(&range, today).unwrap(),
                (day(from), day(to)),
                "{range:?}"
            );
        }
    }

    #[test]
    fn weeks_start_on_monday_and_end_on_sunday() {
        for today in ["2026-10-19", "2026-10-25"] {
            assert_eq!(
                resolve(&Range::ThisWeek, day(today)).unwrap(),
                (day("2026-10-19"), day("2026-10-25")),
                "{today}"
            );
        }
    }

    #[test]
    fn presets_cross_the_year_end() {
        for (range, today, from, to) in [
            (Range::LastMonth, "2026-01-15", "2025-12-01", "2025-12-31"),
            (Range::ThisWeek, "2026-01-01", "2025-12-29", "2026-01-04"),
            (Range::LastWeek, "2026-01-05", "2025-12-29", "2026-01-04"),
            (Range::Yesterday, "2026-01-01", "2025-12-31", "2025-12-31"),
        ] {
            assert_eq!(
                resolve(&range, day(today)).unwrap(),
                (day(from), day(to)),
                "{range:?} on {today}"
            );
        }
    }

    #[test]
    fn months_end_on_their_last_day_in_leap_and_common_years() {
        for (range, today, from, to) in [
            (Range::ThisMonth, "2028-02-10", "2028-02-01", "2028-02-29"),
            (Range::ThisMonth, "2026-02-10", "2026-02-01", "2026-02-28"),
            (Range::LastMonth, "2026-03-31", "2026-02-01", "2026-02-28"),
            (Range::LastMonth, "2028-03-01", "2028-02-01", "2028-02-29"),
            (Range::ThisMonth, "2026-12-31", "2026-12-01", "2026-12-31"),
        ] {
            assert_eq!(
                resolve(&range, day(today)).unwrap(),
                (day(from), day(to)),
                "{range:?} on {today}"
            );
        }
    }

    #[test]
    fn a_custom_range_resolves_to_itself_and_may_be_one_day() {
        let today = day("2026-10-21");
        for (from, to) in [("2026-10-01", "2026-10-07"), ("2026-10-05", "2026-10-05")] {
            let range = Range::Custom {
                from: day(from),
                to: day(to),
            };
            assert_eq!(resolve(&range, today).unwrap(), (day(from), day(to)));
        }
    }

    #[test]
    fn a_custom_range_that_ends_before_it_starts_is_invalid_input() {
        let range = Range::Custom {
            from: day("2026-10-07"),
            to: day("2026-10-01"),
        };
        let err = resolve(&range, day("2026-10-21")).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn ranges_have_a_tagged_camel_case_wire_shape() {
        for (range, wire) in [
            (Range::Today, json!({ "kind": "today" })),
            (Range::ThisWeek, json!({ "kind": "thisWeek" })),
            (Range::LastMonth, json!({ "kind": "lastMonth" })),
            (
                Range::Custom {
                    from: day("2026-10-01"),
                    to: day("2026-10-07"),
                },
                json!({ "kind": "custom", "from": "2026-10-01", "to": "2026-10-07" }),
            ),
        ] {
            assert_eq!(serde_json::to_value(range).unwrap(), wire, "{range:?}");
            assert_eq!(serde_json::from_value::<Range>(wire).unwrap(), range);
        }
    }

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
        assert_eq!(wire["tagOverlap"], false);
        assert_eq!(
            wire["groups"][0],
            json!({
                "key": "p-portal",
                "label": "Acme / Portal",
                "color": "blue",
                "shade": 4,
                "durationMs": minutes(90),
                "roundedMs": minutes(90),
                "notes": [{ "text": "fix login", "days": [D] }],
                "rows": [{
                    "id": "r1",
                    "day": D,
                    "start": start,
                    "end": start + minutes(90),
                    "durationMs": minutes(90),
                    "roundedMs": minutes(90),
                    "projectId": "p-portal",
                    "project": "Portal",
                    "client": "Acme",
                    "color": "blue",
                    "shade": 4,
                    "tags": [],
                    "note": "fix login",
                }],
            })
        );
    }

    #[test]
    fn projects_are_ordered_by_client_then_project_with_client_less_ones_last() {
        let result = report(
            &by(Group::Project),
            D,
            D,
            vec![
                on("r1", "p-solo", D, 10),
                on("r2", "p-site", D, 20),
                on("r3", "p-portal", D, 30),
                on("r4", "p-api", D, 40),
            ],
        );

        assert_eq!(
            labels(&result),
            ["Acme / API", "Acme / Portal", "Beta / Site", "Aardvark"]
        );
        assert_eq!(keys(&result), ["p-api", "p-portal", "p-site", "p-solo"]);
        assert_eq!(durations(&result), [40, 30, 20, 10]);
    }

    #[test]
    fn project_groups_carry_the_projects_color_and_shade() {
        let result = report(
            &by(Group::Project),
            D,
            D,
            vec![on("r1", "p-site", D, 10), on("r2", "p-solo", D, 10)],
        );

        let seen: Vec<_> = result
            .groups
            .iter()
            .map(|g| (g.color.as_deref(), g.shade))
            .collect();
        assert_eq!(seen, [(Some("green"), Some(7)), (Some("orange"), Some(2))]);
    }

    #[test]
    fn groups_and_total_sum_their_recordings() {
        let result = report(
            &by(Group::Project),
            D,
            D,
            vec![
                rec("r1", "p-portal", at(D, 9, 0), Some(at(D, 10, 0)), "", &[]),
                rec("r2", "p-portal", at(D, 11, 0), Some(at(D, 11, 30)), "", &[]),
                rec("r3", "p-site", at(D, 13, 0), Some(at(D, 13, 45)), "", &[]),
            ],
        );

        assert_eq!(durations(&result), [90, 45]);
        assert_eq!(result.total_ms, minutes(135));
        assert_eq!(result.total_rounded_ms, minutes(135));
        assert_eq!(result.groups[0].rows.len(), 2);
    }

    #[test]
    fn rows_of_a_group_are_ordered_by_start() {
        let result = report(
            &by(Group::Project),
            D,
            D,
            vec![
                rec(
                    "late",
                    "p-portal",
                    at(D, 14, 0),
                    Some(at(D, 15, 0)),
                    "",
                    &[],
                ),
                rec("early", "p-portal", at(D, 8, 0), Some(at(D, 9, 0)), "", &[]),
            ],
        );

        let ids: Vec<_> = result.groups[0]
            .rows
            .iter()
            .map(|r| r.id.as_str())
            .collect();
        assert_eq!(ids, ["early", "late"]);
    }

    #[test]
    fn a_recording_crossing_the_range_start_is_clipped() {
        let midnight = day(D).bounds().0;
        let r = rec(
            "r1",
            "p-portal",
            midnight - minutes(120),
            Some(midnight + minutes(180)),
            "",
            &[],
        );

        let result = report(&by(Group::Project), D, D, vec![r]);

        let row = &result.groups[0].rows[0];
        assert_eq!((row.start, row.end), (midnight, midnight + minutes(180)));
        assert_eq!(row.duration_ms, minutes(180));
        assert_eq!(row.day, day(D));
        assert_eq!(result.total_ms, minutes(180));
    }

    #[test]
    fn a_recording_crossing_the_range_end_is_clipped() {
        let end_of_day = day(D).bounds().1;
        let r = rec(
            "r1",
            "p-portal",
            end_of_day - minutes(60),
            Some(end_of_day + minutes(120)),
            "",
            &[],
        );

        let result = report(&by(Group::Project), D, D, vec![r]);

        let row = &result.groups[0].rows[0];
        assert_eq!((row.start, row.end), (end_of_day - minutes(60), end_of_day));
        assert_eq!(result.total_ms, minutes(60));
    }

    #[test]
    fn a_recording_over_midnight_is_split_between_the_days_it_touches() {
        let midnight = day("2026-10-21").bounds().0;
        let r = with_note(
            rec(
                "r1",
                "p-portal",
                midnight - minutes(60),
                Some(midnight + minutes(30)),
                "",
                &[],
            ),
            "night shift",
        );

        let by_day = report(&by(Group::Day), D, "2026-10-21", vec![r.clone()]);
        let by_project = report(&by(Group::Project), D, "2026-10-21", vec![r]);

        assert_eq!(keys(&by_day), [D, "2026-10-21"]);
        assert_eq!(durations(&by_day), [60, 30]);
        assert_eq!(by_day.total_ms, minutes(90));
        assert_eq!(by_project.groups.len(), 1);
        let parts: Vec<_> = by_project.groups[0]
            .rows
            .iter()
            .map(|r| (r.id.as_str(), r.day, r.duration_ms))
            .collect();
        assert_eq!(
            parts,
            [
                ("r1", day(D), minutes(60)),
                ("r1", day("2026-10-21"), minutes(30))
            ]
        );
        assert_eq!(by_project.total_ms, minutes(90));
        let notes = &by_project.groups[0].notes;
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].days, [day(D), day("2026-10-21")]);
    }

    #[test]
    fn a_running_recording_ends_at_now() {
        let start = at("2026-10-21", 9, 0);
        let r = rec("r1", "p-portal", start, None, "", &[]);

        let result = build(
            &by(Group::Project),
            (day("2026-10-21"), day("2026-10-21")),
            &data(vec![r]),
            at("2026-10-21", 11, 30),
        );

        assert_eq!(result.groups[0].rows[0].end, at("2026-10-21", 11, 30));
        assert_eq!(result.total_ms, minutes(150));
    }

    #[test]
    fn a_running_recording_stops_at_the_end_of_a_past_range() {
        let r = rec("r1", "p-portal", at(D, 22, 0), None, "", &[]);

        let result = report(&by(Group::Project), D, D, vec![r]);

        assert_eq!(result.groups[0].rows[0].end, day(D).bounds().1);
    }

    #[test]
    fn recordings_outside_the_range_are_left_out() {
        let bounds = day(D).bounds();
        let result = report(
            &by(Group::Project),
            D,
            D,
            vec![
                on("before", "p-portal", "2026-10-19", 60),
                on("after", "p-portal", "2026-10-21", 60),
                rec(
                    "ends-at-start",
                    "p-portal",
                    bounds.0 - minutes(60),
                    Some(bounds.0),
                    "",
                    &[],
                ),
                rec(
                    "starts-at-end",
                    "p-portal",
                    bounds.1,
                    Some(bounds.1 + minutes(60)),
                    "",
                    &[],
                ),
            ],
        );

        assert!(result.groups.is_empty());
        assert_eq!(result.total_ms, 0);
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
                tag_overlap: false,
            }
        );
    }

    #[test]
    fn notes_are_trimmed_distinct_and_in_order_of_first_occurrence() {
        let result = report(
            &by(Group::Project),
            "2026-10-19",
            "2026-10-21",
            vec![
                with_note(
                    rec(
                        "r1",
                        "p-portal",
                        at("2026-10-19", 9, 0),
                        Some(at("2026-10-19", 10, 0)),
                        "",
                        &[],
                    ),
                    "  fix login ",
                ),
                with_note(
                    rec(
                        "r2",
                        "p-portal",
                        at("2026-10-19", 10, 0),
                        Some(at("2026-10-19", 11, 0)),
                        "",
                        &[],
                    ),
                    "review",
                ),
                with_note(
                    rec(
                        "r3",
                        "p-portal",
                        at("2026-10-19", 11, 0),
                        Some(at("2026-10-19", 12, 0)),
                        "",
                        &[],
                    ),
                    "",
                ),
                with_note(
                    rec(
                        "r4",
                        "p-portal",
                        at("2026-10-19", 12, 0),
                        Some(at("2026-10-19", 13, 0)),
                        "",
                        &[],
                    ),
                    "   ",
                ),
                with_note(
                    rec(
                        "r5",
                        "p-portal",
                        at("2026-10-19", 13, 0),
                        Some(at("2026-10-19", 14, 0)),
                        "",
                        &[],
                    ),
                    "Fix login",
                ),
                with_note(on("r6", "p-portal", "2026-10-21", 60), "fix login"),
                with_note(on("r7", "p-portal", D, 60), "fix login"),
            ],
        );

        let notes: Vec<_> = result.groups[0]
            .notes
            .iter()
            .map(|n| (n.text.as_str(), n.days.clone()))
            .collect();
        assert_eq!(
            notes,
            [
                (
                    "fix login",
                    vec![day("2026-10-19"), day(D), day("2026-10-21")]
                ),
                ("review", vec![day("2026-10-19")]),
                ("Fix login", vec![day("2026-10-19")]),
            ]
        );
        assert_eq!(result.groups[0].rows[0].note, "fix login");
    }

    #[test]
    fn notes_are_collected_per_group() {
        let result = report(
            &by(Group::Project),
            D,
            D,
            vec![
                with_note(on("r1", "p-portal", D, 30), "a"),
                with_note(on("r2", "p-site", D, 30), "b"),
            ],
        );

        let notes: Vec<Vec<&str>> = result
            .groups
            .iter()
            .map(|g| g.notes.iter().map(|n| n.text.as_str()).collect())
            .collect();
        assert_eq!(notes, [vec!["a"], vec!["b"]]);
    }

    #[test]
    fn a_row_lists_its_tag_names_by_name() {
        let result = report(
            &by(Group::Project),
            D,
            D,
            vec![with_tags(
                on("r1", "p-portal", D, 30),
                &["t-sup", "t-admin", "t-unknown"],
            )],
        );

        assert_eq!(result.groups[0].rows[0].tags, ["admin", "support"]);
    }

    #[test]
    fn grouping_by_tag_counts_a_multi_tag_recording_in_each_group_and_once_in_the_total() {
        let result = report(
            &by(Group::Tag),
            D,
            D,
            vec![
                with_tags(on("r1", "p-portal", D, 60), &["t-admin", "t-sup"]),
                with_tags(
                    rec("r2", "p-portal", at(D, 11, 0), Some(at(D, 11, 30)), "", &[]),
                    &["t-admin"],
                ),
            ],
        );

        assert_eq!(labels(&result), ["admin", "support"]);
        assert_eq!(keys(&result), ["t-admin", "t-sup"]);
        assert_eq!(durations(&result), [90, 60]);
        assert!(result.tag_overlap);
        assert_eq!(result.total_ms, minutes(90));
        assert_eq!(result.total_rounded_ms, minutes(90));
    }

    #[test]
    fn untagged_recordings_form_a_no_tag_group_at_the_end() {
        let result = report(
            &by(Group::Tag),
            D,
            D,
            vec![
                on("r1", "p-portal", D, 20),
                with_tags(
                    rec("r2", "p-portal", at(D, 11, 0), Some(at(D, 11, 40)), "", &[]),
                    &["t-sup"],
                ),
                with_tags(
                    rec("r3", "p-portal", at(D, 12, 0), Some(at(D, 12, 10)), "", &[]),
                    &["t-admin"],
                ),
            ],
        );

        assert_eq!(labels(&result), ["admin", "support", "No tag"]);
        assert_eq!(result.groups[2].key, "untagged");
        assert_eq!(durations(&result), [10, 40, 20]);
        assert!(!result.tag_overlap);
        assert_eq!(result.groups[2].color, None);
    }

    #[test]
    fn tags_do_not_overlap_when_each_recording_has_at_most_one() {
        let result = report(
            &by(Group::Tag),
            D,
            D,
            vec![
                with_tags(on("r1", "p-portal", D, 20), &["t-admin"]),
                with_tags(
                    rec("r2", "p-portal", at(D, 11, 0), Some(at(D, 11, 30)), "", &[]),
                    &["t-sup"],
                ),
            ],
        );

        assert!(!result.tag_overlap);
    }

    #[test]
    fn other_groupings_never_report_a_tag_overlap() {
        let recordings = vec![with_tags(
            on("r1", "p-portal", D, 60),
            &["t-admin", "t-sup"],
        )];
        for group_by in [Group::Project, Group::Day, Group::Week, Group::Month] {
            let result = report(&by(group_by), D, D, recordings.clone());
            assert!(!result.tag_overlap, "{group_by:?}");
        }
    }

    #[test]
    fn the_tag_filter_keeps_recordings_with_any_chosen_tag() {
        let filter = ReportFilter {
            tag_ids: vec!["t-sup".into(), "t-dev".into()],
            ..by(Group::Project)
        };

        let result = report(
            &filter,
            D,
            D,
            vec![
                with_tags(on("r1", "p-portal", D, 10), &["t-admin", "t-sup"]),
                with_tags(
                    rec("r2", "p-portal", at(D, 11, 0), Some(at(D, 11, 20)), "", &[]),
                    &["t-dev"],
                ),
                with_tags(
                    rec("r3", "p-portal", at(D, 12, 0), Some(at(D, 12, 40)), "", &[]),
                    &["t-admin"],
                ),
                rec("r4", "p-portal", at(D, 13, 0), Some(at(D, 13, 50)), "", &[]),
            ],
        );

        assert_eq!(durations(&result), [30]);
        assert_eq!(result.total_ms, minutes(30));
    }

    #[test]
    fn the_tag_filter_limits_the_tag_groups_to_the_chosen_tags() {
        let filter = ReportFilter {
            tag_ids: vec!["t-sup".into()],
            ..by(Group::Tag)
        };

        let result = report(
            &filter,
            D,
            D,
            vec![
                with_tags(on("r1", "p-portal", D, 60), &["t-admin", "t-sup"]),
                with_tags(
                    rec("r2", "p-portal", at(D, 11, 0), Some(at(D, 11, 30)), "", &[]),
                    &["t-admin"],
                ),
            ],
        );

        assert_eq!(keys(&result), ["t-sup"]);
        assert_eq!(durations(&result), [60]);
        assert!(!result.tag_overlap);
        assert_eq!(result.total_ms, minutes(60));
    }

    #[test]
    fn the_client_filter_keeps_only_projects_of_those_clients() {
        let filter = ReportFilter {
            client_ids: vec!["c-beta".into()],
            ..by(Group::Project)
        };

        let result = report(
            &filter,
            D,
            D,
            vec![
                on("r1", "p-portal", D, 10),
                on("r2", "p-site", D, 20),
                on("r3", "p-solo", D, 30),
            ],
        );

        assert_eq!(keys(&result), ["p-site"]);
    }

    #[test]
    fn several_clients_are_alternatives() {
        let filter = ReportFilter {
            client_ids: vec!["c-beta".into(), "c-acme".into()],
            ..by(Group::Project)
        };

        let result = report(
            &filter,
            D,
            D,
            vec![
                on("r1", "p-portal", D, 10),
                on("r2", "p-site", D, 20),
                on("r3", "p-solo", D, 30),
            ],
        );

        assert_eq!(keys(&result), ["p-portal", "p-site"]);
    }

    #[test]
    fn the_project_filter_keeps_only_those_projects() {
        let filter = ReportFilter {
            project_ids: vec!["p-api".into(), "p-solo".into()],
            ..by(Group::Project)
        };

        let result = report(
            &filter,
            D,
            D,
            vec![
                on("r1", "p-portal", D, 10),
                on("r2", "p-api", D, 20),
                on("r3", "p-solo", D, 30),
            ],
        );

        assert_eq!(keys(&result), ["p-api", "p-solo"]);
    }

    #[test]
    fn client_project_and_tag_filters_narrow_together() {
        let filter = ReportFilter {
            client_ids: vec!["c-acme".into()],
            project_ids: vec!["p-portal".into(), "p-site".into()],
            tag_ids: vec!["t-admin".into()],
            ..by(Group::Project)
        };

        let result = report(
            &filter,
            D,
            D,
            vec![
                with_tags(on("keep", "p-portal", D, 10), &["t-admin"]),
                on("no-tag", "p-portal", D, 10),
                with_tags(
                    rec(
                        "other-client",
                        "p-site",
                        at(D, 11, 0),
                        Some(at(D, 12, 0)),
                        "",
                        &[],
                    ),
                    &["t-admin"],
                ),
                with_tags(
                    rec(
                        "other-project",
                        "p-api",
                        at(D, 13, 0),
                        Some(at(D, 14, 0)),
                        "",
                        &[],
                    ),
                    &["t-admin"],
                ),
            ],
        );

        let ids: Vec<_> = result
            .groups
            .iter()
            .flat_map(|g| g.rows.iter().map(|r| r.id.as_str()))
            .collect();
        assert_eq!(ids, ["keep"]);
    }

    #[test]
    fn recordings_of_archived_projects_are_left_out_unless_included() {
        let recordings = vec![on("r1", "p-portal", D, 10), on("r2", "p-old", D, 20)];

        let default = report(&by(Group::Project), D, D, recordings.clone());
        let included = report(
            &ReportFilter {
                include_archived: true,
                ..by(Group::Project)
            },
            D,
            D,
            recordings,
        );

        assert_eq!(keys(&default), ["p-portal"]);
        assert_eq!(keys(&included), ["p-old", "p-portal"]);
        assert_eq!(included.total_ms, minutes(30));
    }

    #[test]
    fn grouping_by_day_keys_groups_by_date_in_ascending_order() {
        let result = report(
            &by(Group::Day),
            "2026-10-19",
            "2026-10-21",
            vec![
                on("r1", "p-portal", "2026-10-21", 30),
                on("r2", "p-portal", "2026-10-19", 60),
                on("r3", "p-site", "2026-10-19", 15),
            ],
        );

        assert_eq!(keys(&result), ["2026-10-19", "2026-10-21"]);
        assert_eq!(labels(&result), ["Mon 2026-10-19", "Wed 2026-10-21"]);
        assert_eq!(durations(&result), [75, 30]);
        assert_eq!(result.groups[0].color, None);
    }

    #[test]
    fn grouping_by_week_keys_the_monday_and_labels_the_span() {
        let result = report(
            &by(Group::Week),
            "2026-10-19",
            "2026-10-26",
            vec![
                on("r1", "p-portal", "2026-10-24", 30),
                on("r2", "p-portal", "2026-10-19", 60),
                on("r3", "p-portal", "2026-10-26", 15),
            ],
        );

        assert_eq!(keys(&result), ["2026-10-19", "2026-10-26"]);
        assert_eq!(
            labels(&result),
            [
                "Week 43 · 19 – 25 Oct 2026",
                "Week 44 · 26 Oct – 1 Nov 2026"
            ]
        );
        assert_eq!(durations(&result), [90, 15]);
    }

    #[test]
    fn a_week_across_the_year_end_repeats_both_years() {
        let result = report(
            &by(Group::Week),
            "2025-12-29",
            "2026-01-04",
            vec![
                on("r1", "p-portal", "2025-12-30", 30),
                on("r2", "p-portal", "2026-01-02", 30),
            ],
        );

        assert_eq!(keys(&result), ["2025-12-29"]);
        assert_eq!(labels(&result), ["Week 1 · 29 Dec 2025 – 4 Jan 2026"]);
        assert_eq!(durations(&result), [60]);
    }

    #[test]
    fn grouping_by_month_keys_year_and_month() {
        let result = report(
            &by(Group::Month),
            "2026-10-31",
            "2026-11-01",
            vec![
                on("r1", "p-portal", "2026-11-01", 30),
                on("r2", "p-portal", "2026-10-31", 60),
            ],
        );

        assert_eq!(keys(&result), ["2026-10", "2026-11"]);
        assert_eq!(labels(&result), ["October 2026", "November 2026"]);
        assert_eq!(durations(&result), [60, 30]);
    }

    #[test]
    fn day_labels_name_the_weekday() {
        assert_eq!(day_label(day("2026-10-20")), "Tue 2026-10-20");
        assert_eq!(day_label(day("2026-10-25")), "Sun 2026-10-25");
    }

    fn rounded_for(rounding: Rounding, len: Ms) -> Ms {
        let start = at(D, 9, 0);
        let filter = ReportFilter {
            rounding,
            ..by(Group::Project)
        };
        let result = report(
            &filter,
            D,
            D,
            vec![rec("r1", "p-portal", start, Some(start + len), "", &[])],
        );
        assert_eq!(result.groups[0].rows[0].duration_ms, len);
        result.groups[0].rows[0].rounded_ms
    }

    #[test]
    fn rounding_applies_the_step_per_recording() {
        for (rounding, len_ms, expected) in [
            (Rounding::None, minutes(7), minutes(7)),
            (Rounding::None, 7 * MINUTE + 123, 7 * MINUTE + 123),
            (Rounding::Nearest15, minutes(7), minutes(15)),
            (Rounding::Nearest15, minutes(1), minutes(15)),
            (Rounding::Nearest15, minutes(22), minutes(15)),
            (Rounding::Nearest15, 22 * MINUTE + 29_999, minutes(15)),
            (Rounding::Nearest15, minutes(23), minutes(30)),
            (Rounding::Nearest15, 22 * MINUTE + 30_000, minutes(30)),
            (Rounding::Nearest15, 7 * MINUTE + 30_000, minutes(15)),
            (Rounding::Nearest15, minutes(60), minutes(60)),
            (Rounding::Nearest5, minutes(12), minutes(10)),
            (Rounding::Nearest5, minutes(13), minutes(15)),
            (Rounding::Nearest6, minutes(2), minutes(6)),
            (Rounding::Nearest6, minutes(8), minutes(6)),
            (Rounding::Nearest6, minutes(9), minutes(12)),
            (Rounding::Nearest10, minutes(14), minutes(10)),
            (Rounding::Nearest10, minutes(15), minutes(20)),
            (Rounding::Nearest30, minutes(44), minutes(30)),
            (Rounding::Nearest30, minutes(45), minutes(60)),
            (Rounding::Up5, minutes(6), minutes(10)),
            (Rounding::Up5, minutes(5), minutes(5)),
            (Rounding::Up6, minutes(7), minutes(12)),
            (Rounding::Up10, minutes(11), minutes(20)),
            (Rounding::Up15, minutes(1), minutes(15)),
            (Rounding::Up15, minutes(15), minutes(15)),
            (Rounding::Up15, minutes(16), minutes(30)),
            (Rounding::Up15, 15 * MINUTE + 1, minutes(30)),
            (Rounding::Up30, minutes(31), minutes(60)),
        ] {
            assert_eq!(
                rounded_for(rounding, len_ms),
                expected,
                "{rounding:?} {len_ms} ms"
            );
        }
    }

    #[test]
    fn totals_sum_the_rounded_recordings_not_the_rounded_sum() {
        let filter = ReportFilter {
            rounding: Rounding::Nearest15,
            ..by(Group::Project)
        };
        let result = report(
            &filter,
            D,
            D,
            vec![
                on("r1", "p-portal", D, 7),
                rec("r2", "p-portal", at(D, 10, 0), Some(at(D, 10, 7)), "", &[]),
                rec("r3", "p-site", at(D, 11, 0), Some(at(D, 11, 40)), "", &[]),
            ],
        );

        assert_eq!(result.groups[0].duration_ms, minutes(14));
        assert_eq!(result.groups[0].rounded_ms, minutes(30));
        assert_eq!(result.groups[1].rounded_ms, minutes(45));
        assert_eq!(result.total_ms, minutes(54));
        assert_eq!(result.total_rounded_ms, minutes(75));
    }

    #[test]
    fn a_multi_tag_recording_is_rounded_once_in_the_total() {
        let filter = ReportFilter {
            rounding: Rounding::Up15,
            ..by(Group::Tag)
        };
        let result = report(
            &filter,
            D,
            D,
            vec![with_tags(
                on("r1", "p-portal", D, 10),
                &["t-admin", "t-sup"],
            )],
        );

        assert_eq!(result.groups[0].rounded_ms, minutes(15));
        assert_eq!(result.groups[1].rounded_ms, minutes(15));
        assert_eq!(result.total_rounded_ms, minutes(15));
    }

    #[test]
    fn a_clipped_recording_is_rounded_from_its_clipped_length() {
        let midnight = day(D).bounds().0;
        let r = rec(
            "r1",
            "p-portal",
            midnight - minutes(600),
            Some(midnight + minutes(7)),
            "",
            &[],
        );
        let filter = ReportFilter {
            rounding: Rounding::Nearest15,
            ..by(Group::Project)
        };

        let result = report(&filter, D, D, vec![r]);

        assert_eq!(result.total_ms, minutes(7));
        assert_eq!(result.total_rounded_ms, minutes(15));
    }

    #[test]
    fn rounding_steps_name_minutes_and_direction() {
        for (rounding, expected) in [
            (Rounding::None, None),
            (Rounding::Nearest5, Some((5, false))),
            (Rounding::Nearest6, Some((6, false))),
            (Rounding::Nearest10, Some((10, false))),
            (Rounding::Nearest15, Some((15, false))),
            (Rounding::Nearest30, Some((30, false))),
            (Rounding::Up5, Some((5, true))),
            (Rounding::Up6, Some((6, true))),
            (Rounding::Up10, Some((10, true))),
            (Rounding::Up15, Some((15, true))),
            (Rounding::Up30, Some((30, true))),
        ] {
            assert_eq!(rounding_step(rounding), expected, "{rounding:?}");
        }
    }

    #[test]
    fn durations_are_exact_to_the_millisecond() {
        let start = at(D, 9, 0) + 17;
        let r = rec("r1", "p-portal", start, Some(start + 3_723_456), "", &[]);

        let result = report(&by(Group::Project), D, D, vec![r]);

        assert_eq!(result.total_ms, 3_723_456);
        assert_eq!(result.groups[0].rows[0].rounded_ms, 3_723_456);
    }

    #[test]
    fn recordings_of_unknown_projects_are_left_out() {
        let result = report(&by(Group::Project), D, D, vec![on("r1", "p-gone", D, 30)]);

        assert!(result.groups.is_empty());
        assert_eq!(result.total_ms, 0);
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
                start,
                end: Some(start + minutes(90)),
                note: " fix login ".into(),
                tag_ids: vec![admin.id.clone()],
            },
        )
        .await
        .unwrap();
        recordings::insert(
            &db,
            NewRecording {
                id: "r2".into(),
                project_id: portal.id.clone(),
                start: at("2026-10-20", 9, 0),
                end: Some(at("2026-10-20", 10, 0)),
                note: String::new(),
                tag_ids: vec![],
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
        assert_eq!(result.groups[0].rows[0].tags, ["admin"]);
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
