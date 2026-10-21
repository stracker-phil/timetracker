# 0009. External activity enters through CLI, socket and deep link; every activity carries a `source`

## Status

Accepted (2026-10-21), implemented.

## Context

The user runs other tools that know about the day (a planner that schedules breaks and meetings). Those
should show up on the timeline next to captured activity so the day can be recorded in one place. Letting
other apps write the SQLite file directly would bypass validation and the single-writer rule.

## Decision

Every `activities` row has a `source`: `tracker` for the capture module, `external:<name>` for anything
else. External activity enters only through the running app: the CLI (`timetracker activity add|start|stop`,
also JSON on stdin), the Unix socket the CLI uses (any process may speak the same line protocol) and the
`timetracker://activity?…` deep link. Rust validates (times, length, no overlap within one source) and
inserts. External spans are activity, not recordings: they are read-only, dimmed when covered, never in
reports. The Day view gives each external source its own lane.

## Consequences

- Other apps need no library: a shell call or a URL is enough.
- The tracker never sees external activity; its state machine ignores other sources.
- A source name is free text, so a typo makes a new lane; the Day view lists known sources for restoring.
