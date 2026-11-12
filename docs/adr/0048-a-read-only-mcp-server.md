# 0048. A read-only MCP server, with writes as suggestions

## Status

Accepted (2026-11-12), implemented since 2026-10. Inherited from the family catalog (mcp-server; reference:
DataBee ADR 0025). Records what `src-tauri/src/mcp/` already does; the catalog took its shape from it.

## Context

Claude and other MCP clients should answer questions about the owner's time ("what did I record for this
project last week") and suggest time, with the same answers as the app. The server existed without a record;
the family catalog makes an MCP server an optional capability that each app adopts on the owner's choice.

## Decision

- **`timetracker mcp`**: an MCP server over stdio, a subcommand of the same binary, built on `rmcp`. It never
  starts Tauri; stdout carries only JSON-RPC.
- **Reads work with the app closed**: the read tools open the database read-only through the headless core
  (`mcp/data/`).
- **Writes go through the running app** over the socket, like the CLI, and land as suggestions with a source
  the owner reviews in the Day view ([0028](0028-outside-writes-are-suggestions.md),
  [0041](0041-programmatic-timers-start-as-suggestions.md)); `final` only when the owner asked for it.
- stdio is not the network, so [0013](0013-no-network.md) holds.
- Rejected: a write-capable server that changes recordings directly (the owner would not see the change); a
  local HTTP API for the same tools (a listener any local process and browser can reach).

## Consequences

- Installing it is one line: `claude mcp add timetracker -- ~/.local/bin/timetracker mcp`.
- `docs/backend.md` → MCP server documents the tools; `docs/adapting.md` → Dropping the MCP server says how to
  remove it.
