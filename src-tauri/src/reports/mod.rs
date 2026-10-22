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
use crate::db::recordings::{Recording, Span};
use crate::db::tags::Tag;
use crate::db::time::{Day, Ms, clip};
use crate::db::{Db, clients, projects, recordings, tags};
use crate::error::{AppError, AppResult};
use crate::store::settings::{Group, Rounding};

const MINUTE: Ms = 60_000;
const NO_TAG_KEY: &str = "untagged";
const NO_TAG_LABEL: &str = "Untagged";

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
    pub total_ms: i64,
    pub total_rounded_ms: i64,
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

/// What a range steps by: a calendar month, a Monday-to-Sunday week, or its own number of days.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    Month,
    Week,
    Days(i64),
}

fn unit((from, to): (Day, Day)) -> Unit {
    if from == from.month_start() && month(from).1 == to {
        Unit::Month
    } else if from == from.week_start() && add_days(from, 6) == to {
        Unit::Week
    } else {
        Unit::Days((to.date() - from.date()).num_days() + 1)
    }
}

const PRESETS: [Range; 6] = [
    Range::Today,
    Range::Yesterday,
    Range::ThisWeek,
    Range::LastWeek,
    Range::ThisMonth,
    Range::LastMonth,
];

/// The period of `range`'s unit `by` steps away: from the range itself, or with `anchor` from the period
/// that holds it (for days, the ones ending on it). A result that matches a preset is that preset.
pub fn shift(range: &Range, anchor: Option<Day>, by: i32, today: Day) -> AppResult<Range> {
    let current = resolve(range, today)?;
    let (from, to) = match unit(current) {
        Unit::Month => shift_month(anchor.map_or(current.0, |d| d.month_start()), by),
        Unit::Week => shift_week(anchor.map_or(current.0, |d| d.week_start()), by),
        Unit::Days(n) => shift_days(anchor.unwrap_or(current.1), n, by),
    };
    let preset = PRESETS
        .into_iter()
        .find(|p| resolve(p, today).is_ok_and(|r| r == (from, to)));
    Ok(preset.unwrap_or(Range::Custom { from, to }))
}

/// The month `by` months from the one starting on `start`.
fn shift_month(start: Day, by: i32) -> (Day, Day) {
    month(add_months(start, by))
}

/// The week `by` weeks from the one starting on `start`.
fn shift_week(start: Day, by: i32) -> (Day, Day) {
    let start = add_days(start, 7 * i64::from(by));
    (start, add_days(start, 6))
}

/// The `n` days ending on `end`, moved `by` times their length.
fn shift_days(end: Day, n: i64, by: i32) -> (Day, Day) {
    let end = add_days(end, n * i64::from(by));
    (add_days(end, 1 - n), end)
}

fn add_days(day: Day, days: i64) -> Day {
    let date = if days >= 0 {
        day.date() + Days::new(days.unsigned_abs())
    } else {
        day.date() - Days::new(days.unsigned_abs())
    };
    Day::from_date(date)
}

fn add_months(day: Day, months: i32) -> Day {
    let date = if months >= 0 {
        day.date() + Months::new(months.unsigned_abs())
    } else {
        day.date() - Months::new(months.unsigned_abs())
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
    ReportResult {
        filter: filter.clone(),
        from,
        to,
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
            || recording
                .tag_id
                .as_ref()
                .is_some_and(|t| filter.tag_ids.contains(t)))
        && (filter.include_archived || project.archived_at.is_none())
}

/// One row per recording and day, with its spans of the day: a span over midnight counts on both days, as
/// in the Day view.
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
        let tag: Option<&Tag> = recording
            .tag_id
            .as_ref()
            .and_then(|id| data.tags.iter().find(|t| &t.id == id));
        for (day, spans) in parts_by_day(recording, range, now) {
            let (Some(first), Some(last)) = (spans.first(), spans.last()) else {
                continue;
            };
            let (start, end) = (first.start, last.end);
            let duration: Ms = spans.iter().map(|s| s.end - s.start).sum();
            rows.push(ReportRow {
                id: recording.id.clone(),
                day,
                start,
                end,
                spans,
                duration_ms: duration,
                rounded_ms: rounded(duration, filter.rounding),
                project_id: project.id.clone(),
                project: project.name.clone(),
                client: project
                    .client_id
                    .as_deref()
                    .and_then(|id| clients.get(id))
                    .map(|c| c.name.clone()),
                color: project.color.clone(),
                shade: project.shade,
                tag: tag.map(|t| t.name.clone()),
                tag_id: tag.map(|t| t.id.clone()),
                note: recording.note.trim().to_string(),
            });
        }
    }
    rows.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.id.cmp(&b.id)));
    rows
}

