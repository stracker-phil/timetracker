//! `timetracker record`: what the CLI sends and gets back, and the overlap message it prints.

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::commands::changing;
use super::{Overlap, RecordingInput, Span};
use crate::db::recordings::{Recording, RecordingSpan};
use crate::db::time::Ms;
use crate::db::{Db, projects, tags};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// `timetracker record`: the project by id, name or unique prefix, the tag by name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordRequest {
    pub project: String,
    /// At least one.
    pub spans: Vec<Span>,
    pub note: String,
    pub tag: Option<String>,
    pub overlap: Overlap,
}

/// The saved recording with the names the CLI prints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recorded {
    #[serde(flatten)]
    pub recording: Recording,
    pub project_name: String,
}

pub async fn record(app: &AppHandle, request: RecordRequest) -> AppResult<Recorded> {
    let db = app.state::<AppState>().data.db()?;
    let project = projects::resolve(&db, &request.project).await?;
    let tag_id = tags::resolve_optional_name(&db, request.tag.as_deref()).await?;
    let input = RecordingInput {
        project_id: project.id,
        spans: request.spans,
        note: request.note,
        tag_id,
    };
    match changing(app, async |s| s.create(input, request.overlap).await).await {
        Ok(recording) => Ok(Recorded {
            recording,
            project_name: project.name,
        }),
        Err(AppError::Overlap { details, .. }) => Err(AppError::Overlap {
            message: overlap_text(&db, &details).await,
            details,
        }),
        Err(e) => Err(e),
    }
}

/// `overlaps 09:15–09:45 Acme · standup (use --replace)`.
async fn overlap_text(db: &Db, details: &serde_json::Value) -> String {
    let others: Vec<Recording> = serde_json::from_value(details.clone()).unwrap_or_default();
    let mut labels = Vec::new();
    for other in &others {
        labels.push(overlap_label(db, other).await);
    }
    format!("overlaps {} (use --replace)", labels.join(", "))
}

/// `09:15–09:45, 14:00–15:00 Acme · standup`.
async fn overlap_label(db: &Db, recording: &Recording) -> String {
    let name = projects::get(db, &recording.project_id)
        .await
        .map_or_else(|_| recording.project_id.clone(), |p| p.name);
    let note = if recording.note.is_empty() {
        String::new()
    } else {
        format!(" · {}", recording.note)
    };
    format!("{} {name}{note}", spans_text(&recording.spans, clock))
}

/// `09:15–09:45, 14:00–now`, each time through `clock`.
pub fn spans_text(spans: &[RecordingSpan], clock: impl Fn(Ms) -> String) -> String {
    spans
        .iter()
        .map(|s| {
            let end = s.end.map_or_else(|| "now".to_string(), &clock);
            format!("{}–{end}", clock(s.start))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn clock(ms: Ms) -> String {
    DateTime::from_timestamp_millis(ms)
        .map(|t| t.with_timezone(&Local).format("%H:%M").to_string())
        .unwrap_or_else(|| ms.to_string())
}
