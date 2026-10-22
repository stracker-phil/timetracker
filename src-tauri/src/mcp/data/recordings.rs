use serde::Serialize;

use super::{Core, day_of, minutes};
use crate::db::time::{Day, to_local_iso};
use crate::error::AppResult;
use crate::reports::cli::{ReportRequest, filter_of};
use crate::reports::{Range, ReportFilter, ReportRow};
use crate::store::settings::{Group, Rounding};

const DEFAULT_RECORDINGS: u32 = 500;
const MAX_RECORDINGS: u32 = 5000;

pub struct RecordingsQuery {
    pub from: String,
    pub to: String,
    pub project: Option<String>,
    pub tag: Option<String>,
    pub limit: Option<u32>,
}

/// A recording's part in one day: its spans of that day.
#[derive(Serialize)]
pub struct RecordingOut {
    pub id: String,
    pub day: Day,
    /// The start of the first span and the end of the last one.
    pub start_iso: String,
    pub end_iso: String,
    pub spans: Vec<RecordingSpanOut>,
    pub duration_min: f64,
    pub project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    pub note: String,
    /// `null` when untagged.
    pub tag: Option<String>,
}

#[derive(Serialize)]
pub struct RecordingSpanOut {
    pub start_iso: String,
    pub end_iso: String,
}

