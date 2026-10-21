# 0003. Errors cross IPC as `{kind, message, code}`

## Status

Accepted (2026-10-21), implemented. Inherited from DataBee ADR 0003.

## Context

The page reacts to classes of errors: an overlapping recording asks whether to replace, a missing
Accessibility permission opens the onboarding, a paused tracker shows a resume button. Parsing English
messages for that is fragile.

## Decision

`AppError` serializes to a stable `kind` (`db`, `io`, `invalid_input`, `not_found`, `overlap`,
`permission_required`, `tracker_paused`, `session_not_found`, `cancelled`), the display `message`, an optional
`code` (SQLite extended result code) and, for form errors, `field: { path, message }`. The UI branches on
`kind` only; `toAppError` accepts only known kinds. An `overlap` error carries the overlapped recordings in
`message`'s companion field `details` so the dialog can list them.

## Consequences

- UI flows such as `overlap` and `permission_required` are plain branches, testable with a mock.
- Adding a kind touches both sides.
- Messages may change freely without breaking the UI.
