# 0044. The open editor keeps every change until Save

## Status

Accepted (2026-11-11), implemented.

## Context

The recording editor saved an existing recording's time changes the moment they were made: a dragged span, a
typed time, a clicked block or selection taken in. Save then wrote project, note and tag only, and Cancel dropped
only those. Taking a suggestion's time into a recording therefore covered the suggestion at once, and Cancel
could not bring it back. A click on a suggestion while the editor was open also left the editor for that
suggestion, so a suggestion could not be added to the recording being edited. In use, the open editor is the
strongest sign of what the owner wants: to edit that recording.

## Decision

- **Every change waits for Save**, the times of an existing recording included, as a new recording's and a
  suggestion's always did. Save writes project, note, tag and the changed times as one undoable change; Cancel
  (Escape) drops every change. Stop Timer saves the changed times before it stops the timer.
- **While the editor is open, clicks add to it.** A click on an activity or summary block toggles its span (as
  before); a click on a suggestion adds the time it shows. Time the draft covers hides from the suggestions in the
  time column (the timesheet hides a suggestion card the draft covers whole), so time the draft lets go of shows
  as the suggestion again.
- **Another recording opens only from an unchanged editor**; a changed one stays open with a toast asking to save
  or cancel first. Leaving the view still asks "Discard changes?".
- Rejected: keeping times saved at once and restoring covered suggestions on Cancel through undo (Cancel would
  write, and an undo stack entry per click is noise); a dialog for each click on another recording (the click
  was most likely meant for the open recording).

## Consequences

- The time column draws the draft's spans in place of the edited recording's closed spans; the running timer's
  open span keeps its block, whose start drag changes the draft.
- The suggestions' parts outside the draft are computed in the page (`lib/suggestionParts.ts`), from what Rust
  sent for the day: drawing only, nothing is saved from them.
- Supersedes the "times save at once on a recording" rule that `docs/ux.md` described since 2026-11-05.
