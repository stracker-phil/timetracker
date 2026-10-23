# 0028. Writes from other apps are suggestions unless final

## Status

Accepted (2026-10-23), implemented.

## Context

Other apps drive TimeTracker through the CLI and MCP: a reminder that asks "still working on X?", Claude
filling in a day. Until now `timetracker record` wrote recordings directly, so a script or a model could
change the timesheet without the owner seeing it. Rule suggestions already give the owner a review step,
but they are computed from rules and activity, and nothing could store one.

## Decision

- **Stored suggestions.** A new table, `stored_suggestions` (`db/stored_suggestions.rs`), holds proposed
  recordings: project, note, tag, spans on one local day, and the `source` that stored them (`cli`, `claude`
  or the caller's `--source`). They may share time with recordings and with each other.
- **Reviewed like a rule's.** The Day view shows each stored suggestion's spans without recorded time, and
  without the time of suggestions stored before it. Rule suggestions fill only the time left. Approving
  records all of a stored suggestion's shown pieces as one recording and deletes it, in one undo step;
  dismissing deletes it. Creating, editing and deleting one is undoable like any recording change.
- **Draft by default.** The CLI's `record`, `edit` and `delete` act on suggestions. `--final` acts on
  recordings, and `--draft` forces a suggestion even when `--final` is given (draft wins), so a wrapper
  can't be overridden by an extra flag. `edit SUGGESTION --final` and `approve` turn a suggestion into a
  recording.
- **The timer stays direct.** `start`, `stop` and `timer update` change the running recording: they are
  live control the owner asks for, and a suggested timer would have no end.
- **MCP writes through the app.** The MCP server reads the database itself but writes over the app's socket
  (ADR 0017), with the same `final` and `draft` options and source `claude`. There is no setting: a write
  is a suggestion unless the caller asks for `final`.

## Consequences

- A stored suggestion whose time is fully recorded disappears from the Day view but stays stored until it
  is approved, dismissed or deleted.
- Editing a stored suggestion with several spans in the app's editor approves the edited span only; the
  other spans go with the deleted suggestion.
- Backups carry `storedSuggestions` (backup version 4).
