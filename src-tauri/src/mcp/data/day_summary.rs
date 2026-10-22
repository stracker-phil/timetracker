use serde::Serialize;

use super::{Core, day_of, minutes};
use crate::day;
use crate::db::time::{Day, now_ms};
use crate::error::AppResult;
use crate::reports::{Range, ReportFilter, ReportGroup};
use crate::store::settings::Group;

#[derive(Serialize)]
pub struct DaySummary {
    pub day: Day,
    pub recorded_min: f64,
    pub tracked_min: f64,
    pub away_min: f64,
    pub by_project: Vec<SummaryGroup>,
    pub by_tag: Vec<SummaryGroup>,
}

#[derive(Serialize)]
pub struct SummaryGroup {
    pub name: String,
    pub minutes: f64,
    /// Distinct notes in the order they first occur.
    pub notes: Vec<String>,
}

impl Core {
    pub async fn day_summary(&self, day: &str) -> AppResult<DaySummary> {
        let day = day_of(day)?;
        let settings = self.settings();
        let totals = day::view_of(self.db().await?, day, &settings, now_ms())
            .await?
            .totals;
        let grouped = |group_by| ReportFilter {
            range: Range::Custom { from: day, to: day },
            group_by,
            include_archived: true,
            ..ReportFilter::default()
        };
        let by_project = self.run_filter(&grouped(Group::Project)).await?;
        let by_tag = self.run_filter(&grouped(Group::Tag)).await?;
        Ok(DaySummary {
            day,
            recorded_min: minutes(totals.recorded_ms),
            tracked_min: minutes(totals.tracked_ms),
            away_min: minutes(totals.away_ms),
            by_project: summary_groups(by_project.groups),
            by_tag: summary_groups(by_tag.groups),
        })
    }
}

fn summary_groups(groups: Vec<ReportGroup>) -> Vec<SummaryGroup> {
    groups
        .into_iter()
        .map(|group| SummaryGroup {
            name: group.label,
            minutes: minutes(group.duration_ms),
            notes: group.notes.into_iter().map(|note| note.text).collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::error::ErrorKind;
    use crate::mcp::data::fixtures::{DAY, named, seeded};

    #[tokio::test]
    async fn a_day_summary_totals_the_day_and_lists_notes_in_time_order() {
        let seed = seeded().await;

        let summary = seed.core.day_summary(DAY).await.unwrap();

        let out = serde_json::to_value(&summary).unwrap();
        assert_eq!(out["day"], DAY);
        assert_eq!(out["recorded_min"], 205.0);
        assert_eq!(out["tracked_min"], 90.0);
        assert_eq!(out["away_min"], 20.0);
        assert_eq!(
            named(&out["by_project"], "name", "Acme / Portal"),
            &json!({"name": "Acme / Portal", "minutes": 150.0, "notes": ["fix login", "review"]})
        );
        assert_eq!(
            named(&out["by_project"], "name", "Podcast"),
            &json!({"name": "Podcast", "minutes": 55.0, "notes": ["outline", "edit episode"]})
        );
        assert_eq!(
            named(&out["by_tag"], "name", "development"),
            &json!({"name": "development", "minutes": 150.0, "notes": ["fix login", "review"]})
        );
        assert_eq!(
            named(&out["by_tag"], "name", "Untagged"),
            &json!({"name": "Untagged", "minutes": 55.0, "notes": ["outline", "edit episode"]})
        );
    }

    #[tokio::test]
    async fn the_summary_of_an_empty_day_is_zero() {
        let seed = seeded().await;

        let out = serde_json::to_value(seed.core.day_summary("2026-10-19").await.unwrap()).unwrap();

        assert_eq!(
            out,
            json!({
                "day": "2026-10-19",
                "recorded_min": 0.0,
                "tracked_min": 0.0,
                "away_min": 0.0,
                "by_project": [],
                "by_tag": [],
            })
        );
    }

    #[tokio::test]
    async fn a_day_summary_refuses_a_day_that_does_not_exist() {
        let seed = seeded().await;

        let err = seed.core.day_summary("last tuesday").await.err().unwrap();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(
            err.to_string(),
            "Not a day: `last tuesday` (use YYYY-MM-DD, today or yesterday)"
        );
    }
}
