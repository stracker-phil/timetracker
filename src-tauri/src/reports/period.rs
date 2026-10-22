//! The status bar's numbers for the shown period: its name, the recorded time and the billable share.

use chrono::Datelike;
use serde::Serialize;
use ts_rs::TS;

use super::range::add_days;
use super::{Range, billable, resolve};
use crate::calendar::titles::{day_title, range_title};
use crate::db::Db;
use crate::db::time::{Day, Ms};
use crate::error::AppResult;
use crate::store::settings::DayTitle;

/// `This week · recorded 1:30 · Billable 67 %`; `billable_percent` is `None` while nothing is recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct PeriodTotals {
    pub label: String,
    pub recorded_ms: i64,
    pub billable_ms: i64,
    pub billable_percent: Option<u8>,
}

/// Every recording of `range`, archived projects included, whatever the report's filter.
pub async fn period_totals(db: &Db, range: &Range, today: Day, now: Ms) -> AppResult<PeriodTotals> {
    let (from, to) = resolve(range, today)?;
    let billable = billable(db, range, today, now).await?;
    Ok(PeriodTotals {
        label: period_label(from, to, today),
        recorded_ms: billable.recorded_ms,
        billable_ms: billable.billable_ms,
        billable_percent: billable.percent,
    })
}

