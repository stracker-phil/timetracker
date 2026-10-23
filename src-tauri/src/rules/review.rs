//! The owner's review of the day's suggestions: approve one, several or all of a rule's (each an ordinary,
//! undoable create), approve one as edited, or dismiss them. Each looks the suggestions up again, so a
//! stale id is `not_found`.

use super::suggestions_of;
use super::types::Suggestion;
use crate::db::dismissed::Dismissal;
use crate::db::recordings::Recording;
use crate::db::time::Day;
use crate::error::{AppError, AppResult};
use crate::recordings::{Change, Overlap, RecordingInput, Recordings, Span, suggestions_label};
use crate::store::settings::Settings;

fn input_of(suggestion: Suggestion) -> RecordingInput {
    RecordingInput {
        project_id: suggestion.project_id,
        spans: vec![Span {
            start: suggestion.start,
            end: suggestion.end,
        }],
        note: suggestion.note,
        tag_id: suggestion.tag_id,
    }
}

/// One per rule of the suggestion, each over all of its time.
fn dismissals_of(day: Day, suggestion: &Suggestion) -> Vec<Dismissal> {
    suggestion
        .rule_ids
        .iter()
        .map(|rule_id| Dismissal {
            rule_id: rule_id.clone(),
            day,
            start: suggestion.start,
            end: suggestion.end,
        })
        .collect()
}

/// The day's suggestions that `keep` holds for; `not_found` with `gone` when none is left.
async fn find_where(
    service: &Recordings<'_>,
    settings: &Settings,
    day: Day,
    keep: impl Fn(&Suggestion) -> bool,
    gone: &str,
) -> AppResult<Vec<Suggestion>> {
    let found: Vec<Suggestion> = suggestions_of(service.db, day, settings)
        .await?
        .into_iter()
        .filter(|s| keep(s))
        .collect();
    if found.is_empty() {
        return Err(AppError::NotFound(gone.into()));
    }
    Ok(found)
}

/// The day's suggestions with one of `ids`.
async fn find_all(
    service: &Recordings<'_>,
    settings: &Settings,
    day: Day,
    ids: &[String],
) -> AppResult<Vec<Suggestion>> {
    let gone = if ids.len() == 1 {
        "The suggestion is gone"
    } else {
        "The suggestions are gone"
    };
    find_where(service, settings, day, |s| ids.contains(&s.id), gone).await
}

async fn find(
    service: &Recordings<'_>,
    settings: &Settings,
    day: Day,
    id: &str,
) -> AppResult<Suggestion> {
    let mut found = find_all(service, settings, day, &[id.to_string()]).await?;
    Ok(found.remove(0))
}

async fn approve_found(
    service: &Recordings<'_>,
    found: Vec<Suggestion>,
) -> AppResult<Change<Vec<Recording>>> {
    let label = suggestions_label("Approve", found.len());
    service
        .create_all(found.into_iter().map(input_of).collect(), label)
        .await
}

/// Records the suggestion `id` of `day` as it is; refused with `overlap` when a recording took part of its
/// time.
pub async fn approve(
    service: &Recordings<'_>,
    settings: &Settings,
    day: Day,
    id: &str,
) -> AppResult<Change<Recording>> {
    let suggestion = find(service, settings, day, id).await?;
    service.create(input_of(suggestion), Overlap::Cancel).await
}

/// Records `input`, the suggestion `id` as the owner edited it, and dismisses the rest of the suggestion,
/// as one undoable change.
pub async fn approve_edited(
    service: &Recordings<'_>,
    settings: &Settings,
    day: Day,
    id: &str,
    input: RecordingInput,
    overlap: Overlap,
) -> AppResult<Change<Recording>> {
    let suggestion = find(service, settings, day, id).await?;
    service
        .create_settling(input, overlap, dismissals_of(day, &suggestion))
        .await
}

/// Records the suggestions `ids` of `day` (the ones the page shows) as one undoable change; ids that are
/// gone are skipped.
pub async fn approve_all(
    service: &Recordings<'_>,
    settings: &Settings,
    day: Day,
    ids: &[String],
) -> AppResult<Change<Vec<Recording>>> {
    let found = find_all(service, settings, day, ids).await?;
    approve_found(service, found).await
}

/// Records every suggestion of the rule on `day`, those it shares with other rules too, as one undoable change.
pub async fn approve_rule(
    service: &Recordings<'_>,
    settings: &Settings,
    day: Day,
    rule_id: &str,
) -> AppResult<Change<Vec<Recording>>> {
    let found = find_where(
        service,
        settings,
        day,
        |s| s.rule_ids.iter().any(|id| id == rule_id),
        "The rule has no suggestions on this day",
    )
    .await?;
    approve_found(service, found).await
}

/// Dismisses the suggestions `ids` of `day` as one undoable change: their rules suggest nothing for that
/// time again. Returns how many were dismissed.
pub async fn dismiss(
    service: &Recordings<'_>,
    settings: &Settings,
    day: Day,
    ids: &[String],
) -> AppResult<Change<usize>> {
    let found = find_all(service, settings, day, ids).await?;
    service
        .record_dismissals(found.iter().map(|s| dismissals_of(day, s)).collect())
        .await
}
