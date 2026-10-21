# 0017. The CLI is a second process that asks the running app over the socket

## Status

Accepted (2026-10-21), implemented.

## Context

The user drives the app from scripts and other tools: show a day, start a timer, record a span, print a
report, send activity. The app holds the SQLite writer, the tracker and the undo stack; a CLI that opened the
database itself would race the app and bypass the overlap rule and undo.

## Decision

`timetracker <subcommand>` parses with clap into an `IpcMessage`, connects to `timetracker.sock`, sends one
JSON line, prints the reply and exits (DataBee's `ipc.rs`/`remote.rs` pattern, including the peer-uid check).
The running app answers every request through the same core functions the UI uses (`remote::handle` maps
each message to one module function), so CLI writes are ops with undo like UI writes. `show`, `focus` and a
bare `timetracker` start the app when none runs; every other subcommand exits 2 with "TimeTracker is not
running". `--json` prints the reply as JSON for scripts. Only `timetracker mcp` opens the database itself,
read-only, so Claude can query history while the app is closed.

## Consequences

- One implementation of every action; the CLI is a thin table of mappings.
- The CLI needs the app running for anything that writes.
- Any process that speaks the line protocol (the planner) can integrate without the CLI binary.
