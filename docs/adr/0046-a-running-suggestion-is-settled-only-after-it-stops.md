# 0046. A running suggestion is settled only after it stops

## Status

Accepted (2026-11-11), implemented. Supersedes in part 0041 ("Approving it makes it the running timer",
"dismissing it deletes it") and 0045 (✓ on a running suggestion's card).

## Context

ADR 0041 let the owner approve a suggested timer while it ran (it became the running recording) and dismiss it
(which deleted and so stopped it). A running suggestion is not final: its end keeps moving, and the app that
started it may still be timing it. Approving or dismissing it while it runs changed what that app was timing
under its hands.

## Decision

- **While its timer runs, a suggestion cannot be approved or dismissed.** Rust refuses it alone
  (`rules::review`: "The suggested timer is still running; stop it first") and skips it among others (Approve
  all, Dismiss all, a rule's approval). The page shows no ✓ or ✕ on its card, ↵ and ⌫ do nothing on it, and
  the timer picker has no Approve.
- **It stays editable** (ADR 0045): project, note, tag and its start, saved as a suggestion.
- **Once stopped** (Stop in the picker or the menu bar, `timetracker timer stop`), it is an ordinary suggestion
  to approve or dismiss.
- The app that started it may still delete it (`timetracker delete ID`, the socket and MCP equivalents): that
  ends its own timer, not the owner's review.
- Rejected: approving into a running recording (0041; the owner's review would race the other app's timing).

## Consequences

- `Recordings::approve_suggested_timer` and the frontend's `approveTimer` are gone; `timetracker approve ID` of
  a running suggestion is refused (a breaking change: 2.0.0, ADR 0043).
