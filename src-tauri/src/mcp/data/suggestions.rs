use serde::Serialize;

use super::{Core, day_of, minutes};
use crate::db::time::Day;
use crate::error::AppResult;
use crate::scripting::listing::{self, SuggestionKind};

/// A suggestion as the Day view shows it: what rules or another app proposed, not yet on the timesheet.
#[derive(Serialize)]
pub struct SuggestionOut {
    /// What `approve`, `edit` and `delete` take.
    pub id: String,
    pub kind: SuggestionKind,
    /// The rules' names or the app that stored it.
    pub by: String,
    pub day: Day,
    pub spans: Vec<SuggestionSpanOut>,
    pub duration_min: f64,
    pub project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    pub note: String,
    /// `null` when untagged.
    pub tag: Option<String>,
}

#[derive(Serialize)]
pub struct SuggestionSpanOut {
    pub start_iso: String,
    pub end_iso: String,
}

impl Core {
    pub async fn suggestions(&self, from: &str, to: Option<&str>) -> AppResult<Vec<SuggestionOut>> {
        let from = day_of(from)?;
        let to = to.map(day_of).transpose()?.unwrap_or(from);
        let entries = listing::suggestions(self.db().await?, &self.settings(), from, to).await?;
        Ok(entries
            .into_iter()
            .map(|entry| SuggestionOut {
                spans: entry
                    .spans
                    .into_iter()
                    .map(|span| SuggestionSpanOut {
                        start_iso: span.start_iso,
                        // Only a running timer's span is open; a suggestion's never is.
                        end_iso: span.end_iso.unwrap_or_default(),
                    })
                    .collect(),
                duration_min: minutes(entry.duration_ms),
                id: entry.id,
                kind: entry.kind,
                by: entry.by,
                day: entry.day,
                project: entry.labels.project,
                client: entry.labels.client,
                note: entry.labels.note,
                tag: entry.labels.tag,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::db::recordings::Span;
    use crate::db::stored_suggestions::{self, StoredSuggestion};
    use crate::error::ErrorKind;
    use crate::mcp::data::fixtures::{DAY, Seed, at, iso, seeded};

    async fn store_on(seed: &Seed, id: &str, day: &str, spans: &[(i64, i64, i64, i64)]) {
        let day = Day::parse(day).unwrap();
        let shift = day.bounds().0 - Day::parse(DAY).unwrap().bounds().0;
        let suggestion = StoredSuggestion {
            id: id.into(),
            project_id: seed.portal.id.clone(),
            note: "review PR".into(),
            tag_id: Some(seed.development.id.clone()),
            source: "Claude Desktop".into(),
            day,
            spans: spans
                .iter()
                .map(|&(from_h, from_m, to_h, to_m)| Span {
                    start: at(from_h, from_m) + shift,
                    end: at(to_h, to_m) + shift,
                })
                .collect(),
            created_at: 1,
            updated_at: 1,
        };
        let mut conn = seed.db.pool().acquire().await.unwrap();
        stored_suggestions::put_on(&mut conn, &suggestion)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn a_stored_suggestion_shows_its_source_minutes_spans_and_labels() {
        let seed = seeded().await;
        store_on(&seed, "s1", DAY, &[(15, 0, 15, 45), (16, 0, 16, 30)]).await;

        let out = serde_json::to_value(seed.core.suggestions(DAY, None).await.unwrap()).unwrap();

        assert_eq!(
            out,
            json!([{
                "id": "s1",
                "kind": "stored",
                "by": "Claude Desktop",
                "day": DAY,
                "spans": [
                    {"start_iso": iso(15, 0), "end_iso": iso(15, 45)},
                    {"start_iso": iso(16, 0), "end_iso": iso(16, 30)},
                ],
                "duration_min": 75.0,
                "project": "Portal",
                "client": "Acme",
                "note": "review PR",
                "tag": "development",
            }])
        );
    }

    #[tokio::test]
    async fn the_last_day_defaults_to_the_first() {
        let seed = seeded().await;
        store_on(&seed, "s1", DAY, &[(15, 0, 15, 45)]).await;
        store_on(&seed, "s2", "2026-10-21", &[(15, 0, 15, 45)]).await;

        let only_first = seed.core.suggestions(DAY, None).await.unwrap();
        let both = seed
            .core
            .suggestions(DAY, Some("2026-10-21"))
            .await
            .unwrap();

        let ids = |list: &[SuggestionOut]| list.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&only_first), ["s1"]);
        assert_eq!(ids(&both), ["s1", "s2"]);
    }

    #[tokio::test]
    async fn a_day_without_suggestions_gives_an_empty_list() {
        let seed = seeded().await;

        let out = serde_json::to_value(seed.core.suggestions(DAY, None).await.unwrap()).unwrap();

        assert_eq!(out, json!([]));
    }

    #[tokio::test]
    async fn a_day_that_does_not_exist_is_refused() {
        let seed = seeded().await;

        for (from, to) in [("soon", None), (DAY, Some("later"))] {
            let err = seed.core.suggestions(from, to).await.err().unwrap();

            assert_eq!(err.kind(), ErrorKind::InvalidInput, "{from} {to:?}");
        }
    }

    #[tokio::test]
    async fn a_range_of_more_than_62_days_is_refused() {
        let seed = seeded().await;

        let err = seed
            .core
            .suggestions("2026-08-01", Some(DAY))
            .await
            .err()
            .unwrap();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }
}
