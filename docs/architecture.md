# Architecture

TimeTracker is a local-first automatic time tracker for macOS: one person, one computer, no network. It is a
single-window Tauri 2 app. A Rust process owns every piece of logic: capture, the tracker, persistence,
timeline layout, overlap checks, report math, rules and the CLI. A React 19 webview renders state and
forwards intent. The app keeps running in the menu bar while its window is closed. The layer pages go into
detail: [backend](backend.md), [frontend](frontend.md), [design language](ux.md).

## Three layers of data

| Layer | What it is | Who writes it |
|---|---|---|
| **Activity** | one continuous span in one app and window, or an away span (no input, sleep, lock) | the background tracker, or an external app through the CLI |
| **Recording** | a block of time the user confirms for a project; what reports show | the user (Day view, timers, CLI, away prompt) |
| **Project** | a named bucket for recordings, optionally under a client | the user |

Activity never becomes a recording on its own. Rules only suggest recordings; tags classify them for
reports; a timer is a recording whose end is "now" ([ADR 0005](adr/0005-activity-and-recordings-are-separate-layers.md)).

## Design rules

These rules hold everywhere in the code. Keep them when you change things.

1. **Rust owns the logic; React only renders** ([ADR 0001](adr/0001-rust-owns-the-logic.md)). Timeline
   layout, grouping, snapping, overlap checks, report math, rule matching and report text are Rust. The page
   asks, renders and forwards intent. It does no date math beyond formatting.
2. **IPC only in `api.ts` files** ([ADR 0002](adr/0002-ipc-in-api-files.md)). Only `src/api/index.ts` (and
   `src/<module>/api.ts` for an optional module) imports `@tauri-apps/api` or a `@tauri-apps/plugin-*`
   package. Everything else imports the `api` object, which is also what UI tests mock. A boundary test
   (`src/test/boundary.test.ts`) enforces it.
3. **Errors are data** ([ADR 0003](adr/0003-errors-as-kinds.md)). Every command returns
   `Result<T, AppError>`, serialized as `{ kind, message, code, field?, details? }` (`ErrorJson` in
   `error.rs`). The UI branches on `kind` only.
4. **Types are generated** ([ADR 0004](adr/0004-generated-ipc-types.md)). Every type crossing IPC derives
   `ts_rs::TS` and exports to `src/api/types.gen.ts` (camelCase). `bun run types` regenerates it;
   `bun run lint` fails on drift. Never edit the generated file.
5. **No silent writes to recordings** ([ADR 0010](adr/0010-recordings-never-overlap-ops-with-undo.md)).
   Every change to recordings is an operation with an inverse, applied in Rust and undoable. Activity is
   never modified; deleting it is an explicit user action with an undo window.
6. **No network** ([ADR 0013](adr/0013-no-network.md)). No HTTP client, no network crate, no update check.
   The CSP allows IPC only.
7. **Panics unwind** ([ADR 0014](adr/0014-panic-unwind.md)). A bug fails one command, not the app. Locks
   are taken with the poison-tolerant helpers in `lock.rs`.
8. **No color literal outside `src/styles/schemes/`** ([ADR 0016](adr/0016-color-schemes.md)). Schemes
   are complete token sets; a Vitest scan enforces it.

Time is UTC milliseconds (`i64`) in storage and IPC; a day is a local calendar day `YYYY-MM-DD`, resolved
to bounds in Rust ([ADR 0011](adr/0011-utc-milliseconds-local-days.md)).

## Process model

```
┌──────────── TimeTracker.app ────────────────────────────────────────────────┐
│  Webview (React, hidden when closed)      Rust core (tokio runtime)         │
│  ┌──────────────────────┐   invoke   ┌────────────────────────────────┐    │
│  │ components/          │ ─────────▶ │ #[tauri::command]s             │    │
│  │ state/store.ts       │            │ commands.rs, feature modules   │    │
│  │ commands/ (registry) │ ◀───────── │                                │    │
│  │ api/index.ts         │   events   ├────────────────────────────────┤    │
│  └──────────────────────┘            │ tracker/ ◀── capture/          │    │
│  Tray icon (menu bar)  ◀────────────▶│ AppState: stores, Db           │    │
│                                      │ store/ (JSON files)            │    │
└──────────────────────────────────────┴──────┬────────────┬──────────────────┘
                     Unix socket (ipc.rs)     │            │ sqlx
  `timetracker …` (2nd process, CLI) ─────────┘            ▼
                                                    data/timetracker.sqlite
```

