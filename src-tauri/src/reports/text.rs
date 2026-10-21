//! The report as plain text for a timesheet:
//!
//! ```text
//! Mon 2026-10-20
//!
//! Acme / Portal  3:30 h
//!   - fix login
//!
//! Total  3:30 h
//! ```

use std::fmt::Write;

use super::{ReportNote, ReportResult, day_label, rounding_step};
use crate::db::time::{Day, Ms};
use crate::store::settings::Group;

/// Up to a week, a note's days read as weekdays; longer ranges name the date.
const WEEKDAYS_UP_TO: usize = 7;

pub fn text(result: &ReportResult) -> String {
    let days = Day::range(&result.from, &result.to).len();
    let mut out = header(result, days);
    out.push_str("\n\n");
    if result.groups.is_empty() {
        out.push_str("No recordings.\n");
        return out;
    }
    let note_days = days > 1 && result.filter.group_by != Group::Day;
    for group in &result.groups {
        let _ = writeln!(out, "{}  {}", group.label, hours(group.rounded_ms));
        for note in &group.notes {
            let suffix = if note_days {
                format!(" ({})", days_of(note, days))
            } else {
                String::new()
            };
            let _ = writeln!(out, "  - {}{suffix}", note.text);
        }
        out.push('\n');
    }
    let _ = writeln!(out, "Total  {}", hours(result.total_rounded_ms));
    if result.tag_overlap {
        out.push_str("Tags overlap: a recording with several tags counts in each.\n");
    }
    out
}

/// `Mon 2026-10-20` or `2026-10-20 – 2026-10-26`, plus the rounding when active.
fn header(result: &ReportResult, days: usize) -> String {
    let mut header = if days == 1 {
        day_label(result.from)
    } else {
        format!("{} – {}", result.from, result.to)
    };
    if let Some((minutes, up)) = rounding_step(result.filter.rounding) {
        let how = if up { "rounded up to" } else { "rounded to" };
        let _ = write!(header, " · {how} {minutes} min");
    }
    header
}

