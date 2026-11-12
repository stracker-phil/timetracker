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
| [ux.md](ux.md) | Design language: principles, surfaces, appearance and tokens, type, components, inputs, overlays, keys |
| [testing.md](testing.md) | Unit and integration tests, the test agents, scripts that drive the app in the background |
| [adapting.md](adapting.md) | Identity, platform-specific code, common changes, conventions to keep |
| [adr/](adr/README.md) | Architecture decision records: why the design is the way it is |

Suggested reading order: architecture, then the layer you are changing, then adapting.

## Sibling apps

TimeTracker is one of five apps by the same owner, with the same stack and process. Their shared decisions
live in one family catalog (the owner's `app-blueprint` skill); each app's ADRs are copies adapted to it. When
this repo has no answer yet, look there (read-only, never write to them):

| App | Path | Take from it |
|---|---|---|
| Taskboard | `../taskboard` | the newest decisions: the debug build on demo data, signing and the hardened runtime, the Settings window, motion, stable controls, the command bus, in-page menus, TanStack Query reads |
| Daybook | `../daybook` | the look (`docs/ux.md`, `src/styles/`), the Settings window's layout, scrollbars, dev scripts |
| DataBee | `../databee` | optional features as inline modules, ports with fakes, the read-only MCP server |
| Launcher | `../project-launcher` | the Settings search over every row, Settings → Shortcuts (Reassign, Reset), the About and Status pages |

TimeTracker is ahead in SQLite through sqlx (migrations, undo as ops), the seeds and the showcase, the CLI
over the socket with `watch`, and the performance tests; keep those when porting.

Demo data only: nothing in the repo carries the owner's real data ([`demo-data.md`](../.claude/rules/demo-data.md)).

The rules in [`.claude/rules/`](../.claude/rules/) are binding for changes:
- [`demo-data.md`](../.claude/rules/demo-data.md): nothing in the repo carries real data; the debug build
  runs on the showcase ([ADR 0031](adr/0031-the-debug-build-runs-on-demo-data.md))
- [`testing.md`](../.claude/rules/testing.md)
- [`commits.md`](../.claude/rules/commits.md)
- [`rust-test-conventions.md`](../.claude/rules/rust-test-conventions.md)
- [`ts-test-conventions.md`](../.claude/rules/ts-test-conventions.md)
