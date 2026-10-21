# 0014. Keep `panic = "unwind"`

## Status

Accepted (2026-10-21, inherited from DataBee ADR 0017)

## Context

The app runs for days and tracks in the background. With `panic = "abort"` (the Tauri template default) a
panic in any command or task kills the process and stops tracking until the user notices. Tauri and tokio
isolate unwinding panics per command and per task.

## Decision

Use the default unwinding strategy. Runtime-reachable `unwrap`/`expect` calls with a plausible failure become
an `AppError` or a fallback. `std::sync` locks are taken through `lock::MutexExt::lock_unpoisoned` and
`RwLockExt::{read,write}_unpoisoned`, so a poisoned lock keeps working. The tracker task is supervised: if it
ends with a panic, `tracker::watch` logs it and restarts it after a short delay.

## Consequences

- A failing command returns an error; tracking survives bugs in views and reports.
- Slightly larger binary (unwind tables).
- Guarded sections are kept to single updates so a panic cannot leave data inconsistent.
