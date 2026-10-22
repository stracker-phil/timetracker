//! A report's rows: each recording's parts per day of the range, filtered and rounded.

use std::collections::HashMap;

use super::{MINUTE, ReportData, ReportFilter, ReportRow};
use crate::db::clients::Client;
use crate::db::projects::Project;
use crate::db::recordings::{Recording, Span};
use crate::db::tags::Tag;
use crate::db::time::{Day, Ms, clip};
use crate::store::settings::Rounding;

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
pub(super) fn rows(
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
        let client = project.client_id.as_deref().and_then(|id| clients.get(id));
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
                client: client.map(|c| c.name.clone()),
                client_id: client.map(|c| c.id.clone()),
                client_color: client.map(|c| c.color.clone()),
                color: project.color.clone(),
                shade: project.shade,
                billable: project.billable,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reports::testing::*;
    use crate::reports::*;
    use crate::store::settings::{Group, Rounding};

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
    fn rows_carry_their_projects_billable_flag() {
        let result = billing_report(
            &by(Group::Day),
            &["p-portal"],
            vec![on("r1", "p-portal", D, 30), on("r2", "p-site", D, 30)],
        );

        let flags: Vec<_> = result.groups[0]
            .rows
            .iter()
            .map(|r| (r.project_id.as_str(), r.billable))
            .collect();
        assert_eq!(flags, [("p-portal", true), ("p-site", false)]);
    }
}
