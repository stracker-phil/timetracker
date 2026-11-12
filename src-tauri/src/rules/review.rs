//! The owner's review of the day's suggestions: approve one, several or all of a rule's (each an ordinary,
//! undoable create), approve one as edited, or dismiss them. Each looks the suggestions up again, so a
//! stale id is `not_found`. A stored suggestion is approved and dismissed whole: every piece of it the day
//! shows becomes one recording's spans, and the stored suggestion is deleted with it. A running one (a
//! suggested timer) is neither approved nor dismissed until its timer stops (ADR 0046): alone it is refused,
//! among others it is skipped. Its finished pieces, from an earlier suggestion it continued, are settled
//! without it.

use super::suggestions_of;
use super::types::Suggestion;
use crate::db::dismissed::Dismissal;
use crate::db::recordings::{Recording, RecordingPatch};
use crate::db::stored_suggestions::{self, StoredSuggestion};
use crate::db::time::Day;
use crate::error::{AppError, AppResult};
use crate::recordings::ops::Op;
use crate::recordings::{Change, Overlap, RecordingInput, Recordings, Span, suggestions_label};
use crate::store::settings::Settings;

const NO_PROJECT: &str = "The suggestion has no project yet; pick one first";
const RUNNING: &str = "The suggested timer is still running; stop it first";

/// What settling a suggestion takes: all of it, or only the finished pieces of a stored one whose timer
/// still runs.
enum Scope {
    Whole,
    Finished(StoredSuggestion),
}

impl Scope {
    fn holds(&self, piece: &Suggestion) -> bool {
        match self {
            Scope::Whole => true,
            Scope::Finished(stored) => stored.open_start.is_some_and(|open| piece.end <= open),
        }
    }
}

/// What settling `suggestion` takes; None while it is the running part of a suggested timer.
async fn scope_of(service: &Recordings<'_>, suggestion: &Suggestion) -> AppResult<Option<Scope>> {
    let Some(id) = &suggestion.stored_id else {
        return Ok(Some(Scope::Whole));
    };
    let stored = stored_suggestions::get(service.db, id).await?;
    if !stored.is_running() {
        return Ok(Some(Scope::Whole));
    }
    let scope = Scope::Finished(stored);
    Ok(scope.holds(suggestion).then_some(scope))
}

/// Refuses to settle `suggestion` while it is the running part of a suggested timer.
async fn settleable(service: &Recordings<'_>, suggestion: &Suggestion) -> AppResult<Scope> {
    scope_of(service, suggestion)
        .await?
        .ok_or_else(|| AppError::InvalidInput(RUNNING.into()))
}

/// What approving `suggestion` records, under `project_id` when given (an edit); `shown` are the day's
/// suggestions, for a stored one's other pieces in `scope`. Refused when neither names a project (ADR 0030).
fn input_of(
    suggestion: &Suggestion,
    shown: &[Suggestion],
    scope: &Scope,
    project_id: Option<String>,
) -> AppResult<RecordingInput> {
    let project_id = project_id
        .or_else(|| suggestion.project_id.clone())
        .ok_or_else(|| AppError::InvalidInput(NO_PROJECT.into()))?;
    let pieces: Vec<&Suggestion> = match &suggestion.stored_id {
        Some(id) => shown
            .iter()
            .filter(|s| s.stored_id.as_ref() == Some(id) && scope.holds(s))
            .collect(),
        None => vec![suggestion],
    };
    Ok(RecordingInput {
        project_id,
        spans: pieces
            .iter()
            .map(|s| Span {
                start: s.start,
                end: s.end,
            })
            .collect(),
        note: suggestion.note.clone(),
        tag_id: suggestion.tag_id.clone(),
    })
}

