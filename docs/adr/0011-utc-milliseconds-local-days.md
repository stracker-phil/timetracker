# 0011. Timestamps are UTC milliseconds; days are local calendar days computed in Rust

## Status

Accepted (2026-10-21), implemented.

## Context

Activity spans cross midnight and DST changes. Reports group by local day, week and month. The page would
have to replicate time-zone rules to compute day bounds, and two implementations would disagree on the two
DST days a year.

## Decision

Every instant is an `i64` of UTC milliseconds (`Ms`) in SQLite and in IPC. A `Day` is a `YYYY-MM-DD` string
naming a local calendar day; `Day::bounds()` resolves it with `chrono::Local` to `(start_ms, end_ms)`, so a
day is 23, 24 or 25 hours long. Spans are stored unsplit; readers clip them to the requested day. The page
formats times for display (`Intl`) and never computes bounds, durations across days or week starts. Weeks
start on Monday. Storing local wall-clock times was rejected because a changed time zone would shift history;
ISO strings were rejected for index size and comparison cost.

## Consequences

- One implementation of calendar rules, tested in Rust with fixed zones.
- A span across midnight shows on both days with its clipped duration; totals per day add up to the span.
- A time-zone change moves the day boundaries of past days, which is correct for "what did I do on that date".
