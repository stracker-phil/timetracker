# 0020. A recording holds one or more spans of time

## Status

Accepted (2026-10-22), implemented. Supersedes in part
[0010](0010-recordings-never-overlap-ops-with-undo.md) (what overlaps and what a replace cuts).

## Context

A recording was one stretch of time: `start` and `end`. In use the owner works on one thing in several
sittings a day and wants one entry for it, as Timely does: "WEB-214: Checkout validation" is 14:25–15:00 plus
16:05–20:15, 4 h 45 m in total, with one note and one tag. As two recordings, the report listed the note
twice and the editor had to keep two copies of project, note and tag in step.

## Decision

A **recording** is what the user confirms for a project: project, note, tag, created and updated times. Its
time is a list of **spans** (`recording_spans`: `id`, `recording_id`, `start`, `end`), at least one, by start,
never overlapping each other (touching is allowed). Only the running timer has an open span (`end` NULL),
and it is the recording's last. `recordings` keeps no cached start or end; a reader that sorts by time uses
the first span.

- **Overlap.** ADR 0010's rule applies per span: no span shares time with a span of another recording.
  `Overlap::Cancel` refuses with the recordings in the way; `Overlap::Replace` removes a span inside the new
  time, trims one that sticks out on one side, and cuts one that sticks out on both into two spans of the
  same recording (0010 made the right part a new recording). A recording left without spans is deleted.
- **Days and totals.** A recording's duration is the sum of its spans. It belongs to every day one of its
  spans touches; a day shows only the spans on that day (one block each) and counts only their parts inside
  it, so a day's, week's and month's totals still add up to the time recorded.
- **Operations.** Every change is still an `Op` on whole recordings (with their spans and span ids), so undo
  needs nothing new. The service adds `add_span`, `update_span`, `remove_span` (the last one deletes the
  recording), `split_span` (two touching spans) and `merge` (the spans of B join A; B is deleted).
- **Move and copy.** With a span id, one span moves (the Week view's drag) or is copied as a new recording;
  without, the whole recording moves by the days between its first span's day and the target day, each span
  at its own local time.
- **Timer.** The timer's recording has the open span; stopping closes it. Starting a timer for the project,
  note and tag of a finished recording of today continues that recording with a new span.
- **Surfaces.** The CLI takes `--span 14:25-15:00` (repeatable; `--from`/`--to` is one span); MCP and reports
  list a recording's spans per day; a report row is one recording and day with all its spans, and a group's
  notes follow the rows' first spans. Rule suggestions stay single-span; offering "add to <recording>" when
  one is approved next to a recording of the same project and note is left for later.

Keeping `start` and `end` on `recordings` as a cache was rejected: no reader needs it for speed (the
first-span subquery uses the `(recording_id, start)` index), and a second copy of the time could disagree
with the spans.

## Consequences

- Migration `0007_recording_spans.sql` gives every recording one span under its own id; durations are
  unchanged. The backup format moves to version 3 (`spans` instead of `start` and `end`).
- `RecordingInput` and `RecordingPatch` take `spans`; `Recording` carries `spans: RecordingSpan[]`;
  `RecordingBlock` gains `spanId` and `spanCount`; `ReportRow` gains `spans`.
- The current editor edits one span and lists the others; the Day view's linked blocks and the inline span
  editor come later on this model.
