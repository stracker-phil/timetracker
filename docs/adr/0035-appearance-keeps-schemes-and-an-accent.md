# 0035. Appearance keeps a scheme per mode and an accent

## Status

Accepted (2026-11-04). Owner's decision. Confirms [0022](0022-a-scheme-per-system-mode.md) against Taskboard
ADR 0028.

## Context

Taskboard reduced its appearance to Automatic, Light or Dark with two palettes and no accent picker, so the
app can override the macOS mode. TimeTracker follows the macOS mode and lets the owner pick a scheme for
each mode (Default, Light, Dark, built from four palettes) and an app accent. The modernization asked whether
to follow Taskboard.

## Decision

- TimeTracker keeps ADR 0022: the app follows the macOS mode, Settings → Appearance picks a scheme per mode
  and an accent (System or one of the nine hues). There is no app-level Light or Dark override.
- The nine hues stay as project and client colors, as before.
- Rejected:
  - Taskboard's Automatic / Light / Dark with two palettes, because it would drop the Default scheme (a dark
    sidebar beside light content) and the accent picker that TimeTracker has, and need a settings migration
    (the owner chose to keep them, 2026-11-04);
  - an app override of the macOS mode, because ADR 0022 chose to follow the system on purpose.

## Consequences

- No settings migration; `lightScheme`, `darkScheme` and `accent` stay.
- The palettes stay four complete token sets (`src/styles/palettes/`), and the scheme audit in `docs/ux.md`
  keeps covering every scheme in both modes.
