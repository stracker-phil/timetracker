use serde::Serialize;

use super::{Core, day_of};
use crate::db::activities::{self, ActivityRow};
use crate::db::away::{self, AwayRow};
use crate::db::time::{Ms, now_ms, to_local_iso};
use crate::error::{AppError, AppResult};

pub const ACTIVITY_HIDDEN: &str = "Activity is not exposed to MCP. Enable it in Settings → Data.";

#[derive(Serialize)]
pub struct ActivityOut {
    pub activity: Vec<SpanOut>,
    pub away: Vec<AwayOut>,
}

#[derive(Serialize)]
pub struct SpanOut {
    pub start_iso: String,
    pub end_iso: String,
    pub app: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// `tracker` or `external:<name>`.
    pub source: String,
}

#[derive(Serialize)]
pub struct AwayOut {
    pub start_iso: String,
    pub end_iso: String,
    pub reason: &'static str,
}

impl Core {
    pub async fn activity(&self, day: &str) -> AppResult<ActivityOut> {
        if !self.expose_activity() {
            return Err(AppError::PermissionRequired(ACTIVITY_HIDDEN.into()));
        }
        let (from, to) = day_of(day)?.bounds();
        let now = now_ms();
        let db = self.db().await?;
        Ok(ActivityOut {
            activity: spans_out(activities::list_clipped(db, from, to, now).await?, now),
            away: away_out(away::list_clipped(db, from, to, now).await?, now),
        })
    }
}

/// A private span keeps only its app.
fn spans_out(rows: Vec<ActivityRow>, now: Ms) -> Vec<SpanOut> {
    rows.into_iter()
        .map(|row| {
            let (title, url, path) = if row.private {
                (String::new(), None, None)
            } else {
                (row.title, row.url, row.path)
            };
            SpanOut {
                start_iso: to_local_iso(row.start),
                end_iso: to_local_iso(row.end.unwrap_or(now)),
                app: row.app,
                title,
                url,
                path,
                source: row.source,
            }
        })
        .collect()
}

fn away_out(rows: Vec<AwayRow>, now: Ms) -> Vec<AwayOut> {
    rows.into_iter()
        .map(|row| AwayOut {
            start_iso: to_local_iso(row.start),
            end_iso: to_local_iso(row.end.unwrap_or(now)),
            reason: row.reason.label(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::error::ErrorKind;
    use crate::mcp::data::fixtures::{DAY, iso, seeded, write_settings};

    #[tokio::test]
    async fn activity_is_refused_until_the_user_exposes_it() {
        let seed = seeded().await;
        assert!(!seed.core.expose_activity());

        for settings in [None, Some(json!({"mcp": {"exposeActivity": false}}))] {
            if let Some(settings) = settings {
                write_settings(seed.dir.path(), settings);
            }

            let err = seed.core.activity(DAY).await.err().unwrap();

            assert_eq!(err.kind(), ErrorKind::PermissionRequired);
            assert_eq!(err.to_string(), ACTIVITY_HIDDEN);
        }
    }

    #[tokio::test]
    async fn exposed_activity_lists_spans_and_away_spans_with_private_rows_reduced_to_the_app() {
        let seed = seeded().await;
        write_settings(seed.dir.path(), json!({"mcp": {"exposeActivity": true}}));

        let out = serde_json::to_value(seed.core.activity(DAY).await.unwrap()).unwrap();

        assert_eq!(
            out,
            json!({
                "activity": [
                    {
                        "start_iso": iso(9, 0),
                        "end_iso": iso(10, 0),
                        "app": "Xcode",
                        "title": "main.rs",
                        "path": "/src/main.rs",
                        "source": "tracker",
                    },
                    {
                        "start_iso": iso(10, 0),
                        "end_iso": iso(10, 30),
                        "app": "Safari",
                        "title": "",
                        "source": "tracker",
                    },
                    {
                        "start_iso": iso(11, 0),
                        "end_iso": iso(11, 30),
                        "app": "Planner",
                        "title": "Standup",
                        "url": "https://plan.example/standup",
                        "source": "external:planner",
                    },
                ],
                "away": [
                    {"start_iso": iso(12, 0), "end_iso": iso(12, 20), "reason": "idle"},
                ],
            })
        );
    }

    #[tokio::test]
    async fn the_settings_are_read_again_on_every_call() {
        let seed = seeded().await;
        assert!(seed.core.activity(DAY).await.is_err());

        write_settings(seed.dir.path(), json!({"mcp": {"exposeActivity": true}}));
        assert!(seed.core.expose_activity());
        assert!(seed.core.activity(DAY).await.is_ok());

        write_settings(seed.dir.path(), json!({"mcp": {"exposeActivity": false}}));
        assert!(!seed.core.expose_activity());
        assert!(seed.core.activity(DAY).await.is_err());
    }
}
