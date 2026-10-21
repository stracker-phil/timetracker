# 0007. The tracker runs inside the app process; the app keeps running without a window

## Status

Accepted (2026-10-21)

## Context

Tracking must start at login and continue when the main window is closed. Timing ships a separate helper
process for this; it survives the main app and needs its own install, permission grant and IPC. The PRD wants
a single light native app.

## Decision

One process. The Tauri app keeps the `Regular` activation policy (Dock icon, ⌘Tab), shows a tray status
item, hides the window on close (`WindowEvent::CloseRequested` → `prevent_close` + hide) and refuses the
implicit exit when the last window closes (`RunEvent::ExitRequested { code: None }` → `prevent_exit`). Quit is
explicit: tray menu, ⌘Q with the window focused, or `timetracker quit`. Launch at login is a LaunchAgent via
`tauri-plugin-autostart` with `--hidden`. The tracker is a tokio task owned by `AppState`. A separate daemon
was rejected for the second permission grant, the second install step and the IPC it would need; a
menu-bar-only agent (`LSUIElement`) was rejected because the main window is used daily and should be
discoverable.

## Consequences

- One Accessibility grant, one install, one log.
- A crash of the UI process stops tracking; `panic = "unwind"` (ADR 0014) keeps bugs from becoming crashes.
- The webview stays loaded while hidden; it must be cheap when idle (no polling loops while hidden).
- A debug rebuild changes the bundle, so TCC may forget the Accessibility grant; dev builds sign ad hoc with
  the stable identifier.
