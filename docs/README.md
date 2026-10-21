# TimeTracker technical documentation

These docs are for developers and coding agents working on the code. For what the app does and how to
use it, see the [project README](../README.md).

| Page | Covers |
|---|---|
| [architecture.md](architecture.md) | Design rules, process model, repository map |
| [backend.md](backend.md) | Rust core: startup, `AppState`, Tauri commands, events, CLI and single-instance IPC, window lifecycle, tray, persistence, errors, native menu, logging, content security policy |
| [capture.md](capture.md) | Capture source (macOS APIs, fake), tracker state machine, pause, status text, permission onboarding |
| [data.md](data.md) | SQLite data store: data folder, schema and data rules, time helpers, repositories, backup, migrations |
| [frontend.md](frontend.md) | React app: boot, store, API boundary, command registry, components, styling, tests |
| [ux.md](ux.md) | Design language: principles, surfaces, color schemes and tokens, type, inputs, overlays, keys |
| [testing.md](testing.md) | Unit and integration tests, the test agents, scripts that drive the app in the background |
| [adapting.md](adapting.md) | Identity, platform-specific code, common changes, conventions to keep |
| [adr/](adr/README.md) | Architecture decision records: why the design is the way it is |

Suggested reading order: architecture, then the layer you are changing, then adapting.

The rules in [`.claude/rules/`](../.claude/rules/) are binding for changes:
- [`testing.md`](../.claude/rules/testing.md)
- [`commits.md`](../.claude/rules/commits.md)
- [`rust-test-conventions.md`](../.claude/rules/rust-test-conventions.md)
- [`ts-test-conventions.md`](../.claude/rules/ts-test-conventions.md)
