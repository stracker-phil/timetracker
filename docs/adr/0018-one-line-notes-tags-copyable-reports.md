# 0018. Notes are one line, tags classify work, reports are built for copying

## Status

Accepted (2026-10-21), implemented.

Amended 2026-10-21: reports also group by day, week and month (`store::settings::Group`); project stays the
default.

## Context

The user records time once a day or during the day and pastes the result into an external timesheet: hours
per project and what was done. Free-form multi-line notes do not paste well; a classification of the kind of
work (admin, research, development, maintenance, internal meeting, external meeting, support) is needed for
reporting and must not be typed into each note.

## Decision

A recording's `note` is one line: the editor is a single-line field, line breaks are rejected on write. Tags
are a separate many-to-many table with seeded defaults the user can edit; a recording has any number of tags.
A report groups recordings by project (default) or by tag, prints hours per group and the distinct notes of
the group in time order, and renders to plain text in Rust (`reports::text`) so the Reports screen's "Copy as
text", `timetracker report` and the MCP `report` tool print the same block. PDF and CSV export were dropped at
the user's request; the only export is the full JSON backup.

## Consequences

- The editor stays small; a note never needs formatting.
- Reports are deterministic text, tested by exact string.
- A second classification axis later (e.g. per-client codes) is a new tag group, not a new field.
