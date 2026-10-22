# 0023. The Week and Month list recordings per day; a Week drag moves a recording's day

## Status

Accepted (2026-10-22), implemented. Supersedes in part [0020](0020-recordings-have-spans.md): the Week view's
drag no longer moves one span by its id.

## Context

The Week was a time grid: seven columns over a 24-hour axis, one block per span, dragged and resized like the
Day view. The owner wants Timely's Week instead: per day the recordings in time order, taller for longer
ones, and a + to add one; and a Month of whole weeks with one equal line per recording. A grid of mostly
empty hours shows little, and moving or resizing single spans belongs in the Day view, which has the
activity to judge them by.

## Decision

- **Entries, not blocks.** `calendar::entries` gives each day one entry per recording (a `TimesheetCard`:
  the day's parts of its spans added up, by start), the same for Week columns and Month cells. Rust also
  computes the Week card's height units (one per started quarter hour, from a minimum that fits a title and
  the foot to a cap at six hours), so the page only multiplies them by a size (ADR 0001, 0012).
- **Move and copy by day.** A Week card is a recording's share of one day, so its drag moves the spans
  touching that day: `recording_move` and `recording_copy` take `from: Day | null` in place of `spanId`. With
  `from`, those spans shift by the days between `from` and the target, each at its own local time and length;
  the recording's spans on other days stay. Without, the whole recording moves as before (the Day view's ⌥←
  ⌥→). No command moved one span by id from anywhere else.
- **New recording for a day.** `recording_default_span` takes an optional `day`: the half hour up to now, on
  another day at the same clock times. The Week's and Month's + open the editor on it, as a sheet.
- The Week drops time resize and move within a day; times are edited in the editor or the Day view.

## Consequences

- `DayColumn` loses `bounds` and `recordings` and gains `entries`; `MonthDay` gains `weekday` and `entries`
  and loses `projects` (`ProjectShare` is gone). `CalendarEntry` is new.
- `RecordingBlock` serves only the Day view's time column; the Week's `TimeAxis` and positioned blocks are gone.
- A recording past midnight drags from either day's card and takes only that day's spans with it; a span
  over midnight touches both days and moves with either.
