//! A report's groups: by project, tag, day, week or month, each with its distinct notes.

use chrono::Datelike;

use super::range::add_days;
use super::{ReportFilter, ReportGroup, ReportNote, ReportRow};
use crate::db::time::Day;
use crate::store::settings::Group;

const NO_TAG_KEY: &str = "untagged";
const NO_TAG_LABEL: &str = "Untagged";

struct GroupKey {
    key: String,
    label: String,
    /// Groups sort by it: whether the group goes last, then by name or date.
    order: (bool, String, String),
    /// Only when grouped by project.
    project: Option<GroupProject>,
}

struct GroupProject {
    color: String,
    shade: u8,
    billable: bool,
}

/// Each row counts in exactly one group, so the groups add up to the total.
pub(super) fn groups(filter: &ReportFilter, rows: &[ReportRow]) -> Vec<ReportGroup> {
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
            color: key.project.as_ref().map(|p| p.color.clone()),
            shade: key.project.as_ref().map(|p| p.shade),
            billable: key.project.map(|p| p.billable),
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
                project: Some(GroupProject {
                    color: row.color.clone(),
                    shade: row.shade,
                    billable: row.billable,
                }),
            }
        }
        Group::Tag => match (&row.tag_id, &row.tag) {
            (Some(id), Some(name)) => GroupKey {
                key: id.clone(),
                label: name.clone(),
                order: (false, name.to_lowercase(), String::new()),
                project: None,
            },
            _ => GroupKey {
                key: NO_TAG_KEY.into(),
                label: NO_TAG_LABEL.into(),
                order: (true, String::new(), String::new()),
                project: None,
            },
        },
        Group::Day => GroupKey {
            key: row.day.to_string(),
            label: day_label(row.day),
            order: (false, row.day.to_string(), String::new()),
            project: None,
        },
        Group::Week => {
            let start = row.day.week_start();
            GroupKey {
                key: start.to_string(),
                label: week_label(start),
                order: (false, start.to_string(), String::new()),
                project: None,
            }
        }
        Group::Month => {
            let start = row.day.month_start();
            GroupKey {
                key: start.date().format("%Y-%m").to_string(),
                label: start.date().format("%B %Y").to_string(),
                order: (false, start.to_string(), String::new()),
                project: None,
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
    use super::*;
    use crate::reports::testing::*;
    use crate::reports::*;
    use crate::store::settings::Rounding;

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

    #[test]
    fn project_groups_carry_the_projects_billable_flag() {
        let result = billing_report(
            &by(Group::Project),
            &["p-site"],
            vec![on("r1", "p-portal", D, 30), on("r2", "p-site", D, 30)],
        );

        let flags: Vec<_> = result
            .groups
            .iter()
            .map(|g| (g.key.as_str(), g.billable))
            .collect();
        assert_eq!(flags, [("p-portal", Some(false)), ("p-site", Some(true))]);
    }

    #[test]
    fn groups_other_than_projects_have_no_billable_flag() {
        for group_by in [Group::Tag, Group::Day, Group::Week, Group::Month] {
            let result = billing_report(
                &by(group_by),
                &["p-portal"],
                vec![with_tag(on("r1", "p-portal", D, 30), "t-sup")],
            );

            assert_eq!(result.groups[0].billable, None, "{group_by:?}");
        }
    }
}
