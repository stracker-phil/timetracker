# 0027. Rules match the summary, not every activity

## Status

Accepted (2026-10-23), implemented.

## Context

Rules matched every block of the activity, app and source lanes. A short glance broke a suggestion: two
minutes in a Slack channel during a Zoom meeting went to the Slack rule (then dropped, under the 5-minute
minimum), and the Zoom rule could not bridge over time another rule had matched. The meeting came out as two
suggestions with a hole between them. A recording means "what I mostly did then", which is what the summary
lane shows.

## Decision

- **Rules match the summary.** `DayView.rule_blocks` (`day/rule_blocks.rs`) is the tracker summarized in
  buckets of `timeline.summary_minutes`, each bucket won by the activity with the most time in it. Rust
  builds it whether or not the summary lane is shown.
- **Keyed by document.** The rules' summary keys by app and window title whatever `titles` and
  `groupByApp` show, so title conditions see each window.
- **Worked time only.** Each block is cut to the first and last activity of any app in it (the winner takes
  the rest of its buckets), and sleep, lock and pause inside it are cut out, so buckets never add idle time
  to a suggestion.
- **External activity stays as is.** Imported entries (calendar events and the like) are not part of the
  summary; rules match them on their own.
- Suggestions, a rule's matched time today and the rule preview all read `rule_blocks`.

## Consequences

- Activity that never wins a bucket can't be suggested; it belongs to the recording around it.
- The bucket size moves suggestion edges: larger buckets absorb more switching.
- The visible summary lane may group by app while rules still see documents.
