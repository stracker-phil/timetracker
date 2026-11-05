# 0040. Reads go through TanStack Query, invalidated by events

## Status

Accepted (2026-11-04), implemented. Inherited from Taskboard ADR 0018.

## Context

The page kept everything it read from Rust in zustand slices (`dayView`, the project lists, rules, settings,
the statuses) and re-read by hand: a `data` slice counted changes per day (`dayVersions`, `dataVersion`,
`changeCount`), views keyed `lib/useApiResult` on those counters, slices reloaded their lists after each write,
and `loadDay` dropped answers for a day no longer shown with a request counter. Every new view had to pick the
right counter, a counter bumped for any change reran reads the change could not touch, and the slices mixed
page state with copies of Rust's data.

## Decision

- **Reads are queries.** Everything the page reads from Rust goes through TanStack Query hooks in
  `src/queries/` (one module per domain), which call `api`. Each window has one module-level `queryClient`
  (`QueryRoot` provides it); it never retries and never goes stale on its own (`staleTime: Infinity`): data
  changes only when Rust says so. Reads without an event of their own (apps, autostart, data info, the rule
  preview, today's rules) are asked again whenever a view shows them.
- **Events invalidate.** `src/queries/useEvents.ts` maps each `data_changed` kind (and day) to the query keys
  it makes stale and refetches the shown ones; events that carry their value (`settings_changed`,
  `keybindings_changed`, `timer_status`, `tracker_status`) write it into their query.
- **Non-hook code reads the same cache** through accessors (`shownDayView()`, `currentSettings()`,
  `projectLists()`), so commands and store actions see what the page shows.
- **Zustand keeps page state and writes:** the view and day, the selection and focus, the editor's draft,
  dialogs, toasts, menus, the report filter, the window layout, and every write action. A write stores Rust's
  answer in its query or awaits the refetch (`refreshed`) where a form closes on fresh lists.
- Rejected: keeping the zustand slices refreshed by version counters (every view picks its counter, a change
  reruns reads it can't affect, and each slice hand-rolls stale-answer guards); a hand-rolled cache like
  `useApiResult` (no sharing between views of the same data, no placeholder, no invalidation by key, and every
  feature reinvents what a query library does).

## Consequences

- One key per read: two views of a day share one request and one answer, and an answer for a day no longer
  shown lands under its own key instead of needing a guard.
- What an event refreshes is one table (`DATA_INVALIDATIONS`); a new read adds its key there.
- Structural sharing keeps unchanged rows identical, so a refetch that changed nothing doesn't re-render them.
- Tests share the module client, cleared before each test; data a test starts from is seeded by query key
  rather than put into the store.
- A dependency more (`@tanstack/react-query`), and the store no longer holds a snapshot of Rust's data:
  dev scripts read the cache as `window.timetrackerQueries`.
