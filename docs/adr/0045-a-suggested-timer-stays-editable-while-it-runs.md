# 0045. A suggested timer stays editable while it runs

## Status

Accepted (2026-11-11), implemented. Refines 0041.

## Context

A timer another app starts is a running stored suggestion (ADR 0041). Opening it in the Day view offered only
Approve, which turned it into a running recording with the edited project, note and tag but dropped an edited
start. The recording got a new id, so the origin of the start (`timers::origin`, kept in memory only) no longer
matched the running timer: when the other app repeated its start, the owner's recording stopped and a new
suggestion with the original fields replaced it. A restart of the app forgot the origin as well.

## Decision

- **Only the start creates the suggestion's details.** After that, a repeat of the same start leaves the running
  timer as the owner edited it; the timer itself moves only the end.
- **The origin survives**: it is kept in `timer_origin.json` in the config folder (`TimerOriginStore`), and an
  approved suggested timer keeps the suggestion's id as its recording's id, so the origin still matches it.
- **Editing a running suggestion saves it as a suggestion.** The editor opens it like the running timer's
  recording (its start a field, its end "running") with Save instead of Approve; Save sends `suggestion_update`
  (project, note, tag and a moved start; `Recordings::update_suggestion` takes `open_start` for a running one,
  not in the future, not before midnight, not before its earlier spans end). ✓ on its card still approves it as
  the running timer (ADR 0041).
- Rejected: a column with the origin on each timer row (a migration for what one small file holds; there is at
  most one running timer); approving a running suggestion from the editor (it is not final while the timer runs).

## Consequences

- A recording and the stored suggestion it came from may share an id (only the approved suggested timer; the
  suggestion row is deleted in the same change and comes back only through undo).
