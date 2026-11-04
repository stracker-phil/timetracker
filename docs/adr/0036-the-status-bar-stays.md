# 0036. The status bar stays

## Status

Accepted (2026-11-04). Owner's decision. Differs from Taskboard ADR 0010, which has no status bar.

## Context

Taskboard's native look has no status bar: what a status bar would say sits in the toolbar or the sidebar.
TimeTracker's status bar (`components/StatusBar.tsx`) holds the sidebar toggle, the tracker's state while the
sidebar is hidden, the running timer (a click stops it), and the shown period's recorded time with a billable
meter. The modernization asked whether to drop it.

## Decision

- The status bar stays, below the content, on `--chrome`.
- It shows what is true right now and has no other home: the running timer and the tracker's state stay in
  sight while the sidebar is hidden, and the period totals follow every view with a period (Day, Week, Month,
  Reports).
- Rejected:
  - moving the timer and the totals into each view's toolbar, because every toolbar would repeat them and they
    would crowd the actions there;
  - showing them only in the sidebar, because they would vanish whenever the sidebar is hidden.

## Consequences

- The status bar keeps its place in `docs/ux.md` → Shell and its tests.
- A new piece of status goes into it only when it is about the whole app, not one view.
