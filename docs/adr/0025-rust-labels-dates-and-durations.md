# 0025. Rust labels dates in the region's order; one duration format

## Status

Accepted (2026-10-22), implemented. Supersedes in part [0011](0011-utc-milliseconds-local-days.md): the page
no longer formats dates for display with `Intl`.

## Context

The app showed six duration formats (`30m`, `1h 00m`, `0:30`, `0:30 h`, `3h 42m`, `15 min`) and dates in two
orders (`Oct 19 – 25, 2026` beside `19 – 25 Oct 2026`), formatted half in Rust and half in the page with
`toLocaleDateString("en-US")`. The webview does not know the macOS region (the app is not localized), so the
page could not follow it.

## Decision

Durations in the UI have one format, `1h 05m` / `30m` / `0m`, produced by `db::time::format_duration`; the
page's `formatDuration` mirrors it only for durations it shows from milliseconds (live timers, selections,
values it also draws). Settings inputs keep their units, and the copied report text and the CLI keep `3:30 h`
and `1:23`, which other tools read.

Every date label is text from Rust (`calendar/titles.rs`), in the order the macOS region reads dates:
`calendar::locale::DateOrder::system()` asks `NSDateFormatter` for the current locale's `yMMMd` pattern once
per run. Names stay English; only the day and month swap. Tests always get month first.

## Consequences

- A user in a day-first region sees `Wednesday, 21 October 2026` and `Week 43 · 19 – 25 Oct 2026`; in the US
  `Wednesday, October 21, 2026` and `Week 43 · Oct 19 – 25, 2026`.
- New IPC fields carry the labels: `ReportResult.title`, `ReportRow.dayLabel`, `DateJump.label`,
  `PreviewHit.dayLabel`, `DataInfo.firstActivityLabel`, `ActivityCount.rangeLabel`.
- Changing the region needs a restart of the app.
