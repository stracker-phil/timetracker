# 0051. Settings has Status and About pages

## Status

Accepted (2026-11-12), implemented. Inherited from the family catalog (settings-window; reference: Launcher
`AboutPage`/`StatusPage`, Taskboard ADR 0010). Owner's decision (2026-11-12).

## Context

The family's generic Settings pages are Appearance, Shortcuts, Status, About and Privacy & Data. TimeTracker
had Data (its Privacy & Data) but no Status or About page: the version showed nowhere in Settings, and whether
the build captured real or made-up windows, held the Accessibility permission or had its command linked could
only be read from logs or the shell.

## Decision

- **Status** (System): the build and version, what is captured (the windows in front, or made-up ones in a
  debug build), the Accessibility permission, the tracker's line, the settings folder, the database, the log
  folder and whether the `timetracker` command is linked. One Rust command, `about::app_status`, gathers the
  facts; the page only renders them.
- **About** (System, last): the version, Show Data Folder, Open Log Folder and Open README, which run the same
  bus commands as the Help menu.
- Rejected: one page for both (diagnostics would crowd the version and the folders a bug report needs);
  putting the version on the Data page (it isn't data).

## Consequences

- Both pages register their rows, so the Settings search finds "version", "Accessibility" or "log folder".