/// Today, Yesterday, This week, Last week, This month or Last month; else the day (`Mon, Oct 19`), the week
/// (`Week 43`), the month (`October`) or the days (`Oct 1 – 14`), with the year when it is not `today`'s.
pub fn period_label(from: Day, to: Day, today: Day) -> String {
    let other_year =
        |day: Day| (day.date().year() != today.date().year()).then(|| day.date().year());
    if from == to {
        return match from {
            d if d == today => "Today".into(),
            d if d == today.prev() => "Yesterday".into(),
            d => day_title(d, DayTitle::Short, today),
        };
    }
    if from == from.week_start() && to == add_days(from, 6) {
        let this_week = today.week_start();
        return match from {
            d if d == this_week => "This week".into(),
            d if d == add_days(this_week, -7) => "Last week".into(),
            d => match other_year(d) {
                Some(year) => format!("Week {}, {year}", d.date().iso_week().week()),
                None => format!("Week {}", d.date().iso_week().week()),
            },
        };
    }
    if from == from.month_start() && to.month_start() == from && to.next().month_start() != from {
        let this_month = today.month_start();
        return match from {
            d if d == this_month => "This month".into(),
            d if d == this_month.prev().month_start() => "Last month".into(),
            d => match other_year(d) {
                Some(year) => format!("{} {year}", d.date().format("%B")),
                None => d.date().format("%B").to_string(),
            },
        };
    }
    range_title(from, to, DayTitle::Short, today)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::db::projects::{self, Project, ProjectInput};
    use crate::error::ErrorKind;
    use crate::reports::testing::*;

    const TODAY: &str = "2026-10-22";

    #[test]
    fn a_single_day_is_today_yesterday_or_its_title() {
        for (given, expected) in [
            ("2026-10-22", "Today"),
            ("2026-10-21", "Yesterday"),
            ("2026-10-19", "Mon, Oct 19"),
            ("2025-10-20", "Mon, Oct 20, 2025"),
        ] {
            assert_eq!(
                period_label(day(given), day(given), day(TODAY)),
                expected,
                "{given}"
            );
        }
    }

    #[test]
    fn a_whole_week_is_this_week_last_week_or_its_number() {
        for (from, to, expected) in [
            ("2026-10-19", "2026-10-25", "This week"),
            ("2026-10-12", "2026-10-18", "Last week"),
            ("2026-10-05", "2026-10-11", "Week 41"),
            ("2025-10-06", "2025-10-12", "Week 41, 2025"),
        ] {
            assert_eq!(
                period_label(day(from), day(to), day(TODAY)),
                expected,
                "{from} to {to}"
            );
        }
    }

    #[test]
    fn a_whole_month_is_this_month_last_month_or_its_name() {
        for (from, to, expected) in [
            ("2026-10-01", "2026-10-31", "This month"),
            ("2026-09-01", "2026-09-30", "Last month"),
            ("2026-08-01", "2026-08-31", "August"),
            ("2025-08-01", "2025-08-31", "August 2025"),
        ] {
            assert_eq!(
                period_label(day(from), day(to), day(TODAY)),
                expected,
                "{from} to {to}"
            );
        }
    }

    #[test]
    fn last_month_is_found_across_the_new_year() {
        assert_eq!(
            period_label(day("2025-12-01"), day("2025-12-31"), day("2026-01-10")),
            "Last month"
        );
    }

    #[test]
    fn any_other_range_names_its_first_and_last_day() {
        for (from, to, expected) in [
            ("2026-10-01", "2026-10-14", "Oct 1 \u{2013} 14"),
            ("2026-10-20", "2026-10-22", "Oct 20 \u{2013} 22"),
            ("2026-09-28", "2026-10-14", "Sep 28 \u{2013} Oct 14"),
            (
                "2025-12-20",
                "2026-01-05",
                "Dec 20, 2025 \u{2013} Jan 5, 2026",
            ),
            ("2025-10-01", "2025-10-14", "Oct 1 \u{2013} 14, 2025"),
        ] {
            assert_eq!(
                period_label(day(from), day(to), day(TODAY)),
                expected,
                "{from} to {to}"
            );
        }
    }

    async fn stored_project(db: &Db, name: &str, billable: bool) -> Project {
        projects::insert(
            db,
            ProjectInput {
                client_id: None,
                name: name.into(),
                color: String::new(),
                shade: None,
                billable,
            },
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn totals_sum_all_recordings_of_the_range_and_the_billable_share() {
        let db = Db::open_memory().await.unwrap();
        let work = stored_project(&db, "Work", true).await;
        let chores = stored_project(&db, "Chores", false).await;
        let old = stored_project(&db, "Old", true).await;
        for (project, from, to) in [
            (&work, (9, 0), (10, 0)),
            (&chores, (12, 0), (13, 0)),
            (&old, (14, 0), (14, 30)),
        ] {
            crate::test_support::recording(
                &db,
                &project.id,
                at(TODAY, from.0, from.1),
                Some(at(TODAY, to.0, to.1)),
            )
            .await;
        }
        crate::test_support::recording(
            &db,
            &work.id,
            at("2026-10-21", 9, 0),
            Some(at("2026-10-21", 11, 0)),
        )
        .await;
        projects::set_archived(&db, &old.id, true).await.unwrap();

        let totals = period_totals(&db, &Range::Today, day(TODAY), now())
            .await
            .unwrap();

        assert_eq!(
            totals,
            PeriodTotals {
                label: "Today".into(),
                recorded_ms: minutes(150),
                billable_ms: minutes(90),
                billable_percent: Some(60),
            }
        );
    }

    #[tokio::test]
    async fn totals_of_a_range_without_recordings_have_no_percent() {
        let db = Db::open_memory().await.unwrap();

        let totals = period_totals(&db, &Range::ThisWeek, day(TODAY), now())
            .await
            .unwrap();

        assert_eq!(
            totals,
            PeriodTotals {
                label: "This week".into(),
                recorded_ms: 0,
                billable_ms: 0,
                billable_percent: None,
            }
        );
    }

    #[tokio::test]
    async fn totals_are_labelled_by_the_resolved_range() {
        let db = Db::open_memory().await.unwrap();

        let totals = period_totals(&db, &custom("2026-10-01", "2026-10-14"), day(TODAY), now())
            .await
            .unwrap();

        assert_eq!(totals.label, "Oct 1 \u{2013} 14");
    }

    #[tokio::test]
    async fn totals_reject_a_reversed_custom_range() {
        let db = Db::open_memory().await.unwrap();

        let err = period_totals(&db, &custom("2026-10-07", "2026-10-01"), day(TODAY), now())
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn totals_serialize_with_camel_case_fields() {
        let totals = PeriodTotals {
            label: "Today".into(),
            recorded_ms: minutes(90),
            billable_ms: minutes(60),
            billable_percent: Some(67),
        };

        assert_eq!(
            serde_json::to_value(&totals).unwrap(),
            json!({
                "label": "Today",
                "recordedMs": minutes(90),
                "billableMs": minutes(60),
                "billablePercent": 67
            })
        );
    }
}
