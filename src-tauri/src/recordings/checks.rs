//! Validation of what a write brings: spans, note, project and tag.

use super::{MINUTE_MS, Span, snap_span};
use crate::db::projects::{self, Project};
use crate::db::recordings::{Recording, RecordingSpan};
use crate::db::time::Ms;
use crate::db::{Db, tags};
use crate::error::{AppError, AppResult};

const MAX_SPAN_MS: Ms = 24 * 60 * MINUTE_MS;
const MAX_NOTE_CHARS: usize = 500;

pub(super) fn invalid(field: &str, message: &str) -> AppError {
    AppError::InvalidField {
        field: field.into(),
        message: message.into(),
    }
}

pub(super) fn timer_running() -> AppError {
    AppError::InvalidInput("Stop the timer first".into())
}

pub(super) fn checked_note(note: &str) -> AppResult<String> {
    if note.contains(['\n', '\r']) {
        return Err(invalid("note", "must be a single line"));
    }
    let note = note.trim();
    if note.chars().count() > MAX_NOTE_CHARS {
        return Err(invalid(
            "note",
            &format!("must be at most {MAX_NOTE_CHARS} characters"),
        ));
    }
    Ok(note.to_string())
}

fn check_length(span: Span) -> AppResult<()> {
    if span.end - span.start > MAX_SPAN_MS {
        let hours = MAX_SPAN_MS / (60 * MINUTE_MS);
        return Err(invalid(
            "end",
            &format!("must be at most {hours} h after the start"),
        ));
    }
    Ok(())
}

/// `span` checked and snapped.
pub(super) fn snapped_span(span: Span, snap_minutes: u32) -> AppResult<Span> {
    if span.end <= span.start {
        return Err(invalid("end", "must be after the start"));
    }
    let span = snap_span(span.start, span.end, snap_minutes);
    check_length(span)?;
    Ok(span)
}

/// Each span checked and snapped, by start; refused when there is none. Whether they overlap each other
/// is `db::recordings::check_spans`'s call once they belong to a recording.
pub(super) fn snapped_spans(spans: &[Span], snap_minutes: u32) -> AppResult<Vec<Span>> {
    if spans.is_empty() {
        return Err(invalid("spans", "must hold at least one span"));
    }
    let mut snapped = spans
        .iter()
        .map(|&span| snapped_span(span, snap_minutes))
        .collect::<AppResult<Vec<_>>>()?;
    snapped.sort_by_key(|s| s.start);
    Ok(snapped)
}

/// Refuses the running timer: only finished recordings are edited.
pub(super) fn require_finished(recording: &Recording) -> AppResult<()> {
    if recording.is_running() {
        return Err(timer_running());
    }
    Ok(())
}

/// `span` closed; the running timer's open span is refused.
pub(super) fn closed_span(span: &RecordingSpan) -> AppResult<Span> {
    let end = span.end.ok_or_else(timer_running)?;
    Ok(Span {
        start: span.start,
        end,
    })
}

/// The project; an archived one only when the recording already belongs to it.
pub(super) async fn usable_project(db: &Db, id: &str, current: Option<&str>) -> AppResult<Project> {
    let project = projects::get(db, id).await?;
    if project.archived_at.is_some() && current != Some(id) {
        return Err(invalid("projectId", "is archived"));
    }
    Ok(project)
}

pub(super) async fn check_tag(db: &Db, tag_id: Option<&str>) -> AppResult<()> {
    let Some(id) = tag_id else { return Ok(()) };
    if tags::list(db).await?.iter().any(|t| t.id == id) {
        Ok(())
    } else {
        Err(invalid("tagId", "is an unknown tag"))
    }
}
