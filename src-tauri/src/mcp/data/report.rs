use chrono::Local;
use clap::ValueEnum;
use serde::Serialize;

use super::{Core, minutes};
use crate::cli::report;
use crate::db::time::{Day, now_ms};
use crate::error::{AppError, AppResult};
use crate::reports::cli::{ReportRequest, filter_of};
use crate::reports::{self, Range, ReportResult};
use crate::store::settings::{Group, Rounding};

/// What to report on; names resolve like the CLI's `--project` and `--tag`.
pub struct ReportQuery {
    pub range: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub group_by: Option<String>,
    pub client: Option<String>,
    pub project: Option<String>,
    pub tag: Option<String>,
}

#[derive(Serialize)]
pub struct ReportOut {
    pub from: Day,
    pub to: Day,
    pub group_by: Group,
    pub rounding: Rounding,
    pub total_min: f64,
    pub total_rounded_min: f64,
    /// Out of `total_rounded_min`; the percent is `null` when nothing is recorded.
    pub billable_min: f64,
    pub billable_percent: Option<u8>,
    pub groups: Vec<ReportGroupOut>,
}

#[derive(Serialize)]
pub struct ReportGroupOut {
    pub label: String,
    pub minutes: f64,
    pub rounded_min: f64,
    pub notes: Vec<NoteOut>,
}

#[derive(Serialize)]
pub struct NoteOut {
    pub text: String,
    pub days: Vec<Day>,
}

fn period_of(range: Option<String>, from: Option<String>, to: Option<String>) -> AppResult<Range> {
    let words = match (range, from, to) {
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
            return Err(AppError::InvalidInput(
                "Give either `range` or `from` and `to`, not both".into(),
            ));
        }
        (Some(range), None, None) => vec![range],
        (None, Some(from), Some(to)) => vec![from, to],
        (None, Some(day), None) | (None, None, Some(day)) => vec![day],
        (None, None, None) => Vec::new(),
    };
    report::range(&words, now_ms(), &Local).map_err(AppError::InvalidInput)
}

fn group_of(text: Option<String>) -> AppResult<Group> {
    let Some(text) = text else {
        return Ok(Group::Project);
    };
    Group::from_str(&text, false).map_err(|_| {
        AppError::InvalidInput(format!(
            "Not a grouping: `{text}` (project, tag, day, week or month)"
        ))
    })
}

impl Core {
    /// The report and its text, as the Reports screen copies it.
    pub async fn report(&self, query: ReportQuery) -> AppResult<(ReportResult, String)> {
        let request = ReportRequest {
            range: period_of(query.range, query.from, query.to)?,
            group_by: group_of(query.group_by)?,
            rounding: self.settings().reports.rounding,
            clients: query.client.into_iter().collect(),
            projects: query.project.into_iter().collect(),
            tags: query.tag.into_iter().collect(),
        };
        let filter = filter_of(self.db().await?, request).await?;
        let result = self.run_filter(&filter).await?;
        let text = reports::text(&result);
        Ok((result, text))
    }
}

