//! The Week and Month views: each day's recordings as a list, cut per day like the Day view.

pub mod commands;
pub mod entries;
pub mod jump;
pub mod mini_month;
pub mod period;
pub mod titles;

use chrono::{Datelike, Days, Months, Weekday};
use serde::Serialize;
use ts_rs::TS;

use crate::day::{Names, tracked_ms};
use crate::db::activities::ActivityRow;
use crate::db::recordings::Recording;
use crate::db::time::{Day, Ms};
use crate::reports::week_label;
use entries::{CalendarEntry, entries};

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct WeekView {
    pub week_start: Day,
    /// `Week 43 · 19 – 25 Oct 2026`.
    pub label: String,
    pub days: Vec<DayColumn>,
    pub total_ms: i64,
    pub tracked_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct DayColumn {
    pub day: Day,
    pub weekday: String,
    pub day_of_month: u32,
    pub is_today: bool,
    pub is_weekend: bool,
    /// The day's recordings, by start.
    pub entries: Vec<CalendarEntry>,
    pub total_ms: i64,
    pub tracked_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct MonthView {
    pub month_start: Day,
    /// `October 2026`.
    pub label: String,
    /// Monday to Sunday rows covering the month.
    pub weeks: Vec<Vec<MonthDay>>,
    /// The month's own days only.
    pub total_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct MonthDay {
    pub day: Day,
    pub weekday: String,
    pub day_of_month: u32,
    pub in_month: bool,
    pub is_today: bool,
    pub is_weekend: bool,
    pub recorded_ms: i64,
    /// The day's recordings, by start.
    pub entries: Vec<CalendarEntry>,
}

/// What both views read for their days.
pub struct CalendarInput {
    pub today: Day,
    pub now: Ms,
    pub recordings: Vec<Recording>,
    pub activities: Vec<ActivityRow>,
    pub names: Names,
}

/// The first and the last day the week of `day` covers.
pub fn week_days(day: Day) -> (Day, Day) {
    let start = day.week_start();
    (start, add_days(start, 6))
}

/// The first and the last day of `day`'s month.
pub fn month_range(day: Day) -> (Day, Day) {
    let first = day.month_start();
    (first, Day::from_date(first.date() + Months::new(1)).prev())
}

/// The first and the last day of the month grid of `day`: whole weeks from the Monday on or before the 1st.
pub fn month_days(day: Day) -> (Day, Day) {
    let (first, last) = month_range(day);
    (first.week_start(), add_days(last.week_start(), 6))
}

fn add_days(day: Day, days: u64) -> Day {
    Day::from_date(day.date() + Days::new(days))
}

fn is_weekend(day: Day) -> bool {
    matches!(day.date().weekday(), Weekday::Sat | Weekday::Sun)
}

fn weekday(day: Day) -> String {
    day.date().format("%a").to_string()
}

fn day_entries(day: Day, input: &CalendarInput) -> Vec<CalendarEntry> {
    entries(&input.recordings, &input.names, day.bounds(), input.now)
}

fn recorded_ms(entries: &[CalendarEntry]) -> i64 {
    entries.iter().map(|e| e.card.duration_ms).sum()
}

fn column(day: Day, input: &CalendarInput) -> DayColumn {
    let entries = day_entries(day, input);
    DayColumn {
        day,
        weekday: weekday(day),
        day_of_month: day.date().day(),
        is_today: day == input.today,
        is_weekend: is_weekend(day),
        total_ms: recorded_ms(&entries),
        tracked_ms: tracked_ms(&input.activities, day.bounds(), input.now),
        entries,
    }
}

pub fn week(day: Day, input: &CalendarInput) -> WeekView {
    let (start, end) = week_days(day);
    let days: Vec<DayColumn> = Day::range(&start, &end)
        .into_iter()
        .map(|d| column(d, input))
        .collect();
    WeekView {
        week_start: start,
        label: week_label(start),
        total_ms: days.iter().map(|d| d.total_ms).sum(),
        tracked_ms: days.iter().map(|d| d.tracked_ms).sum(),
        days,
    }
}

pub fn month(day: Day, input: &CalendarInput) -> MonthView {
    let first = day.month_start();
    let (start, end) = month_days(day);
    let cells: Vec<MonthDay> = Day::range(&start, &end)
        .into_iter()
        .map(|d| month_day(d, first, input))
        .collect();
    MonthView {
        month_start: first,
        label: first.date().format("%B %Y").to_string(),
        total_ms: cells
            .iter()
            .filter(|c| c.in_month)
            .map(|c| c.recorded_ms)
            .sum(),
        weeks: cells.chunks(7).map(<[MonthDay]>::to_vec).collect(),
    }
}

fn month_day(day: Day, first: Day, input: &CalendarInput) -> MonthDay {
    let entries = day_entries(day, input);
    MonthDay {
        day,
        weekday: weekday(day),
        day_of_month: day.date().day(),
        in_month: day.month_start() == first,
        is_today: day == input.today,
        is_weekend: is_weekend(day),
        recorded_ms: recorded_ms(&entries),
        entries,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::day::ProjectRef;
    use crate::db::recordings::RecordingSpan;

    const MIN: Ms = 60_000;
    const HOUR: Ms = 60 * MIN;
    const FAR_FUTURE: Ms = 4_000_000_000_000;

    fn day(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    fn start_of(s: &str) -> Ms {
        day(s).bounds().0
    }

    fn names() -> Names {
        let mut names = Names::default();
        for (id, name, color, shade) in [
            ("p1", "Portal", "blue", 4),
            ("p2", "Admin", "red", 2),
            ("p3", "Docs", "teal", 7),
            ("p4", "Misc", "gold", 5),
        ] {
            names.projects.insert(
                id.into(),
                ProjectRef {
                    name: name.into(),
                    client_name: None,
                    color: color.into(),
                    shade,
                },
            );
        }
        names
    }

    fn recording(id: &str, project: &str, start: Ms, end: Option<Ms>) -> Recording {
        spanned(id, project, &[(start, end)])
    }

    fn spanned(id: &str, project: &str, spans: &[(Ms, Option<Ms>)]) -> Recording {
        Recording {
            id: id.into(),
            project_id: project.into(),
            note: String::new(),
            tag_id: None,
            created_at: 0,
            updated_at: 0,
            spans: spans
                .iter()
                .enumerate()
                .map(|(i, &(start, end))| RecordingSpan {
                    id: format!("{id}-span{i}"),
                    start,
                    end,
                })
                .collect(),
        }
    }

    fn activity(id: i64, source: &str, start: Ms, end: Option<Ms>) -> ActivityRow {
        ActivityRow {
            id,
            source: source.into(),
            start,
            end,
            app: "Code".into(),
            bundle_id: None,
            title: String::new(),
            url: None,
            path: None,
            private: false,
        }
    }

    fn input(recordings: Vec<Recording>, activities: Vec<ActivityRow>) -> CalendarInput {
        CalendarInput {
            today: day("2026-10-21"),
            now: FAR_FUTURE,
            recordings,
            activities,
            names: names(),
        }
    }

    fn cell<'a>(view: &'a MonthView, d: &str) -> &'a MonthDay {
        view.weeks
            .iter()
            .flatten()
            .find(|c| c.day == day(d))
            .unwrap_or_else(|| panic!("no cell {d}"))
    }

    #[test]
    fn a_week_has_seven_columns_from_its_monday() {
        for (given, monday) in [
            ("2026-10-21", "2026-10-19"),
            ("2026-10-19", "2026-10-19"),
            ("2026-10-25", "2026-10-19"),
            ("2026-10-26", "2026-10-26"),
        ] {
            let view = week(day(given), &input(vec![], vec![]));

            let days: Vec<Day> = view.days.iter().map(|d| d.day).collect();
            assert_eq!(view.week_start, day(monday), "{given}");
            let sunday = week_days(day(monday)).1;
            assert_eq!(days, Day::range(&day(monday), &sunday), "{given}");
            assert_eq!(days.len(), 7, "{given}");
        }
    }

    #[test]
    fn week_days_run_from_monday_to_sunday() {
        assert_eq!(
            week_days(day("2026-10-25")),
            (day("2026-10-19"), day("2026-10-25"))
        );
        assert_eq!(
            week_days(day("2026-12-30")),
            (day("2026-12-28"), day("2027-01-03"))
        );
    }

    #[test]
    fn a_week_is_labelled_with_its_number_and_range_and_each_column_with_its_weekday() {
        let view = week(day("2026-10-21"), &input(vec![], vec![]));

        assert_eq!(view.label, "Week 43 · 19 – 25 Oct 2026");
        let labels: Vec<String> = view
            .days
            .iter()
            .map(|d| format!("{} {}", d.weekday, d.day_of_month))
            .collect();
        assert_eq!(
            labels,
            [
                "Mon 19", "Tue 20", "Wed 21", "Thu 22", "Fri 23", "Sat 24", "Sun 25"
            ]
        );
    }

    #[test]
    fn only_saturday_and_sunday_are_weekend_columns() {
        let view = week(day("2026-10-21"), &input(vec![], vec![]));

        let flags: Vec<bool> = view.days.iter().map(|d| d.is_weekend).collect();
        assert_eq!(flags, [false, false, false, false, false, true, true]);
    }

    #[test]
    fn only_saturday_and_sunday_are_weekend_cells_in_the_month() {
        let view = month(day("2026-10-21"), &input(vec![], vec![]));

        for row in &view.weeks {
            let flags: Vec<bool> = row.iter().map(|c| c.is_weekend).collect();
            assert_eq!(
                flags,
                [false, false, false, false, false, true, true],
                "{}",
                row[0].day
            );
        }
    }

    #[test]
    fn a_column_carries_its_short_weekday_and_day_of_the_month() {
        let view = week(day("2026-10-28"), &input(vec![], vec![]));

        let columns: Vec<(&str, u32)> = view
            .days
            .iter()
            .map(|d| (d.weekday.as_str(), d.day_of_month))
            .collect();
        assert_eq!(
            columns,
            [
                ("Mon", 26),
                ("Tue", 27),
                ("Wed", 28),
                ("Thu", 29),
                ("Fri", 30),
                ("Sat", 31),
                ("Sun", 1)
            ]
        );
    }

    #[test]
    fn month_cells_carry_their_weekday_from_monday_to_sunday() {
        let view = month(day("2026-10-21"), &input(vec![], vec![]));

        for row in &view.weeks {
            let weekdays: Vec<&str> = row.iter().map(|c| c.weekday.as_str()).collect();
            assert_eq!(
                weekdays,
                ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
                "{}",
                row[0].day
            );
        }
    }

    #[test]
    fn only_today_is_flagged_in_the_week() {
        let view = week(day("2026-10-21"), &input(vec![], vec![]));

        let flags: Vec<bool> = view.days.iter().map(|d| d.is_today).collect();
        assert_eq!(flags, [false, false, true, false, false, false, false]);
        let other_week = week(day("2026-10-28"), &input(vec![], vec![]));
        assert!(other_week.days.iter().all(|d| !d.is_today));
    }

    #[test]
    fn a_week_without_recordings_is_empty() {
        let view = week(day("2026-10-21"), &input(vec![], vec![]));

        assert_eq!((view.total_ms, view.tracked_ms), (0, 0));
        assert!(
            view.days
                .iter()
                .all(|d| d.entries.is_empty() && d.total_ms == 0 && d.tracked_ms == 0)
        );
    }

    #[test]
    fn day_and_week_totals_add_up_the_recordings() {
        let mon = start_of("2026-10-19");
        let wed = start_of("2026-10-21");
        let recordings = vec![
            recording("a", "p1", mon + 9 * HOUR, Some(mon + 10 * HOUR)),
            recording("b", "p2", mon + 11 * HOUR, Some(mon + 11 * HOUR + 30 * MIN)),
            recording("c", "p1", wed + 8 * HOUR, Some(wed + 10 * HOUR)),
        ];

        let view = week(day("2026-10-21"), &input(recordings, vec![]));

        let totals: Vec<i64> = view.days.iter().map(|d| d.total_ms).collect();
        assert_eq!(totals, [HOUR + 30 * MIN, 0, 2 * HOUR, 0, 0, 0, 0]);
        assert_eq!(view.total_ms, 3 * HOUR + 30 * MIN);
        let ids: Vec<&str> = view.days[0]
            .entries
            .iter()
            .map(|e| e.card.id.as_str())
            .collect();
        assert_eq!(ids, ["a", "b"]);
        let summed: i64 = view.days[0]
            .entries
            .iter()
            .map(|e| e.card.duration_ms)
            .sum();
        assert_eq!(summed, view.days[0].total_ms);
    }

    #[test]
    fn entries_come_one_per_recording_in_start_order_with_the_project_look() {
        let mon = start_of("2026-10-19");
        let recordings = vec![
            recording("late", "p3", mon + 14 * HOUR, Some(mon + 15 * HOUR)),
            recording("early", "p1", mon + 8 * HOUR, Some(mon + 9 * HOUR)),
        ];

        let view = week(day("2026-10-19"), &input(recordings, vec![]));

        let entries = &view.days[0].entries;
        let ids: Vec<&str> = entries.iter().map(|e| e.card.id.as_str()).collect();
        assert_eq!(ids, ["early", "late"]);
        let card = &entries[1].card;
        assert_eq!(
            (card.project_name.as_str(), card.color.as_str(), card.shade),
            ("Docs", "teal", 7)
        );
    }

    #[test]
    fn several_spans_on_one_day_make_one_entry_with_the_summed_duration() {
        let mon = start_of("2026-10-19");
        let recordings = vec![spanned(
            "a",
            "p1",
            &[
                (mon + 9 * HOUR, Some(mon + 10 * HOUR)),
                (mon + 14 * HOUR, Some(mon + 14 * HOUR + 30 * MIN)),
            ],
        )];

        let view = week(day("2026-10-19"), &input(recordings, vec![]));

        let entries = &view.days[0].entries;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].card.duration_ms, HOUR + 30 * MIN);
        assert_eq!(entries[0].card.span_ids, ["a-span0", "a-span1"]);
        assert_eq!(view.days[0].total_ms, HOUR + 30 * MIN);
    }

    #[test]
    fn a_recording_over_midnight_appears_in_both_columns_with_clipped_durations() {
        let mon_end = day("2026-10-19").bounds().1;
        let recordings = vec![recording(
            "night",
            "p1",
            mon_end - HOUR,
            Some(mon_end + 2 * HOUR),
        )];

        let view = week(day("2026-10-21"), &input(recordings, vec![]));

        let (mon, tue) = (&view.days[0], &view.days[1]);
        assert_eq!((mon.total_ms, tue.total_ms), (HOUR, 2 * HOUR));
        assert_eq!(view.total_ms, 3 * HOUR);
        assert_eq!(
            (
                mon.entries[0].card.duration_ms,
                tue.entries[0].card.duration_ms
            ),
            (HOUR, 2 * HOUR)
        );
        assert_eq!(
            (tue.entries[0].card.start, tue.entries[0].card.end),
            (mon_end, mon_end + 2 * HOUR)
        );
    }

    #[test]
    fn a_recording_outside_the_week_is_not_counted() {
        let before = start_of("2026-10-18");
        let after = start_of("2026-10-26");
        let recordings = vec![
            recording("a", "p1", before + HOUR, Some(before + 2 * HOUR)),
            recording("b", "p1", after + HOUR, Some(after + 2 * HOUR)),
        ];

        let view = week(day("2026-10-21"), &input(recordings, vec![]));

        assert_eq!(view.total_ms, 0);
        assert!(view.days.iter().all(|d| d.entries.is_empty()));
    }

    #[test]
    fn a_running_recording_is_clipped_to_now_and_flagged_running() {
        let tue = start_of("2026-10-20");
        let recordings = vec![recording("timer", "p1", tue + HOUR, None)];
        let calendar = CalendarInput {
            now: tue + 3 * HOUR,
            ..input(recordings, vec![])
        };

        let view = week(day("2026-10-21"), &calendar);

        let tuesday = &view.days[1];
        assert_eq!(tuesday.total_ms, 2 * HOUR);
        assert_eq!(
            (tuesday.entries[0].card.running, tuesday.entries[0].card.end),
            (true, tue + 3 * HOUR)
        );
        assert_eq!(view.days[2].total_ms, 0);
        assert_eq!(view.total_ms, 2 * HOUR);
    }

    #[test]
    fn tracked_time_counts_tracker_activity_only_and_overlaps_once() {
        let mon = start_of("2026-10-19");
        let tue = start_of("2026-10-20");
        let activities = vec![
            activity(1, "tracker", mon + 9 * HOUR, Some(mon + 10 * HOUR)),
            activity(
                2,
                "tracker",
                mon + 9 * HOUR + 30 * MIN,
                Some(mon + 11 * HOUR),
            ),
            activity(
                3,
                "external:calendar",
                mon + 13 * HOUR,
                Some(mon + 15 * HOUR),
            ),
            activity(4, "tracker", tue + HOUR, Some(tue + 2 * HOUR)),
        ];

        let view = week(day("2026-10-21"), &input(vec![], activities));

        let tracked: Vec<i64> = view.days.iter().map(|d| d.tracked_ms).collect();
        assert_eq!(tracked, [2 * HOUR, HOUR, 0, 0, 0, 0, 0]);
        assert_eq!(view.tracked_ms, 3 * HOUR);
    }

    #[test]
    fn tracked_time_over_midnight_is_split_between_the_days() {
        let mon_end = day("2026-10-19").bounds().1;
        let activities = vec![activity(
            1,
            "tracker",
            mon_end - HOUR,
            Some(mon_end + 30 * MIN),
        )];

        let view = week(day("2026-10-21"), &input(vec![], activities));

        assert_eq!(
            (
                view.days[0].tracked_ms,
                view.days[1].tracked_ms,
                view.tracked_ms
            ),
            (HOUR, 30 * MIN, HOUR + 30 * MIN)
        );
    }

    #[test]
    fn a_week_view_serializes_with_camel_case_keys() {
        let mon = start_of("2026-10-19");
        let recordings = vec![recording("a", "p1", mon + HOUR, Some(mon + 2 * HOUR))];

        let view = week(day("2026-10-19"), &input(recordings, vec![]));

        let value = serde_json::to_value(&view).unwrap();
        assert_eq!(value["weekStart"], json!("2026-10-19"));
        assert_eq!(value["label"], json!("Week 43 · 19 – 25 Oct 2026"));
        assert_eq!(value["totalMs"], json!(HOUR));
        assert_eq!(value["trackedMs"], json!(0));
        let column = &value["days"][0];
        assert_eq!(column["day"], json!("2026-10-19"));
        assert_eq!(column["isToday"], json!(false));
        assert_eq!(column["totalMs"], json!(HOUR));
        assert_eq!(column["trackedMs"], json!(0));
        assert_eq!(column["weekday"], json!("Mon"));
        assert!(column.get("bounds").is_none());
        assert!(column.get("recordings").is_none());
        let entry = &column["entries"][0];
        assert_eq!(entry["id"], json!("a"));
        assert_eq!(entry["projectName"], json!("Portal"));
        assert_eq!(entry["durationMs"], json!(HOUR));
        assert_eq!(entry["spanIds"], json!(["a-span0"]));
        assert_eq!(entry["heightUnits"], json!(4));
        assert!(entry.get("card").is_none());
    }

    #[test]
    fn a_week_label_across_months_names_both() {
        let view = week(day("2026-10-30"), &input(vec![], vec![]));

        assert_eq!(view.label, "Week 44 · 26 Oct – 1 Nov 2026");
    }

    #[test]
    fn the_month_grid_runs_from_the_monday_on_or_before_the_first_to_the_sunday_on_or_after_the_last()
     {
        for (given, first, last) in [
            ("2026-10-21", "2026-09-28", "2026-11-01"),
            ("2027-02-10", "2027-02-01", "2027-02-28"),
            ("2026-08-15", "2026-07-27", "2026-09-06"),
            ("2024-02-29", "2024-01-29", "2024-03-03"),
        ] {
            assert_eq!(month_days(day(given)), (day(first), day(last)), "{given}");
        }
    }

    #[test]
    fn a_month_has_whole_monday_to_sunday_rows() {
        for (given, rows, first, last) in [
            ("2027-02-10", 4, "2027-02-01", "2027-02-28"),
            ("2026-10-21", 5, "2026-09-28", "2026-11-01"),
            ("2026-08-01", 6, "2026-07-27", "2026-09-06"),
        ] {
            let view = month(day(given), &input(vec![], vec![]));

            assert_eq!(view.weeks.len(), rows, "{given}");
            assert!(view.weeks.iter().all(|w| w.len() == 7), "{given}");
            assert_eq!(view.weeks[0][0].day, day(first), "{given}");
            assert_eq!(view.weeks[rows - 1][6].day, day(last), "{given}");
            let days: Vec<Day> = view.weeks.iter().flatten().map(|c| c.day).collect();
            assert_eq!(days, Day::range(&day(first), &day(last)), "{given}");
        }
    }

    #[test]
    fn a_month_is_labelled_with_its_name_and_year_and_starts_on_the_first() {
        let view = month(day("2026-10-21"), &input(vec![], vec![]));

        assert_eq!(view.label, "October 2026");
        assert_eq!(view.month_start, day("2026-10-01"));
    }

    #[test]
    fn days_outside_the_month_are_flagged() {
        let view = month(day("2026-10-21"), &input(vec![], vec![]));

        let flags: Vec<bool> = view.weeks.iter().flatten().map(|c| c.in_month).collect();
        let outside_before = 3;
        let outside_after = 1;
        let mut expected = vec![false; outside_before];
        expected.extend(vec![true; 31]);
        expected.extend(vec![false; outside_after]);
        assert_eq!(flags, expected);
    }

    #[test]
    fn only_today_is_flagged_in_the_month() {
        let view = month(day("2026-10-21"), &input(vec![], vec![]));

        let today: Vec<Day> = view
            .weeks
            .iter()
            .flatten()
            .filter(|c| c.is_today)
            .map(|c| c.day)
            .collect();
        assert_eq!(today, [day("2026-10-21")]);
    }

    #[test]
    fn the_month_total_counts_the_months_own_days_only() {
        let sep30 = start_of("2026-09-30");
        let oct5 = start_of("2026-10-05");
        let nov1 = start_of("2026-11-01");
        let recordings = vec![
            recording("a", "p1", sep30 + HOUR, Some(sep30 + 2 * HOUR)),
            recording("b", "p1", oct5 + HOUR, Some(oct5 + 3 * HOUR)),
            recording("c", "p1", nov1 + HOUR, Some(nov1 + 5 * HOUR)),
        ];

        let view = month(day("2026-10-21"), &input(recordings, vec![]));

        assert_eq!(view.total_ms, 2 * HOUR);
        assert_eq!(cell(&view, "2026-09-30").recorded_ms, HOUR);
        assert_eq!(cell(&view, "2026-10-05").recorded_ms, 2 * HOUR);
        assert_eq!(cell(&view, "2026-11-01").recorded_ms, 4 * HOUR);
    }

    #[test]
    fn a_recording_over_midnight_counts_in_each_cell_it_touches() {
        let end_of_5th = day("2026-10-05").bounds().1;
        let recordings = vec![recording(
            "night",
            "p1",
            end_of_5th - HOUR,
            Some(end_of_5th + 2 * HOUR),
        )];

        let view = month(day("2026-10-21"), &input(recordings, vec![]));

        assert_eq!(cell(&view, "2026-10-05").recorded_ms, HOUR);
        assert_eq!(cell(&view, "2026-10-06").recorded_ms, 2 * HOUR);
        assert_eq!(view.total_ms, 3 * HOUR);
    }

    #[test]
    fn a_recording_with_spans_on_several_days_counts_each_days_parts_in_the_week() {
        let (mon, tue) = (start_of("2026-10-19"), start_of("2026-10-20"));
        let recordings = vec![spanned(
            "a",
            "p1",
            &[
                (mon + 9 * HOUR, Some(mon + 10 * HOUR)),
                (mon + 14 * HOUR, Some(mon + 14 * HOUR + 30 * MIN)),
                (tue - HOUR, Some(tue + HOUR)),
            ],
        )];

        let view = week(day("2026-10-21"), &input(recordings, vec![]));

        let per_day: Vec<(usize, Ms)> = view
            .days
            .iter()
            .take(3)
            .map(|d| (d.entries.len(), d.total_ms))
            .collect();
        assert_eq!(per_day, [(1, 2 * HOUR + 30 * MIN), (1, HOUR), (0, 0)]);
        assert_eq!(view.total_ms, 3 * HOUR + 30 * MIN);
    }

    #[test]
    fn spans_of_one_recording_add_up_in_the_month_cell_and_the_total() {
        let (d, next) = (start_of("2026-10-07"), start_of("2026-10-08"));
        let recordings = vec![
            spanned(
                "a",
                "p1",
                &[
                    (d + 9 * HOUR, Some(d + 10 * HOUR)),
                    (d + 13 * HOUR, Some(d + 15 * HOUR)),
                    (next - HOUR, Some(next + 30 * MIN)),
                ],
            ),
            recording("b", "p2", d + 16 * HOUR, Some(d + 17 * HOUR)),
        ];

        let view = month(day("2026-10-21"), &input(recordings, vec![]));

        let seventh = cell(&view, "2026-10-07");
        assert_eq!(seventh.recorded_ms, 5 * HOUR);
        let durations: Vec<(&str, Ms)> = seventh
            .entries
            .iter()
            .map(|e| (e.card.id.as_str(), e.card.duration_ms))
            .collect();
        assert_eq!(durations, [("a", 4 * HOUR), ("b", HOUR)]);
        assert_eq!(cell(&view, "2026-10-08").recorded_ms, 30 * MIN);
        assert_eq!(view.total_ms, 5 * HOUR + 30 * MIN);
    }

    #[test]
    fn a_cell_lists_its_entries_in_start_order_and_its_total_is_their_sum() {
        let d = start_of("2026-10-07");
        let recordings = vec![
            recording("a", "p2", d + HOUR, Some(d + 3 * HOUR)),
            recording("b", "p1", d + 4 * HOUR, Some(d + 7 * HOUR)),
            recording("c", "p3", d + 8 * HOUR, Some(d + 9 * HOUR)),
            recording("d", "p1", d + 10 * HOUR, Some(d + 10 * HOUR + 30 * MIN)),
            recording("e", "p4", d + 11 * HOUR, Some(d + 11 * HOUR + 30 * MIN)),
        ];

        let view = month(day("2026-10-21"), &input(recordings, vec![]));

        let cell = cell(&view, "2026-10-07");
        let ids: Vec<&str> = cell.entries.iter().map(|e| e.card.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c", "d", "e"]);
        let summed: i64 = cell.entries.iter().map(|e| e.card.duration_ms).sum();
        assert_eq!((cell.recorded_ms, summed), (7 * HOUR, 7 * HOUR));
    }

    #[test]
    fn empty_days_have_no_entries() {
        let d = start_of("2026-10-07");
        let recordings = vec![recording("a", "p1", d + HOUR, Some(d + 2 * HOUR))];

        let view = month(day("2026-10-21"), &input(recordings, vec![]));

        for c in view
            .weeks
            .iter()
            .flatten()
            .filter(|c| c.day != day("2026-10-07"))
        {
            assert!(c.entries.is_empty(), "{}", c.day);
            assert_eq!(c.recorded_ms, 0, "{}", c.day);
        }
    }

    #[test]
    fn a_running_recording_counts_to_now_in_the_month() {
        let d = start_of("2026-10-07");
        let calendar = CalendarInput {
            now: d + 4 * HOUR,
            ..input(vec![recording("timer", "p1", d + HOUR, None)], vec![])
        };

        let view = month(day("2026-10-21"), &calendar);

        assert_eq!(cell(&view, "2026-10-07").recorded_ms, 3 * HOUR);
        assert_eq!(view.total_ms, 3 * HOUR);
    }

    #[test]
    fn a_month_view_serializes_with_camel_case_keys() {
        let d = start_of("2026-10-07");
        let recordings = vec![recording("a", "p1", d + HOUR, Some(d + 2 * HOUR))];

        let view = month(day("2026-10-21"), &input(recordings, vec![]));

        let value = serde_json::to_value(&view).unwrap();
        assert_eq!(value["monthStart"], json!("2026-10-01"));
        assert_eq!(value["totalMs"], json!(HOUR));
        let cell = &value["weeks"][1][2];
        assert_eq!(
            cell,
            &json!({
                "day": "2026-10-07",
                "weekday": "Wed",
                "dayOfMonth": 7,
                "inMonth": true,
                "isToday": false,
                "isWeekend": false,
                "recordedMs": HOUR,
                "entries": [{
                    "id": "a",
                    "projectId": "p1",
                    "projectName": "Portal",
                    "clientName": null,
                    "color": "blue",
                    "shade": 4,
                    "note": "",
                    "tag": null,
                    "durationMs": HOUR,
                    "start": d + HOUR,
                    "end": d + 2 * HOUR,
                    "spanIds": ["a-span0"],
                    "running": false,
                    "heightUnits": 4
                }]
            })
        );
    }
}