/// What settles `suggestion` once approved or dismissed: a rule's dismissals (one per rule, each over all
/// of its time), the stored suggestion's deletion, or, while its timer runs, its finished spans' removal.
async fn settling(
    service: &Recordings<'_>,
    day: Day,
    suggestion: &Suggestion,
    scope: Scope,
) -> AppResult<Vec<Op>> {
    if let Scope::Finished(stored) = scope {
        let after = StoredSuggestion {
            spans: Vec::new(),
            ..stored.clone()
        };
        return Ok(vec![Op::Suggestion {
            before: Some(stored),
            after: Some(after),
        }]);
    }
    if let Some(id) = &suggestion.stored_id {
        return Ok(vec![deleted(
            stored_suggestions::get(service.db, id).await?,
        )]);
    }
    Ok(suggestion
        .rule_ids
        .iter()
        .map(|rule_id| Op::Dismiss {
            dismissal: Dismissal {
                rule_id: rule_id.clone(),
                day,
                start: suggestion.start,
                end: suggestion.end,
            },
        })
        .collect())
}

fn deleted(suggestion: StoredSuggestion) -> Op {
    Op::Suggestion {
        before: Some(suggestion),
        after: None,
    }
}

/// One entry per stored suggestion: its first piece stands for it.
fn one_per_stored(found: Vec<Suggestion>) -> Vec<Suggestion> {
    let mut seen = Vec::new();
    found
        .into_iter()
        .filter(|s| match &s.stored_id {
            Some(id) if seen.contains(id) => false,
            Some(id) => {
                seen.push(id.clone());
                true
            }
            None => true,
        })
        .collect()
}

/// The day's suggestions that `keep` holds for, and all it shows (for a stored one's other pieces).
struct Found {
    found: Vec<Suggestion>,
    shown: Vec<Suggestion>,
}

/// `not_found` with `gone` when no suggestion `keep` holds for is left.
async fn find_where(
    service: &Recordings<'_>,
    settings: &Settings,
    day: Day,
    keep: impl Fn(&Suggestion) -> bool,
    gone: &str,
) -> AppResult<Found> {
    let shown = suggestions_of(service.db, day, settings).await?;
    let found: Vec<Suggestion> = shown.iter().filter(|s| keep(s)).cloned().collect();
    if found.is_empty() {
        return Err(AppError::NotFound(gone.into()));
    }
    Ok(Found { found, shown })
}

/// The day's suggestions with one of `ids`.
async fn find_all(
    service: &Recordings<'_>,
    settings: &Settings,
    day: Day,
    ids: &[String],
) -> AppResult<Found> {
    let gone = if ids.len() == 1 {
        "The suggestion is gone"
    } else {
        "The suggestions are gone"
    };
    find_where(service, settings, day, |s| ids.contains(&s.id), gone).await
}

/// What approving `suggestion` settles besides creating its recording: a stored one is deleted (or loses
/// its finished spans), a rule's needs nothing (its time is recorded).
async fn settled_by_approval(
    service: &Recordings<'_>,
    day: Day,
    suggestion: &Suggestion,
    scope: Scope,
) -> AppResult<Vec<Op>> {
    match suggestion.stored_id {
        Some(_) => settling(service, day, suggestion, scope).await,
        None => Ok(Vec::new()),
    }
}

/// One entry per stored suggestion, each with its scope; a running part is skipped.
async fn settleable_of(
    service: &Recordings<'_>,
    found: Vec<Suggestion>,
) -> AppResult<Vec<(Suggestion, Scope)>> {
    let mut waiting = Vec::new();
    for suggestion in one_per_stored(found) {
        if let Some(scope) = scope_of(service, &suggestion).await? {
            waiting.push((suggestion, scope));
        }
    }
    Ok(waiting)
}

async fn approve_found(
    service: &Recordings<'_>,
    day: Day,
    Found { found, shown }: Found,
) -> AppResult<Change<Vec<Recording>>> {
    let waiting = settleable_of(service, found).await?;
    let label = suggestions_label("Approve", waiting.len());
    let mut inputs = Vec::new();
    for (suggestion, scope) in waiting {
        let input = input_of(&suggestion, &shown, &scope, None)?;
        let settles = settled_by_approval(service, day, &suggestion, scope).await?;
        inputs.push((input, settles));
    }
    service.create_all_with(inputs, label).await
}