impl Core {
    pub async fn recordings(&self, query: RecordingsQuery) -> AppResult<Vec<RecordingOut>> {
        let (from, to) = (day_of(&query.from)?, day_of(&query.to)?);
        let request = ReportRequest {
            range: Range::Custom { from, to },
            group_by: Group::Project,
            rounding: Rounding::None,
            clients: Vec::new(),
            projects: query.project.into_iter().collect(),
            tags: query.tag.into_iter().collect(),
        };
        let db = self.db().await?;
        let filter = ReportFilter {
            include_archived: true,
            ..filter_of(db, request).await?
        };
        let result = self.run_filter(&filter).await?;
        let mut rows: Vec<ReportRow> = result.groups.into_iter().flat_map(|g| g.rows).collect();
        rows.sort_by(|a, b| (a.start, &a.id).cmp(&(b.start, &b.id)));
        let limit = query
            .limit
            .unwrap_or(DEFAULT_RECORDINGS)
            .clamp(1, MAX_RECORDINGS) as usize;
        Ok(rows
            .into_iter()
            .take(limit)
            .map(|row| RecordingOut {
                start_iso: to_local_iso(row.start),
                end_iso: to_local_iso(row.end),
                spans: row
                    .spans
                    .iter()
                    .map(|s| RecordingSpanOut {
                        start_iso: to_local_iso(s.start),
                        end_iso: to_local_iso(s.end),
                    })
                    .collect(),
                duration_min: minutes(row.duration_ms),
                id: row.id,
                day: row.day,
                project: row.project,
                client: row.client,
                note: row.note,
                tag: row.tag,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::db::projects;
    use crate::error::ErrorKind;
    use crate::mcp::data::fixtures::{DAY, Seed, add_spanned, at, iso, seeded};

    fn recordings_query(
        project: Option<&str>,
        tag: Option<&str>,
        limit: Option<u32>,
    ) -> RecordingsQuery {
        RecordingsQuery {
            from: DAY.into(),
            to: DAY.into(),
            project: project.map(String::from),
            tag: tag.map(String::from),
            limit,
        }
    }

    async fn recorded_projects(seed: &Seed, query: RecordingsQuery) -> Vec<String> {
        seed.core
            .recordings(query)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.project)
            .collect()
    }

    #[tokio::test]
    async fn recordings_come_by_start_with_local_iso_times_and_minutes() {
        let seed = seeded().await;

        let rows = seed
            .core
            .recordings(recordings_query(None, None, None))
            .await
            .unwrap();

        let notes: Vec<&str> = rows.iter().map(|r| r.note.as_str()).collect();
        assert_eq!(notes, ["outline", "fix login", "edit episode", "review"]);
        assert_eq!(
            serde_json::to_value(&rows[1]).unwrap(),
            json!({
                "id": seed.login.id,
                "day": DAY,
                "start_iso": iso(9, 0),
                "end_iso": iso(10, 30),
                "spans": [{"start_iso": iso(9, 0), "end_iso": iso(10, 30)}],
                "duration_min": 90.0,
                "project": "Portal",
                "client": "Acme",
                "note": "fix login",
                "tag": "development",
            })
        );
        assert_eq!(
            serde_json::to_value(&rows[0]).unwrap()["duration_min"],
            25.0
        );
    }

    #[tokio::test]
    async fn a_recording_with_several_spans_on_a_day_lists_them_with_the_summed_minutes() {
        let seed = seeded().await;
        let split = add_spanned(
            &seed.db,
            &seed.podcast,
            &[(at(16, 0), at(17, 0)), (at(18, 0), at(18, 30))],
            "mixing",
            None,
        )
        .await;

        let rows = seed
            .core
            .recordings(recordings_query(None, None, None))
            .await
            .unwrap();

        let row = rows.iter().find(|r| r.id == split.id).unwrap();
        assert_eq!(
            serde_json::to_value(row).unwrap(),
            json!({
                "id": split.id,
                "day": DAY,
                "start_iso": iso(16, 0),
                "end_iso": iso(18, 30),
                "spans": [
                    {"start_iso": iso(16, 0), "end_iso": iso(17, 0)},
                    {"start_iso": iso(18, 0), "end_iso": iso(18, 30)},
                ],
                "duration_min": 90.0,
                "project": "Podcast",
                "note": "mixing",
                "tag": null,
            })
        );
    }

    #[tokio::test]
    async fn a_recording_with_spans_on_two_days_is_listed_once_per_day_with_that_days_spans() {
        let seed = seeded().await;
        let next = Day::parse("2026-10-21").unwrap().bounds().0;
        let split = add_spanned(
            &seed.db,
            &seed.podcast,
            &[
                (at(19, 0), at(20, 0)),
                (next + 9 * 3_600_000, next + 10 * 3_600_000),
            ],
            "two days",
            None,
        )
        .await;
        let mut query = recordings_query(None, None, None);
        query.to = "2026-10-21".into();

        let rows = seed.core.recordings(query).await.unwrap();

        let parts: Vec<(String, usize, f64)> = rows
            .iter()
            .filter(|r| r.id == split.id)
            .map(|r| (r.day.to_string(), r.spans.len(), r.duration_min))
            .collect();
        let expected = [
            (DAY.to_string(), 1, 60.0),
            ("2026-10-21".to_string(), 1, 60.0),
        ];
        assert_eq!(parts, expected);
    }

    #[tokio::test]
    async fn an_untagged_recording_has_a_null_tag() {
        let seed = seeded().await;

        let rows = seed
            .core
            .recordings(recordings_query(None, None, None))
            .await
            .unwrap();

        assert_eq!(rows[0].note, "outline");
        assert_eq!(serde_json::to_value(&rows[0]).unwrap()["tag"], Value::Null);
    }

    #[tokio::test]
    async fn recordings_without_a_client_have_no_client_field() {
        let seed = seeded().await;

        let rows = seed
            .core
            .recordings(recordings_query(None, None, None))
            .await
            .unwrap();

        let outline = serde_json::to_value(&rows[0]).unwrap();
        assert_eq!(outline["project"], "Podcast");
        assert!(outline.get("client").is_none(), "{outline}");
    }

    #[tokio::test]
    async fn recordings_resolve_a_project_by_its_unique_prefix() {
        let seed = seeded().await;

        let projects = recorded_projects(&seed, recordings_query(Some("port"), None, None)).await;

        assert_eq!(projects, ["Portal", "Portal"]);
    }

    #[tokio::test]
    async fn recordings_refuse_an_ambiguous_prefix_and_name_both_projects() {
        let seed = seeded().await;

        let err = seed
            .core
            .recordings(recordings_query(Some("po"), None, None))
            .await
            .err()
            .unwrap();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        let message = err.to_string();
        assert!(
            message.starts_with("`po` matches several projects: "),
            "{message}"
        );
        for project in [&seed.portal, &seed.podcast] {
            let candidate = format!("{} ({})", project.name, project.id);
            assert!(message.contains(&candidate), "{candidate} in {message}");
        }
    }

    #[tokio::test]
    async fn recordings_filter_by_tag() {
        let seed = seeded().await;

        let projects =
            recorded_projects(&seed, recordings_query(None, Some("development"), None)).await;

        assert_eq!(projects, ["Portal", "Portal"]);
    }

    #[tokio::test]
    async fn recordings_keep_those_of_archived_projects() {
        let seed = seeded().await;
        projects::set_archived(&seed.db, &seed.podcast.id, true)
            .await
            .unwrap();

        let projects = recorded_projects(&seed, recordings_query(None, None, None)).await;

        assert_eq!(projects.len(), 4);
    }

    #[tokio::test]
    async fn the_recordings_limit_keeps_the_first_by_start_and_is_at_least_one() {
        let seed = seeded().await;
        for (limit, expected) in [
            (Some(2), vec!["outline", "fix login"]),
            (Some(0), vec!["outline"]),
            (
                Some(10),
                vec!["outline", "fix login", "edit episode", "review"],
            ),
            (None, vec!["outline", "fix login", "edit episode", "review"]),
        ] {
            let rows = seed
                .core
                .recordings(recordings_query(None, None, limit))
                .await
                .unwrap();

            let notes: Vec<&str> = rows.iter().map(|r| r.note.as_str()).collect();
            assert_eq!(notes, expected, "{limit:?}");
        }
    }

    #[tokio::test]
    async fn the_recordings_limit_is_capped_at_5000() {
        let seed = seeded().await;
        sqlx::query(
            r#"WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 5010)
               INSERT INTO recordings (id, project_id, note, created_at, updated_at)
               SELECT 'bulk-' || i, ?1, '', 0, 0 FROM n"#,
        )
        .bind(&seed.podcast.id)
        .execute(seed.db.pool())
        .await
        .unwrap();
        sqlx::query(
            r#"WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 5010)
               INSERT INTO recording_spans (id, recording_id, start, "end")
               SELECT 'bulk-span-' || i, 'bulk-' || i, ?1 + i * 1000, ?1 + i * 1000 + 500 FROM n"#,
        )
        .bind(at(15, 0))
        .execute(seed.db.pool())
        .await
        .unwrap();

        let rows = seed
            .core
            .recordings(recordings_query(None, None, Some(1_000_000)))
            .await
            .unwrap();

        assert_eq!(rows.len(), 5000);
    }

    #[tokio::test]
    async fn recordings_refuse_a_day_that_does_not_exist() {
        let seed = seeded().await;
        let mut query = recordings_query(None, None, None);
        query.to = "2026-13-01".into();

        let err = seed.core.recordings(query).await.err().unwrap();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }
}
