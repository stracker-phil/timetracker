//! The billable share of recorded time: a recording is billable exactly when its project is.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{Range, ReportFilter, run};
use crate::db::Db;
use crate::db::time::{Day, Ms, format_hm};
use crate::error::AppResult;

/// Billable time out of the recorded time; `percent` is `None` when nothing is recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Billable {
    pub recorded_ms: i64,
    pub billable_ms: i64,
    /// Rounded to a whole percent.
    pub percent: Option<u8>,
}

impl Billable {
    pub fn of(recorded_ms: i64, billable_ms: i64) -> Self {
        let percent = (recorded_ms > 0).then(|| {
            let percent = (billable_ms as f64 * 100.0 / recorded_ms as f64).round();
            percent.clamp(0.0, 100.0) as u8
        });
        Self {
            recorded_ms,
            billable_ms,
            percent,
        }
    }
}

/// The billable part of every recording in `range`, archived projects included; a running recording
/// counts until `now`.
pub async fn billable(db: &Db, range: &Range, today: Day, now: Ms) -> AppResult<Billable> {
    let filter = ReportFilter {
        range: *range,
        include_archived: true,
        ..ReportFilter::default()
    };
    Ok(run(db, &filter, today, now).await?.billable)
}

/// `6:24 of 8:00 (80 %)`; `None` without recorded time.
pub fn billable_text(billable: &Billable) -> Option<String> {
    let percent = billable.percent?;
    Some(format!(
        "{} of {} ({percent} %)",
        format_hm(billable.billable_ms),
        format_hm(billable.recorded_ms)
    ))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::db::projects::{self, Project, ProjectInput};
    use crate::error::ErrorKind;
    use crate::reports::testing::*;
    use crate::reports::*;
    use crate::store::settings::{Group, Rounding};

    #[test]
    fn the_billable_share_counts_the_billable_projects_over_all_recorded_time() {
        let result = billing_report(
            &by(Group::Project),
            &["p-portal", "p-site"],
            vec![
                on("r1", "p-portal", D, 30),
                rec("r2", "p-site", at(D, 11, 0), Some(at(D, 11, 30)), "", None),
                rec("r3", "p-api", at(D, 13, 0), Some(at(D, 13, 40)), "", None),
            ],
        );

        assert_eq!(
            result.billable,
            Billable {
                recorded_ms: minutes(100),
                billable_ms: minutes(60),
                percent: Some(60),
            }
        );
    }

    #[test]
    fn the_billable_share_adds_up_with_the_rounded_times() {
        let filter = ReportFilter {
            rounding: Rounding::Nearest15,
            ..by(Group::Project)
        };

        let result = billing_report(
            &filter,
            &["p-portal"],
            vec![
                on("r1", "p-portal", D, 7),
                rec("r2", "p-api", at(D, 11, 0), Some(at(D, 11, 40)), "", None),
            ],
        );

        assert_eq!(result.total_rounded_ms, minutes(60));
        assert_eq!(
            result.billable,
            Billable {
                recorded_ms: minutes(60),
                billable_ms: minutes(15),
                percent: Some(25),
            }
        );
    }

    #[test]
    fn the_billable_percent_is_rounded_to_a_whole_number() {
        for (billable_minutes, other_minutes, percent) in [
            (20, 10, 67),
            (10, 20, 33),
            (30, 0, 100),
            (0, 30, 0),
            (1, 299, 0),
        ] {
            let mut recordings = Vec::new();
            if billable_minutes > 0 {
                recordings.push(on("r1", "p-portal", D, billable_minutes));
            }
            if other_minutes > 0 {
                recordings.push(on("r2", "p-api", D, other_minutes));
            }

            let result = billing_report(&by(Group::Project), &["p-portal"], recordings);

            assert_eq!(
                result.billable.percent,
                Some(percent),
                "{billable_minutes} of {}",
                billable_minutes + other_minutes
            );
        }
    }

    #[test]
    fn without_recordings_nothing_is_billable_and_the_percent_is_none() {
        let result = billing_report(&by(Group::Project), &["p-portal"], vec![]);

        assert_eq!(
            result.billable,
            Billable {
                recorded_ms: 0,
                billable_ms: 0,
                percent: None,
            }
        );
    }

    #[test]
    fn billable_time_follows_the_report_filter() {
        let filter = ReportFilter {
            project_ids: vec!["p-portal".into()],
            ..by(Group::Project)
        };

        let result = billing_report(
            &filter,
            &["p-portal"],
            vec![on("r1", "p-portal", D, 30), on("r2", "p-api", D, 30)],
        );

        assert_eq!(result.billable, Billable::of(minutes(30), minutes(30)));
    }

    #[test]
    fn billable_serializes_with_camel_case_fields() {
        assert_eq!(
            serde_json::to_value(Billable::of(minutes(480), minutes(384))).unwrap(),
            json!({ "recordedMs": minutes(480), "billableMs": minutes(384), "percent": 80 })
        );
        assert_eq!(
            serde_json::to_value(Billable::of(0, 0)).unwrap(),
            json!({ "recordedMs": 0, "billableMs": 0, "percent": null })
        );
    }

    #[test]
    fn billable_text_names_billable_and_recorded_time_and_the_percent() {
        for (recorded, billable, expected) in [
            (480, 384, Some("6:24 of 8:00 (80 %)")),
            (65, 5, Some("0:05 of 1:05 (8 %)")),
            (600, 0, Some("0:00 of 10:00 (0 %)")),
            (0, 0, None),
        ] {
            assert_eq!(
                billable_text(&Billable::of(minutes(recorded), minutes(billable))).as_deref(),
                expected,
                "{billable} of {recorded}"
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

    async fn record(db: &Db, project: &Project, d: &str, from: (i64, i64), to: Option<(i64, i64)>) {
        crate::test_support::recording(
            db,
            &project.id,
            at(d, from.0, from.1),
            to.map(|(h, m)| at(d, h, m)),
        )
        .await;
    }

    #[tokio::test]
    async fn billable_counts_the_stored_recordings_of_billable_projects() {
        let db = Db::open_memory().await.unwrap();
        let work = stored_project(&db, "Work", true).await;
        let chores = stored_project(&db, "Chores", false).await;
        record(&db, &work, "2026-10-21", (9, 0), Some((11, 0))).await;
        record(&db, &chores, "2026-10-21", (12, 0), Some((13, 0))).await;
        record(&db, &work, "2026-10-20", (9, 0), Some((10, 0))).await;

        let billable = billable(&db, &Range::Today, day("2026-10-21"), now())
            .await
            .unwrap();

        assert_eq!(billable, Billable::of(minutes(180), minutes(120)));
        assert_eq!(billable.percent, Some(67));
    }

    #[tokio::test]
    async fn billable_counts_only_the_ranges_part_of_a_span_over_midnight() {
        let db = Db::open_memory().await.unwrap();
        let work = stored_project(&db, "Work", true).await;
        crate::test_support::recording(
            &db,
            &work.id,
            at("2026-10-20", 22, 0),
            Some(at("2026-10-21", 2, 0)),
        )
        .await;

        let today = day("2026-10-21");
        let after = billable(&db, &Range::Today, today, now()).await.unwrap();
        let before = billable(&db, &Range::Yesterday, today, now())
            .await
            .unwrap();

        assert_eq!(after, Billable::of(minutes(120), minutes(120)));
        assert_eq!(before, Billable::of(minutes(120), minutes(120)));
    }

    #[tokio::test]
    async fn billable_counts_a_running_timer_up_to_now() {
        let db = Db::open_memory().await.unwrap();
        let work = stored_project(&db, "Work", true).await;
        record(&db, &work, "2026-10-21", (9, 0), None).await;

        let billable = billable(
            &db,
            &Range::Today,
            day("2026-10-21"),
            at("2026-10-21", 10, 30),
        )
        .await
        .unwrap();

        assert_eq!(billable, Billable::of(minutes(90), minutes(90)));
    }

    #[tokio::test]
    async fn billable_includes_archived_projects() {
        let db = Db::open_memory().await.unwrap();
        let old = stored_project(&db, "Old", true).await;
        let current = stored_project(&db, "Current", false).await;
        record(&db, &old, "2026-10-21", (9, 0), Some((10, 0))).await;
        record(&db, &current, "2026-10-21", (11, 0), Some((12, 0))).await;
        projects::set_archived(&db, &old.id, true).await.unwrap();

        let billable = billable(&db, &Range::Today, day("2026-10-21"), now())
            .await
            .unwrap();

        assert_eq!(billable, Billable::of(minutes(120), minutes(60)));
    }

    #[tokio::test]
    async fn billable_does_not_round_the_recorded_time() {
        let db = Db::open_memory().await.unwrap();
        let work = stored_project(&db, "Work", true).await;
        record(&db, &work, "2026-10-21", (9, 0), Some((9, 7))).await;

        let billable = billable(&db, &Range::Today, day("2026-10-21"), now())
            .await
            .unwrap();

        assert_eq!(billable, Billable::of(minutes(7), minutes(7)));
    }

    #[tokio::test]
    async fn billable_is_none_for_a_range_without_recordings() {
        let db = Db::open_memory().await.unwrap();
        let work = stored_project(&db, "Work", true).await;
        record(&db, &work, "2026-10-20", (9, 0), Some((10, 0))).await;

        let billable = billable(&db, &Range::Today, day("2026-10-21"), now())
            .await
            .unwrap();

        assert_eq!(billable, Billable::of(0, 0));
        assert_eq!(billable.percent, None);
    }

    #[tokio::test]
    async fn billable_covers_weeks_months_and_custom_ranges() {
        let db = Db::open_memory().await.unwrap();
        let work = stored_project(&db, "Work", true).await;
        let chores = stored_project(&db, "Chores", false).await;
        record(&db, &work, "2026-10-19", (9, 0), Some((10, 0))).await;
        record(&db, &chores, "2026-10-25", (9, 0), Some((10, 0))).await;
        record(&db, &work, "2026-10-26", (9, 0), Some((9, 30))).await;
        record(&db, &chores, "2026-09-30", (9, 0), Some((11, 0))).await;
        let today = day("2026-10-21");

        for (range, recorded, billable_part) in [
            (Range::ThisWeek, 120, 60),
            (Range::LastWeek, 0, 0),
            (Range::ThisMonth, 150, 90),
            (Range::LastMonth, 120, 0),
            (custom("2026-10-25", "2026-10-26"), 90, 30),
        ] {
            assert_eq!(
                billable(&db, &range, today, now()).await.unwrap(),
                Billable::of(minutes(recorded), minutes(billable_part)),
                "{range:?}"
            );
        }
    }

    #[tokio::test]
    async fn billable_rejects_a_reversed_custom_range() {
        let db = Db::open_memory().await.unwrap();

        let err = billable(
            &db,
            &custom("2026-10-07", "2026-10-01"),
            day("2026-10-21"),
            now(),
        )
        .await
        .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }
}