pub fn report_out(result: &ReportResult) -> ReportOut {
    ReportOut {
        from: result.from,
        to: result.to,
        group_by: result.filter.group_by,
        rounding: result.filter.rounding,
        total_min: minutes(result.total_ms),
        total_rounded_min: minutes(result.total_rounded_ms),
        billable_min: minutes(result.billable.billable_ms),
        billable_percent: result.billable.percent,
        groups: result
            .groups
            .iter()
            .map(|group| ReportGroupOut {
                label: group.label.clone(),
                minutes: minutes(group.duration_ms),
                rounded_min: minutes(group.rounded_ms),
                notes: group
                    .notes
                    .iter()
                    .map(|note| NoteOut {
                        text: note.text.clone(),
                        days: note.days.clone(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::error::ErrorKind;
    use crate::mcp::data::fixtures::{DAY, add_recording, named, seeded, write_settings};
    use crate::reports::ReportFilter;

    fn report_query(range: Option<&str>) -> ReportQuery {
        ReportQuery {
            range: range.map(String::from),
            from: None,
            to: None,
            group_by: None,
            client: None,
            project: None,
            tag: None,
        }
    }

    #[tokio::test]
    async fn todays_report_by_project_is_the_report_text_of_the_same_filter() {
        let seed = seeded().await;
        let today = Day::today().bounds().0;
        add_recording(
            &seed.db,
            &seed.portal,
            (today + 3_600_000, today + 7_200_000),
            "today work",
            None,
        )
        .await;

        let (result, text) = seed.core.report(report_query(Some("today"))).await.unwrap();

        let expected = reports::run(&seed.db, &ReportFilter::default(), Day::today(), now_ms())
            .await
            .unwrap();
        assert_eq!(text, reports::text(&expected));
        assert_eq!(result.total_ms, 3_600_000);
    }

    #[tokio::test]
    async fn a_report_without_a_range_is_todays() {
        let seed = seeded().await;

        let (result, _) = seed.core.report(report_query(None)).await.unwrap();

        assert_eq!((result.from, result.to), (Day::today(), Day::today()));
    }

    #[tokio::test]
    async fn the_json_report_is_in_minutes_and_rounded_as_the_settings_say() {
        let seed = seeded().await;
        write_settings(
            seed.dir.path(),
            json!({"reports": {"rounding": "nearest30"}}),
        );

        let (result, _) = seed.core.report(report_query(Some(DAY))).await.unwrap();

        let out = serde_json::to_value(report_out(&result)).unwrap();
        let mut keys: Vec<&String> = out.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "billable_min",
                "billable_percent",
                "from",
                "group_by",
                "groups",
                "rounding",
                "to",
                "total_min",
                "total_rounded_min",
            ]
        );
        assert_eq!(out["from"], DAY);
        assert_eq!(out["to"], DAY);
        assert_eq!(out["group_by"], "project");
        assert_eq!(out["rounding"], "nearest30");
        assert_eq!(out["total_min"], 205.0);
        assert_eq!(out["total_rounded_min"], 210.0);
        assert_eq!(out["billable_min"], 150.0);
        assert_eq!(out["billable_percent"], 71);
        assert_eq!(out["groups"].as_array().unwrap().len(), 2);
        assert_eq!(
            named(&out["groups"], "label", "Acme / Portal"),
            &json!({
                "label": "Acme / Portal",
                "minutes": 150.0,
                "rounded_min": 150.0,
                "notes": [
                    {"text": "fix login", "days": [DAY]},
                    {"text": "review", "days": [DAY]},
                ],
            })
        );
        assert_eq!(
            named(&out["groups"], "label", "Podcast"),
            &json!({
                "label": "Podcast",
                "minutes": 55.0,
                "rounded_min": 60.0,
                "notes": [
                    {"text": "outline", "days": [DAY]},
                    {"text": "edit episode", "days": [DAY]},
                ],
            })
        );
    }

    #[tokio::test]
    async fn the_json_report_of_a_day_without_recordings_has_no_billable_percent() {
        let seed = seeded().await;

        let (result, _) = seed
            .core
            .report(report_query(Some("2026-10-18")))
            .await
            .unwrap();

        let out = serde_json::to_value(report_out(&result)).unwrap();
        assert_eq!(out["billable_min"], 0.0);
        assert_eq!(out["billable_percent"], Value::Null);
    }

    #[tokio::test]
    async fn a_report_takes_a_range_of_two_days() {
        let seed = seeded().await;
        let mut query = report_query(None);
        query.from = Some("2026-10-19".into());
        query.to = Some(DAY.into());

        let (result, _) = seed.core.report(query).await.unwrap();

        assert_eq!(
            (result.from.to_string(), result.to.to_string()),
            ("2026-10-19".to_string(), DAY.to_string())
        );
        assert_eq!(result.total_ms, 205 * 60_000);
    }

    #[tokio::test]
    async fn a_report_refuses_a_range_together_with_from_or_to() {
        let seed = seeded().await;
        for (from, to) in [(Some(DAY), None), (None, Some(DAY)), (Some(DAY), Some(DAY))] {
            let mut query = report_query(Some("week"));
            query.from = from.map(String::from);
            query.to = to.map(String::from);

            let err = seed.core.report(query).await.unwrap_err();

            assert_eq!(err.kind(), ErrorKind::InvalidInput, "{from:?} {to:?}");
            assert_eq!(
                err.to_string(),
                "Give either `range` or `from` and `to`, not both"
            );
        }
    }

    #[tokio::test]
    async fn a_report_refuses_an_unknown_grouping() {
        let seed = seeded().await;
        let mut query = report_query(Some(DAY));
        query.group_by = Some("year".into());

        let err = seed.core.report(query).await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(
            err.to_string(),
            "Not a grouping: `year` (project, tag, day, week or month)"
        );
    }

    #[tokio::test]
    async fn a_report_refuses_text_that_is_not_a_range() {
        let seed = seeded().await;

        let err = seed
            .core
            .report(report_query(Some("soon")))
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }

    #[tokio::test]
    async fn a_report_by_tag_narrows_to_a_tag_by_its_name_start() {
        let seed = seeded().await;
        let mut query = report_query(Some(DAY));
        query.group_by = Some("tag".into());
        query.tag = Some("dev".into());

        let (result, _) = seed.core.report(query).await.unwrap();

        let labels: Vec<&str> = result.groups.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(labels, ["development"]);
        assert_eq!(result.total_ms, 150 * 60_000);
    }

    #[tokio::test]
    async fn a_report_narrows_to_a_client_and_to_a_project() {
        let seed = seeded().await;
        let mut by_client = report_query(Some(DAY));
        by_client.client = Some("acme".into());
        let mut by_project = report_query(Some(DAY));
        by_project.project = Some("Podcast".into());

        let (clients, _) = seed.core.report(by_client).await.unwrap();
        let (projects, _) = seed.core.report(by_project).await.unwrap();

        assert_eq!(clients.total_ms, 150 * 60_000);
        assert_eq!(projects.total_ms, 55 * 60_000);
    }

    #[tokio::test]
    async fn a_report_names_the_tag_it_cannot_find() {
        let seed = seeded().await;
        let mut query = report_query(Some(DAY));
        query.tag = Some("nope".into());

        let err = seed.core.report(query).await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(err.to_string(), "No tag matches `nope`");
    }
}
