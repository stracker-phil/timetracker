# 0010. Recordings never overlap; edits are Rust-side operations with inverses

## Status

Accepted (2026-10-21)

## Context

Recordings are the deliverable: hours per project must add up, so two recordings cannot claim the same
minute. Arranging blocks (drop, resize, move, delete, accept a suggestion, stop a timer) must be undoable,
including across the Day, Week and timer code paths. DataBee solved writes with a plan → apply → undo
pipeline kept in Rust.

## Decision

The overlap rule is enforced in Rust on every write: a change that would overlap returns
`AppError::Overlap { recordings }` unless the caller passes `Overlap::Replace`, which trims or splits the
overlapped recordings (a recording fully inside the new span is deleted, a partially covered one is cut).
Every write to recordings is a `recordings::Op` with a computed inverse; `AppState.undo` keeps a bounded
stack per app run (not persisted), `undo` and `redo` apply inverses, and every op is applied in one SQLite
transaction. The page never computes a span: it sends intent (`RecordingInput`, a patch, an id).

## Consequences

- Hours per project sum correctly by construction; week and month totals need no de-duplication.
- Every new way to write recordings (timers, suggestions, the away prompt, the CLI) goes through `Op`.
- Undo is per session; restarting the app clears it, which the PRD accepts ("short undo window").