/// The parts of `recording`'s spans inside `range`, cut at midnight and gathered per day, by day.
fn parts_by_day(recording: &Recording, range: (Ms, Ms), now: Ms) -> Vec<(Day, Vec<Span>)> {
    let mut days: Vec<(Day, Vec<Span>)> = Vec::new();
    for span in &recording.spans {
        let Some((start, end)) = clip(span.start, span.end, range, now) else {
            continue;
        };
        let mut day = Day::of(start);
        while day.bounds().0 < end {
            let part = Span {
                start: start.max(day.bounds().0),
                end: end.min(day.bounds().1),
            };
            match days.iter_mut().find(|(d, _)| *d == day) {
                Some((_, spans)) => spans.push(part),
                None => days.push((day, vec![part])),
            }
            day = day.next();
        }
    }
    days.sort_by_key(|(day, _)| *day);
    days
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

/// Each row counts in exactly one group, so the groups add up to the total.
fn groups(filter: &ReportFilter, rows: &[ReportRow]) -> Vec<ReportGroup> {
    let mut groups: Vec<(GroupKey, Vec<ReportRow>)> = Vec::new();
    for row in rows {
        let key = key(filter.group_by, row);
        match groups.iter_mut().find(|(k, _)| k.key == key.key) {
            Some((_, rows)) => rows.push(row.clone()),
            None => groups.push((key, vec![row.clone()])),
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

fn key(group_by: Group, row: &ReportRow) -> GroupKey {
    match group_by {
        Group::Project => {
            let label = match &row.client {
                Some(client) => format!("{client} / {}", row.project),
                None => row.project.clone(),
            };
            GroupKey {
                key: row.project_id.clone(),
                order: (
                    row.client.is_none(),
                    row.client.as_deref().unwrap_or_default().to_lowercase(),
                    row.project.to_lowercase(),
                ),
                label,
                color: Some((row.color.clone(), row.shade)),
            }
        }
        Group::Tag => match (&row.tag_id, &row.tag) {
            (Some(id), Some(name)) => GroupKey {
                key: id.clone(),
                label: name.clone(),
                order: (false, name.to_lowercase(), String::new()),
                color: None,
            },
            _ => GroupKey {
                key: NO_TAG_KEY.into(),
                label: NO_TAG_LABEL.into(),
                order: (true, String::new(), String::new()),
                color: None,
            },
        },
        Group::Day => GroupKey {
            key: row.day.to_string(),
            label: day_label(row.day),
            order: (false, row.day.to_string(), String::new()),
            color: None,
        },
        Group::Week => {
            let start = row.day.week_start();
            GroupKey {
                key: start.to_string(),
                label: week_label(start),
                order: (false, start.to_string(), String::new()),
                color: None,
            }
        }
        Group::Month => {
            let start = row.day.month_start();
            GroupKey {
                key: start.date().format("%Y-%m").to_string(),
                label: start.date().format("%B %Y").to_string(),
                order: (false, start.to_string(), String::new()),
                color: None,
            }
        }
    }
}

/// `Mon 2026-10-20`.
pub fn day_label(day: Day) -> String {
    day.date().format("%a %Y-%m-%d").to_string()
}

/// `Week 43 · 20 – 26 Oct 2026`; the month and year repeat only where the week crosses them.
pub(crate) fn week_label(start: Day) -> String {
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
    use crate::db::recordings::{NewRecording, RecordingSpan};
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
        tag_id: Option<&str>,
    ) -> Recording {
        spanned(id, project, &[(start, end)], note, tag_id)
    }

    fn spanned(
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
    fn in_spans(id: &str, project: &str, parts: &[(&str, i64, i64, i64, i64)]) -> Recording {
        let spans: Vec<(Ms, Option<Ms>)> = parts
            .iter()
            .map(|&(d, sh, sm, eh, em)| (at(d, sh, sm), Some(at(d, eh, em))))
            .collect();
        spanned(id, project, &spans, "", None)
    }

    /// `id` on project `project` for `len` minutes from 09:00 on `d`.
    fn on(id: &str, project: &str, d: &str, len: i64) -> Recording {
        let start = at(d, 9, 0);
        rec(id, project, start, Some(start + minutes(len)), "", None)
    }

    fn with_note(mut r: Recording, note: &str) -> Recording {
        r.note = note.into();
        r
    }

    fn with_tag(mut r: Recording, tag_id: &str) -> Recording {
        r.tag_id = Some(tag_id.into());
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
        assert!(wire.get("tagOverlap").is_none());
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
                    "spans": [{ "start": start, "end": start + minutes(90) }],
                    "durationMs": minutes(90),
                    "roundedMs": minutes(90),
                    "projectId": "p-portal",
                    "project": "Portal",
                    "client": "Acme",
                    "color": "blue",
                    "shade": 4,
                    "tag": null,
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
                    None,
                ),
                rec(
                    "early",
                    "p-portal",
                    at(D, 8, 0),
                    Some(at(D, 9, 0)),
                    "",
                    None,
                ),
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
            None,
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
            None,
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
                None,
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
        let r = rec("r1", "p-portal", start, None, "", None);

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
        let r = rec("r1", "p-portal", at(D, 22, 0), None, "", None);

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
                    None,
                ),
                rec(
                    "starts-at-end",
                    "p-portal",
                    bounds.1,
                    Some(bounds.1 + minutes(60)),
                    "",
                    None,
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
                        None,
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
                        None,
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
                        None,
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
                        None,
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
                        None,
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
    fn a_row_carries_its_tag_name_and_none_when_the_tag_is_gone() {
        let result = report(
            &by(Group::Project),
            D,
            D,
            vec![
                with_tag(on("r1", "p-portal", D, 30), "t-sup"),
                with_tag(
                    rec(
                        "r2",
                        "p-portal",
                        at(D, 11, 0),
                        Some(at(D, 11, 30)),
                        "",
                        None,
                    ),
                    "t-unknown",
                ),
            ],
        );

        let tags: Vec<_> = result.groups[0]
            .rows
            .iter()
            .map(|r| r.tag.as_deref())
            .collect();
        assert_eq!(tags, [Some("support"), None]);
    }

    #[test]
    fn grouping_by_tag_counts_each_recording_once_and_the_groups_sum_to_the_total() {
        let result = report(
            &by(Group::Tag),
            D,
            D,
            vec![
                with_tag(on("r1", "p-portal", D, 60), "t-sup"),
                with_tag(
                    rec(
                        "r2",
                        "p-portal",
                        at(D, 11, 0),
                        Some(at(D, 11, 30)),
                        "",
                        None,
                    ),
                    "t-admin",
                ),
                with_tag(
                    rec(
                        "r3",
                        "p-portal",
                        at(D, 12, 0),
                        Some(at(D, 12, 30)),
                        "",
                        None,
                    ),
                    "t-admin",
                ),
            ],
        );

        assert_eq!(labels(&result), ["admin", "support"]);
        assert_eq!(keys(&result), ["t-admin", "t-sup"]);
        assert_eq!(durations(&result), [60, 60]);
        assert_eq!(result.total_ms, minutes(120));
        assert_eq!(result.total_rounded_ms, minutes(120));
    }

    #[test]
    fn untagged_recordings_form_an_untagged_group_at_the_end() {
        let result = report(
            &by(Group::Tag),
            D,
            D,
            vec![
                on("r1", "p-portal", D, 20),
                with_tag(
                    rec(
                        "r2",
                        "p-portal",
                        at(D, 11, 0),
                        Some(at(D, 11, 40)),
                        "",
                        None,
                    ),
                    "t-sup",
                ),
                with_tag(
                    rec(
                        "r3",
                        "p-portal",
                        at(D, 12, 0),
                        Some(at(D, 12, 10)),
                        "",
                        None,
                    ),
                    "t-admin",
                ),
            ],
        );

        assert_eq!(labels(&result), ["admin", "support", "Untagged"]);
        assert_eq!(result.groups[2].key, "untagged");
        assert_eq!(durations(&result), [10, 40, 20]);
        assert_eq!(result.groups[2].color, None);
        assert_eq!(result.total_ms, minutes(70));
    }

    #[test]
    fn a_recording_whose_tag_is_gone_counts_as_untagged() {
        let result = report(
            &by(Group::Tag),
            D,
            D,
            vec![with_tag(on("r1", "p-portal", D, 20), "t-unknown")],
        );

        assert_eq!(labels(&result), ["Untagged"]);
        assert_eq!(durations(&result), [20]);
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
                with_tag(on("r1", "p-portal", D, 10), "t-sup"),
                with_tag(
                    rec(
                        "r2",
                        "p-portal",
                        at(D, 11, 0),
                        Some(at(D, 11, 20)),
                        "",
                        None,
                    ),
                    "t-dev",
                ),
                with_tag(
                    rec(
                        "r3",
                        "p-portal",
                        at(D, 12, 0),
                        Some(at(D, 12, 40)),
                        "",
                        None,
                    ),
                    "t-admin",
                ),
                rec(
                    "r4",
                    "p-portal",
                    at(D, 13, 0),
                    Some(at(D, 13, 50)),
                    "",
                    None,
                ),
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
                with_tag(on("r1", "p-portal", D, 60), "t-sup"),
                with_tag(
                    rec(
                        "r2",
                        "p-portal",
                        at(D, 11, 0),
                        Some(at(D, 11, 30)),
                        "",
                        None,
                    ),
                    "t-admin",
                ),
                rec(
                    "r3",
                    "p-portal",
                    at(D, 13, 0),
                    Some(at(D, 13, 30)),
                    "",
                    None,
                ),
            ],
        );

        assert_eq!(keys(&result), ["t-sup"]);
        assert_eq!(durations(&result), [60]);
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
                with_tag(on("keep", "p-portal", D, 10), "t-admin"),
                on("no-tag", "p-portal", D, 10),
                with_tag(
                    rec(
                        "other-client",
                        "p-site",
                        at(D, 11, 0),
                        Some(at(D, 12, 0)),
                        "",
                        None,
                    ),
                    "t-admin",
                ),
                with_tag(
                    rec(
                        "other-project",
                        "p-api",
                        at(D, 13, 0),
                        Some(at(D, 14, 0)),
                        "",
                        None,
                    ),
                    "t-admin",
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
            vec![rec("r1", "p-portal", start, Some(start + len), "", None)],
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
                rec("r2", "p-portal", at(D, 10, 0), Some(at(D, 10, 7)), "", None),
                rec("r3", "p-site", at(D, 11, 0), Some(at(D, 11, 40)), "", None),
            ],
        );

        assert_eq!(result.groups[0].duration_ms, minutes(14));
        assert_eq!(result.groups[0].rounded_ms, minutes(30));
        assert_eq!(result.groups[1].rounded_ms, minutes(45));
        assert_eq!(result.total_ms, minutes(54));
        assert_eq!(result.total_rounded_ms, minutes(75));
    }

    #[test]
    fn tag_groups_round_their_recordings_and_sum_to_the_rounded_total() {
        let filter = ReportFilter {
            rounding: Rounding::Up15,
            ..by(Group::Tag)
        };
        let result = report(
            &filter,
            D,
            D,
            vec![
                with_tag(on("r1", "p-portal", D, 10), "t-admin"),
                with_tag(
                    rec(
                        "r2",
                        "p-portal",
                        at(D, 11, 0),
                        Some(at(D, 11, 20)),
                        "",
                        None,
                    ),
                    "t-sup",
                ),
            ],
        );

        assert_eq!(result.groups[0].rounded_ms, minutes(15));
        assert_eq!(result.groups[1].rounded_ms, minutes(30));
        assert_eq!(result.total_rounded_ms, minutes(45));
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
            None,
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
        let r = rec("r1", "p-portal", start, Some(start + 3_723_456), "", None);

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

    #[test]
    fn a_recording_with_several_spans_on_a_day_is_one_row_with_all_of_them() {
        let r = with_note(
            in_spans(
                "r1",
                "p-portal",
                &[(D, 9, 0, 10, 0), (D, 11, 0, 11, 30), (D, 16, 5, 17, 5)],
            ),
            "tooling",
        );

        let result = report(&by(Group::Project), D, D, vec![r]);

        let rows = &result.groups[0].rows;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "r1");
        assert_eq!(
            rows[0].spans,
            [
                Span {
                    start: at(D, 9, 0),
                    end: at(D, 10, 0)
                },
                Span {
                    start: at(D, 11, 0),
                    end: at(D, 11, 30)
                },
                Span {
                    start: at(D, 16, 5),
                    end: at(D, 17, 5)
                },
            ]
        );
        assert_eq!((rows[0].start, rows[0].end), (at(D, 9, 0), at(D, 17, 5)));
        assert_eq!(rows[0].duration_ms, minutes(150));
        assert_eq!(result.total_ms, minutes(150));
        assert_eq!(result.groups[0].notes.len(), 1);
    }

    #[test]
    fn a_recording_with_spans_on_two_days_is_one_row_per_day_with_that_days_parts() {
        let r = in_spans(
            "r1",
            "p-portal",
            &[(D, 9, 0, 10, 0), ("2026-10-21", 14, 0, 14, 45)],
        );

        let result = report(&by(Group::Project), D, "2026-10-21", vec![r.clone()]);
        let one_day = report(&by(Group::Project), "2026-10-21", "2026-10-21", vec![r]);

        let rows: Vec<_> = result.groups[0]
            .rows
            .iter()
            .map(|r| (r.day, r.start, r.end, r.spans.len(), r.duration_ms))
            .collect();
        assert_eq!(
            rows,
            [
                (day(D), at(D, 9, 0), at(D, 10, 0), 1, minutes(60)),
                (
                    day("2026-10-21"),
                    at("2026-10-21", 14, 0),
                    at("2026-10-21", 14, 45),
                    1,
                    minutes(45)
                ),
            ]
        );
        assert_eq!(result.total_ms, minutes(105));
        assert_eq!(one_day.total_ms, minutes(45));
    }

    #[test]
    fn a_span_over_midnight_and_a_later_span_share_the_second_days_row() {
        let midnight = day("2026-10-21").bounds().0;
        let r = spanned(
            "r1",
            "p-portal",
            &[
                (at(D, 22, 0), Some(midnight + minutes(60))),
                (at("2026-10-21", 9, 0), Some(at("2026-10-21", 10, 0))),
            ],
            "",
            None,
        );

        let result = report(&by(Group::Day), D, "2026-10-21", vec![r]);

        let first = &result.groups[0].rows[0];
        assert_eq!(
            first.spans,
            [Span {
                start: at(D, 22, 0),
                end: midnight
            }]
        );
        assert_eq!(first.duration_ms, minutes(120));
        let second = &result.groups[1].rows[0];
        assert_eq!(
            second.spans,
            [
                Span {
                    start: midnight,
                    end: midnight + minutes(60)
                },
                Span {
                    start: at("2026-10-21", 9, 0),
                    end: at("2026-10-21", 10, 0)
                },
            ]
        );
        assert_eq!(
            (second.start, second.end),
            (midnight, at("2026-10-21", 10, 0))
        );
        assert_eq!(second.duration_ms, minutes(120));
    }

    #[test]
    fn an_open_span_after_closed_ones_ends_at_now() {
        let r = spanned(
            "r1",
            "p-portal",
            &[(at(D, 9, 0), Some(at(D, 10, 0))), (at(D, 11, 0), None)],
            "",
            None,
        );

        let result = build(
            &by(Group::Project),
            (day(D), day(D)),
            &data(vec![r]),
            at(D, 11, 20),
        );

        let row = &result.groups[0].rows[0];
        assert_eq!(row.end, at(D, 11, 20));
        assert_eq!(row.spans.len(), 2);
        assert_eq!(row.duration_ms, minutes(80));
    }

    #[test]
    fn rounding_applies_to_a_rows_summed_spans_not_to_each_span() {
        let filter = ReportFilter {
            rounding: Rounding::Nearest15,
            ..by(Group::Project)
        };
        let one = in_spans("one", "p-portal", &[(D, 9, 0, 9, 7), (D, 10, 0, 10, 7)]);
        let two = vec![
            in_spans("a", "p-portal", &[(D, 9, 0, 9, 7)]),
            in_spans("b", "p-portal", &[(D, 10, 0, 10, 7)]),
        ];

        let single = report(&filter, D, D, vec![one]);
        let separate = report(&filter, D, D, two);

        assert_eq!(single.groups[0].rows[0].duration_ms, minutes(14));
        assert_eq!(single.total_rounded_ms, minutes(15));
        assert_eq!(separate.total_ms, minutes(14));
        assert_eq!(separate.total_rounded_ms, minutes(30));
    }

    #[test]
    fn each_day_of_a_multi_span_recording_is_rounded_on_its_own() {
        let filter = ReportFilter {
            rounding: Rounding::Nearest15,
            ..by(Group::Project)
        };
        let r = in_spans(
            "r1",
            "p-portal",
            &[(D, 9, 0, 9, 7), ("2026-10-21", 9, 0, 9, 7)],
        );

        let result = report(&filter, D, "2026-10-21", vec![r]);

        let rounded: Vec<_> = result.groups[0].rows.iter().map(|r| r.rounded_ms).collect();
        assert_eq!(rounded, [minutes(15), minutes(15)]);
        assert_eq!(result.total_rounded_ms, minutes(30));
    }

    #[test]
    fn notes_follow_the_rows_by_their_first_span() {
        let result = report(
            &by(Group::Project),
            D,
            D,
            vec![
                with_note(
                    in_spans("late", "p-portal", &[(D, 12, 0, 12, 30)]),
                    "afternoon",
                ),
                with_note(
                    in_spans("split", "p-portal", &[(D, 9, 0, 9, 30), (D, 16, 0, 17, 0)]),
                    "tooling",
                ),
                with_note(
                    in_spans("middle", "p-portal", &[(D, 10, 0, 11, 0)]),
                    "review",
                ),
            ],
        );

        let notes: Vec<_> = result.groups[0]
            .notes
            .iter()
            .map(|n| n.text.as_str())
            .collect();
        assert_eq!(notes, ["tooling", "review", "afternoon"]);
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

    fn custom(from: &str, to: &str) -> Range {
        Range::Custom {
            from: day(from),
            to: day(to),
        }
    }

    #[test]
    fn shifting_a_preset_lands_on_the_neighbouring_preset_when_there_is_one() {
        for (range, by, expected) in [
            (Range::Today, -1, Range::Yesterday),
            (Range::ThisWeek, -1, Range::LastWeek),
            (Range::LastWeek, 1, Range::ThisWeek),
            (Range::ThisMonth, -1, Range::LastMonth),
            (Range::LastMonth, 1, Range::ThisMonth),
        ] {
            assert_eq!(
                shift(&range, None, by, day(D)).unwrap(),
                expected,
                "{range:?} {by}"
            );
        }
    }

    #[test]
    fn shifting_a_preset_past_the_presets_gives_a_custom_range() {
        for (range, by, expected) in [
            (Range::Today, 1, custom("2026-10-21", "2026-10-21")),
            (Range::ThisMonth, -2, custom("2026-08-01", "2026-08-31")),
            (Range::ThisMonth, 1, custom("2026-11-01", "2026-11-30")),
            (Range::ThisWeek, 1, custom("2026-10-26", "2026-11-01")),
        ] {
            assert_eq!(
                shift(&range, None, by, day(D)).unwrap(),
                expected,
                "{range:?} {by}"
            );
        }
    }

    #[test]
    fn a_custom_range_moves_by_its_own_length_in_days() {
        for (by, expected) in [
            (1, custom("2026-10-11", "2026-10-20")),
            (-1, custom("2026-09-21", "2026-09-30")),
            (2, custom("2026-10-21", "2026-10-30")),
        ] {
            assert_eq!(
                shift(&custom("2026-10-01", "2026-10-10"), None, by, day(D)).unwrap(),
                expected,
                "{by}"
            );
        }
    }

    #[test]
    fn a_custom_whole_calendar_month_steps_as_a_month() {
        let february = custom("2026-02-01", "2026-02-28");

        for (by, expected) in [
            (1, custom("2026-03-01", "2026-03-31")),
            (-1, custom("2026-01-01", "2026-01-31")),
        ] {
            assert_eq!(
                shift(&february, None, by, day(D)).unwrap(),
                expected,
                "{by}"
            );
        }
    }

    #[test]
    fn a_custom_monday_to_sunday_range_steps_as_a_week() {
        let week = custom("2026-01-05", "2026-01-11");

        for (by, expected) in [
            (1, custom("2026-01-12", "2026-01-18")),
            (-1, custom("2025-12-29", "2026-01-04")),
        ] {
            assert_eq!(shift(&week, None, by, day(D)).unwrap(), expected, "{by}");
        }
    }

    #[test]
    fn a_custom_range_that_matches_a_preset_becomes_that_preset() {
        let last_week = custom("2026-10-12", "2026-10-18");

        assert_eq!(shift(&last_week, None, 1, day(D)).unwrap(), Range::ThisWeek);
    }

    #[test]
    fn an_anchor_with_no_step_gives_the_period_of_the_ranges_unit_that_holds_it() {
        let anchor = Some(day("2026-10-22"));

        for (range, expected) in [
            (Range::ThisWeek, Range::ThisWeek),
            (Range::LastMonth, Range::ThisMonth),
            (Range::Yesterday, custom("2026-10-22", "2026-10-22")),
            (custom("2026-02-01", "2026-02-28"), Range::ThisMonth),
            (custom("2026-01-05", "2026-01-11"), Range::ThisWeek),
        ] {
            assert_eq!(
                shift(&range, anchor, 0, day(D)).unwrap(),
                expected,
                "{range:?}"
            );
        }
    }

    #[test]
    fn an_anchor_on_today_with_a_one_day_range_gives_today() {
        assert_eq!(
            shift(&Range::Yesterday, Some(day(D)), 0, day(D)).unwrap(),
            Range::Today
        );
    }

    #[test]
    fn an_anchor_with_a_custom_day_range_gives_the_days_ending_on_it() {
        assert_eq!(
            shift(&custom("2026-09-01", "2026-09-10"), Some(day(D)), 0, day(D)).unwrap(),
            custom("2026-10-11", "2026-10-20")
        );
    }

    #[test]
    fn shifting_an_invalid_custom_range_is_invalid_input() {
        let err = shift(&custom("2026-10-07", "2026-10-01"), None, 1, day(D)).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }
}
