# 0012. Timeline layout and timeline settings are pure Rust functions

## Status

Accepted (2026-10-21), implemented.

Amended 2026-10-21: the function is `day::view(&DayInput, &ViewSettings) -> DayView`; `DayInput` holds the
day, its bounds, now, the rows and the project and tag names. `day::commands` adds the rules' suggestions
after layout.

## Context

The Day view applies Timely's settings: a summary lane per interval, grouping by app, continuity (bridging
short gaps), noise cancellation (hiding short spans), title choice, per-app overrides, hidden apps, lanes for
overlapping spans and external sources, "covered" marking. These transforms interact, must never change the
stored data and must be fast for a day with thousands of rows.

## Decision

`day::view(day, settings, rows) -> DayView` is a pure function in Rust over the day's activity, away spans
and recordings: it groups, bridges, filters, packs into lanes (`col`/`cols` per block), builds the summary
lane and marks covered blocks. Tauri's `day_view(day)` reads the rows and calls it. The page positions blocks
from `start`, `end`, `col` and `cols` only. Settings changes re-run the function; nothing is cached across
days. Doing layout in the page was rejected (ADR 0001); caching layouts was rejected until a measurement
shows a need.

## Consequences

- Every timeline setting is a table-driven unit test on `day::view`.
- The page component is a renderer: no grouping or gap logic in React.
- Changing a setting costs one IPC round trip and one pass over the day's rows.
