# 0026. Timeline pointer input snaps to Rust-provided ticks

## Status

Accepted (2026-10-22), implemented. Supersedes in part [0001](0001-rust-owns-the-logic.md), the consequence
that snapping a dragged edge is a round trip: Rust still snaps, but sends the grid ahead.
[0011](0011-utc-milliseconds-local-days.md) stands: Rust still computes every time; the page only picks from
times Rust sent.

## Context

A drag on the Day view sent the pointer's raw position, in milliseconds, to Rust on every move
(`recording_snap` from `useSnappedSpan`) and drew the raw span until an answer arrived. Answers came back
late and out of step, and Rust snapped both ends each time, so the dragged edge and the fixed one jumped
between marks while the mouse moved. The owner's rule: the app never knows where the pointer really is. A
press becomes a 5-minute mark (13:28 is 13:30); that origin never changes; each move becomes the nearest
mark; nothing drawn is ever off the grid.

## Decision

- **Rust sends the grid.** `DayView.ticks` lists every mark of the snap grid (`timeline.snapMinutes`) from
  the day's first instant to its last, both included, ascending (`recordings::ticks`). It is a plain list
  rather than start and step, so a DST day is right without the page knowing: 277 marks on a 23 h day at
  5 min, 301 on a 25 h one (the repeated hour twice).
- **One event wrapper.** `tickPointer` (`src/components/day/tickPointer.ts`) maps a pointer's y to the index
  of the tick drawn nearest it, through the same `y()` the timeline draws with. It is pixel geometry only: no
  arithmetic on times, no rounding of times, no IPC. Every timeline drag reads the pointer through it.
- **The page picks and clamps tick indexes.** `spanDrag.ts` (`selectedSpan`, `spanWithEdge`, `spanMoved`)
  chooses which tick each edge takes and keeps edges a tick apart and inside the grid; it compares times Rust
  sent but never computes one.
- **A fixed origin.** A selection drag keeps the tick it was pressed on; the span runs from it to the
  pointer's tick, one tick long when they are the same. An edge drag keeps the other edge exactly as it is,
  also off the grid (an old recording at 13:28), and the dragged edge stays at least one tick past it. A move
  shifts both edges by whole ticks.
- **Render ticks, write once.** The block, its labels and the editor's From and To show the tick values while
  the pointer moves. On release one call writes: `recording_draft_resize`, `recording_update_span` or
  `recording_snap` (which a block selection still needs). Rust snaps the moving edge only (`snap_moving`),
  which leaves a tick unchanged and the fixed edge alone, so nothing moves on release.

## Consequences

- No IPC per pointer move; `useSnappedSpan`, `msOfPx` and `spanAfterDrag` are gone, and `recording_snap`
  lost its `moving` argument.
- `snap_moving` changed meaning: the edge a drag holds is snapped and kept past the other, which keeps its
  time; before, both ends were snapped. `Recordings::drag_span` no longer snaps the span again on save.
- A day view is a few hundred numbers larger.