/// Records the suggestion `id` of `day` as it is (a stored one with all of its pieces); refused with
/// `overlap` when a recording took part of its time.
pub async fn approve(
    service: &Recordings<'_>,
    settings: &Settings,
    day: Day,
    id: &str,
) -> AppResult<Change<Recording>> {
    let Found { found, shown } = find_all(service, settings, day, &[id.to_string()]).await?;
    let suggestion = &found[0];
    let scope = settleable(service, suggestion).await?;
    let input = input_of(suggestion, &shown, &scope, None)?;
    let settles = settled_by_approval(service, day, suggestion, scope).await?;
    service.create_with(input, Overlap::Cancel, settles).await
}

/// Records `input`, the suggestion `id` as the owner edited it, and dismisses the rest of the suggestion,
/// as one undoable change. A running one is refused; a finished piece of it settles only its finished spans.
pub async fn approve_edited(
    service: &Recordings<'_>,
    settings: &Settings,
    day: Day,
    id: &str,
    input: RecordingInput,
    overlap: Overlap,
) -> AppResult<Change<Recording>> {
    let Found { found, .. } = find_all(service, settings, day, &[id.to_string()]).await?;
    let suggestion = &found[0];
    let scope = settleable(service, suggestion).await?;
    let settles = settling(service, day, suggestion, scope).await?;
    service.create_with(input, overlap, settles).await
}

/// Records the suggestion `id` of `day` (a stored one with all of its pieces) with `patch` applied, and
/// settles the rest of it, as one undoable change. A running one is refused; a finished piece of it settles
/// only its finished spans.
pub async fn approve_patched(
    service: &Recordings<'_>,
    settings: &Settings,
    day: Day,
    id: &str,
    patch: RecordingPatch,
    overlap: Overlap,
) -> AppResult<Change<Recording>> {
    let Found { found, shown } = find_all(service, settings, day, &[id.to_string()]).await?;
    let suggestion = &found[0];
    let scope = settleable(service, suggestion).await?;
    let base = input_of(suggestion, &shown, &scope, patch.project_id)?;
    let input = RecordingInput {
        spans: patch.spans.unwrap_or(base.spans),
        note: patch.note.unwrap_or(base.note),
        tag_id: patch.tag_id.unwrap_or(base.tag_id),
        ..base
    };
    let settles = settling(service, day, suggestion, scope).await?;
    service.create_with(input, overlap, settles).await
}

/// Records the suggestions `ids` of `day` (the ones the page shows) as one undoable change; ids that are
/// gone, and a running one, are skipped.
pub async fn approve_all(
    service: &Recordings<'_>,
    settings: &Settings,
    day: Day,
    ids: &[String],
) -> AppResult<Change<Vec<Recording>>> {
    let found = find_all(service, settings, day, ids).await?;
    approve_found(service, day, found).await
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
    approve_found(service, day, found).await
}

/// Dismisses the suggestions `ids` of `day` as one undoable change: their rules suggest nothing for that
/// time again, a stored one is deleted. Returns how many were dismissed.
pub async fn dismiss(
    service: &Recordings<'_>,
    settings: &Settings,
    day: Day,
    ids: &[String],
) -> AppResult<Change<usize>> {
    let found = find_all(service, settings, day, ids).await?.found;
    let waiting = settleable_of(service, found).await?;
    if waiting.is_empty() {
        return Err(AppError::InvalidInput(RUNNING.into()));
    }
    let mut settles = Vec::new();
    for (suggestion, scope) in waiting {
        settles.push(settling(service, day, &suggestion, scope).await?);
    }
    service.dismiss_with(settles).await
}
