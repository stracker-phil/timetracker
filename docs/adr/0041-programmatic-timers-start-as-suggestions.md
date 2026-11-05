# 0041. Timers other apps start are suggestions unless final

## Status

Accepted (2026-11-05), implemented. Supersedes in part 0028 ("The timer stays direct").

## Context

ADR 0028 made every write from another app a suggestion, but kept the timer direct: `timetracker timer
start`, the MCP `timer_start` tool and any socket `timerStart` wrote a running recording at once. A reminder
or a model that starts a timer then changes the timesheet without the owner seeing it, which is what 0028
set out to prevent. A suggestion had no open end, so a timer could not be one.

## Decision

- **A stored suggestion can run.** `stored_suggestions.open_start` (migration `0013_running_suggestions.sql`)
  is where its open span starts; `NULL` once stopped. Its closed spans stay in `spans` (empty while a new one
  runs). A partial unique index keeps at most one running suggestion, and every write checks that at most
  one timer runs in total: an open recording span or a running suggestion (`ops::apply`).
- **Started from outside, it is suggested.** The CLI's `timer start` (and the hidden `start`), the MCP
  `timer_start` and a socket `timerStart` without `final` start a suggested timer
  (`Recordings::start_suggested_timer`) from their source (`cli`, the MCP client, or `--source`). It stops
  whatever runs, then continues today's finished suggestion with the same project, note, tag and source, or
  stores a new one, in one undo step. `--final` (MCP `final: true`) starts a recording as before; `--draft`
  wins over it, as for `record`.
- **The app's own starts stay final.** The timer picker, the tray's recent timers and the menus start
  recordings; a final start stops a suggested timer, and a suggested start stops a final one, unless that
  recording has the same project, note and tag (or the start repeats the one that made it): then the owner's
  timer wins and stays as it is (`already running: Acme since 09:15`).
- **One timer view.** `timers::Timer` is the running recording or the running suggestion; the status bar,
  the tray, `timer status`, `watch` and the menu's Stop Timer read it, and stop and update work on both. A
  suggested timer is marked `(suggested)` in the tray and the CLI.
- **Reviewed like any suggestion.** The Day view shows its open span up to now as a piece with the stable id
  `stored:<id>:<start>:open` (`Suggestion.running`). Approving it makes it the running timer: earlier pieces
  become closed spans and the piece reaching now the open span, and the suggestion is deleted, in one undo
  step ("Approve timer for Acme"). Approve all skips it; dismissing it deletes it and so stops it.
- Rejected: a "suggested" flag on recordings (recordings are the timesheet; every report, total and overlap
  check would have to leave them out); an open end inside the spans JSON (each reader of `spans` would have to
  learn about a span without an end, and the one-timer rule could not be an index); continuing to start
  final timers from outside (the one write that skipped review).

## Consequences

- A running suggestion takes no new spans until its timer stops ("Stop the timer first"); its project, note
  and tag can change (`timer update`, `edit ID`).
- `RunningTimer` is flat (`id`, `suggested`, `source`, `projectId`, `note`, `tagId`, `openStart`, …) instead of
  carrying the recording; `timer_stop` and `timer_update` return a `Timer` tagged by `kind`; `TimerState`
  gains `suggested`, `suggestionId` and `source`; `timer start --json` is tagged by `kind` like `record`'s.
- Backups keep version 4: a running suggestion carries `openStart`, which a reader without it may ignore.