- **One instance.** The first process binds `timetracker.sock` in the config dir and holds
  `timetracker.lock`. A second `timetracker …` sends its request over the socket and exits
  ([backend: single instance](backend.md#cli-and-single-instance-handoff),
  [ADR 0017](adr/0017-cli-asks-the-running-app.md)).
- **Window.** Closing the window hides it; the app keeps running and keeps its Dock icon. Quit only from the
  tray menu, ⌘Q, or (later) `timetracker quit` ([backend: window lifecycle](backend.md#window-lifecycle),
  [ADR 0007](adr/0007-tracker-in-process-menu-bar-app.md)).
- **Tray.** A template-image menu bar item with a status line, Open TimeTracker and Quit TimeTracker.
- **Launch at login** through `tauri-plugin-autostart` (a LaunchAgent) with `--hidden`, so the app starts
  without showing the window.
- **Launch requests are pulled, not pushed.** Rust queues launch events and emits a bare `launch` event;
  the page calls `take_launch_events`, so it can't miss the request that started the app.
- **The native menu and the tray emit `command` events** carrying a command id. The page runs that id
  through the same registry that handles shortcuts and the palette.
- **Data changes** are announced with `data_changed { kind, day }`; views showing that day reload.

## Repository map

The target shape. Each step of the implementation plan creates the folders it needs; the foundation
(config dir, errors, JSON stores, logging, single instance, CLI, window lifecycle, tray, frontend shell)
exists, the domain modules (`db/`, `capture/`, `tracker/` and everything after them) come with later steps.

```
src-tauri/
  Cargo.toml                 crate `timetracker`, lib `timetracker_lib`, edition 2024
  Info.plist                 merged into the bundle's Info.plist (usage descriptions)
  migrations/NNNN_*.sql      sqlx migrations
  src/lib.rs                 startup: config dir, CLI parse, socket handoff, lock, AppState, Tauri builder
  src/main.rs
  src/cli.rs ipc.rs remote.rs pending.rs launch.rs   CLI parsing, socket, requests the page answers, launch queue
  src/events.rs              every event name and payload the backend emits
  src/state.rs               AppState
  src/error.rs lock.rs       AppError (+ ErrorKind), poison-tolerant lock helpers
  src/store/                 JsonFile, JsonStore, settings.rs, keybindings.rs, ui_state.rs
  src/commands.rs            settings, keybindings, ui state, misc; feature commands sit in their module
  src/tray.rs menu.rs window.rs deep_link.rs   tray, native menu, window helpers, timetracker:// links
  src/dev.rs                 debug builds only: window frame, background launch, eval
  src/test_support.rs        test helpers behind cfg(test) / feature test-util
  src/db/                    Db (sqlx pool), migrations, time helpers, repositories
  src/capture/               Source trait, Snapshot types, macos.rs (objc2), fake.rs (test-util)
  src/tracker/               state machine: spans, away, pause, status; the tokio task
  src/activity/ day/ week.rs month.rs projects/ tags.rs recordings/ rules/ reports/ timers.rs away_prompt.rs mcp/
  tests/                     integration tests on a temp SQLite file
src/
  main.tsx modules.ts App.tsx
  api/index.ts types.gen.ts types.ts
  state/store.ts requests.ts slices/
  commands/                  registry, scopes, index (APP_COMMANDS)
  components/                shell: Toolbar, Sidebar, StatusBar, Modal, CommandPalette, Dialogs, settings/
  registry/settingsPages.ts
  lib/ styles/ test/ assets/fonts/
docs/                        this folder; adr/ holds the decision records
scripts/dev/ scripts/install-*.sh
.claude/                     rules, agents, skills, settings
```

## Platform

The code base targets **macOS on Apple Silicon** only. The platform-specific parts are listed in
[adapting.md](adapting.md#platform-specific-code): the config path, Unix sockets, window vibrancy, the tray,
the LaunchAgent and the dev scripts; later the capture module (objc2, Accessibility, Apple Events).