fn days_of(note: &ReportNote, range_days: usize) -> String {
    let format = if range_days <= WEEKDAYS_UP_TO {
        "%a"
    } else {
        "%-d %b"
    };
    note.days
        .iter()
        .map(|d| d.date().format(format).to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// `3:30 h`; hours without a leading zero.
fn hours(ms: Ms) -> String {
    let minutes = ms.max(0) / 60_000;
    format!("{}:{:02} h", minutes / 60, minutes % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reports::{ReportFilter, ReportGroup};
    use crate::store::settings::Rounding;

    const MINUTE: Ms = 60_000;

    fn day(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    fn note(text: &str, days: &[&str]) -> ReportNote {
        ReportNote {
            text: text.into(),
            days: days.iter().map(|d| day(d)).collect(),
        }
    }

    fn group(label: &str, minutes: i64, notes: Vec<ReportNote>) -> ReportGroup {
        ReportGroup {
            key: label.into(),
            label: label.into(),
            color: None,
            shade: None,
            duration_ms: minutes * MINUTE,
            rounded_ms: minutes * MINUTE,
            notes,
            rows: Vec::new(),
        }
    }

    fn result(from: &str, to: &str, group_by: Group, groups: Vec<ReportGroup>) -> ReportResult {
        let total = groups.iter().map(|g| g.rounded_ms).sum();
        ReportResult {
            filter: ReportFilter {
                group_by,
                ..ReportFilter::default()
            },
            from: day(from),
            to: day(to),
            groups,
            total_ms: total,
            total_rounded_ms: total,
            tag_overlap: false,
        }
    }

    #[test]
    fn one_day_by_project_lists_groups_notes_and_the_total() {
        let report = result(
            "2026-10-20",
            "2026-10-20",
            Group::Project,
            vec![
                group(
                    "Acme / Portal",
                    210,
                    vec![
                        note("fix login", &["2026-10-20"]),
                        note("review", &["2026-10-20"]),
                    ],
                ),
                group("Internal", 45, vec![]),
            ],
        );
        assert_eq!(
            text(&report),
            "Tue 2026-10-20\n\
             \n\
             Acme / Portal  3:30 h\n\
             \x20 - fix login\n\
             \x20 - review\n\
             \n\
             Internal  0:45 h\n\
             \n\
             Total  4:15 h\n"
        );
    }

    #[test]
    fn a_week_by_project_adds_the_weekdays_of_each_note() {
        let report = result(
            "2026-10-19",
            "2026-10-25",
            Group::Project,
            vec![group(
                "Acme / Portal",
                300,
                vec![
                    note("fix login", &["2026-10-19", "2026-10-21"]),
                    note("deploy", &["2026-10-23"]),
                ],
            )],
        );
        assert_eq!(
            text(&report),
            "2026-10-19 – 2026-10-25\n\
             \n\
             Acme / Portal  5:00 h\n\
             \x20 - fix login (Mon, Wed)\n\
             \x20 - deploy (Fri)\n\
             \n\
             Total  5:00 h\n"
        );
    }

    #[test]
    fn a_range_longer_than_a_week_names_the_dates_of_each_note() {
        let report = result(
            "2026-10-01",
            "2026-10-08",
            Group::Project,
            vec![group(
                "Portal",
                120,
                vec![note("fix login", &["2026-10-02", "2026-10-07"])],
            )],
        );
        assert_eq!(
            text(&report),
            "2026-10-01 – 2026-10-08\n\
             \n\
             Portal  2:00 h\n\
             \x20 - fix login (2 Oct, 7 Oct)\n\
             \n\
             Total  2:00 h\n"
        );
    }

    #[test]
    fn grouping_by_day_leaves_the_notes_without_days() {
        let report = result(
            "2026-10-19",
            "2026-10-20",
            Group::Day,
            vec![
                group("Mon 2026-10-19", 60, vec![note("plan", &["2026-10-19"])]),
                group("Tue 2026-10-20", 90, vec![note("build", &["2026-10-20"])]),
            ],
        );
        assert_eq!(
            text(&report),
            "2026-10-19 – 2026-10-20\n\
             \n\
             Mon 2026-10-19  1:00 h\n\
             \x20 - plan\n\
             \n\
             Tue 2026-10-20  1:30 h\n\
             \x20 - build\n\
             \n\
             Total  2:30 h\n"
        );
    }

    #[test]
    fn grouping_by_tag_with_overlap_ends_with_the_caveat() {
        let mut report = result(
            "2026-10-20",
            "2026-10-20",
            Group::Tag,
            vec![group("admin", 60, vec![]), group("support", 60, vec![])],
        );
        report.total_rounded_ms = 60 * MINUTE;
        report.tag_overlap = true;
        assert_eq!(
            text(&report),
            "Tue 2026-10-20\n\
             \n\
             admin  1:00 h\n\
             \n\
             support  1:00 h\n\
             \n\
             Total  1:00 h\n\
             Tags overlap: a recording with several tags counts in each.\n"
        );
    }

    #[test]
    fn the_header_names_the_rounding() {
        for (rounding, suffix) in [
            (Rounding::None, ""),
            (Rounding::Nearest15, " · rounded to 15 min"),
            (Rounding::Nearest6, " · rounded to 6 min"),
            (Rounding::Up15, " · rounded up to 15 min"),
            (Rounding::Up30, " · rounded up to 30 min"),
        ] {
            let mut report = result("2026-10-20", "2026-10-20", Group::Project, vec![]);
            report.filter.rounding = rounding;
            assert_eq!(
                text(&report),
                format!("Tue 2026-10-20{suffix}\n\nNo recordings.\n"),
                "{rounding:?}"
            );
        }
    }

    #[test]
    fn a_range_without_recordings_says_so() {
        let report = result("2026-10-19", "2026-10-25", Group::Project, vec![]);
        assert_eq!(text(&report), "2026-10-19 – 2026-10-25\n\nNo recordings.\n");
    }

    #[test]
    fn the_rounded_time_is_printed_not_the_exact_time() {
        let mut g = group("Portal", 7, vec![]);
        g.rounded_ms = 15 * MINUTE;
        let mut report = result("2026-10-20", "2026-10-20", Group::Project, vec![g]);
        report.total_ms = 7 * MINUTE;
        report.total_rounded_ms = 15 * MINUTE;
        assert_eq!(
            text(&report),
            "Tue 2026-10-20\n\nPortal  0:15 h\n\nTotal  0:15 h\n"
        );
    }

    #[test]
    fn hours_past_ten_are_not_padded() {
        let report = result(
            "2026-10-20",
            "2026-10-20",
            Group::Project,
            vec![group("Portal", 12 * 60 + 5, vec![])],
        );
        assert_eq!(
            text(&report),
            "Tue 2026-10-20\n\nPortal  12:05 h\n\nTotal  12:05 h\n"
        );
    }
}
