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
| **Recording** | time the user confirms for a project, in one or more spans ([ADR 0020](adr/0020-recordings-have-spans.md)); what reports show | the user (Day view, timers, CLI, away prompt) |
| **Project** | a named bucket for recordings, optionally under a client | the user |

Activity never becomes a recording on its own. Rules only suggest recordings; tags classify them for
reports; a timer is a recording whose last span ends "now" ([ADR 0005](adr/0005-activity-and-recordings-are-separate-layers.md)).

## Design rules

These rules hold everywhere in the code. Keep them when you change things.

1. **Rust owns the logic; React only renders** ([ADR 0001](adr/0001-rust-owns-the-logic.md)). Timeline
   layout, grouping, snapping, overlap checks, report math, rule matching and report text are Rust. The page
   asks, renders and forwards intent. Its only date math is formatting and stepping calendar days
   (`lib/day.ts`).
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
8. **No color literal outside `src/styles/palettes/`** ([ADR 0016](adr/0016-color-schemes.md),
   [ADR 0021](adr/0021-appearance-modes-and-schemes.md), [ADR 0022](adr/0022-a-scheme-per-system-mode.md)). Palettes are complete token sets; a Vitest scan
   enforces it.

Time is UTC milliseconds (`i64`) in storage and IPC; a day is a local calendar day `YYYY-MM-DD`, resolved
to bounds in Rust ([ADR 0011](adr/0011-utc-milliseconds-local-days.md)).

## A request, end to end

The Day view showing a day, the main path through every layer:

1. `DayPage` (`src/components/day/`) asks the `day` slice to `loadDay()` when the day, the day's data
   version or today's live tick changes.
2. The slice calls `api.dayView(day)`, which invokes the Tauri command `day_view` (`src/api/index.ts` is the
   only place that calls `invoke`).
3. `day::commands::day_view` parses the day, reads the activity and away rows that share time with it
   (`db::activities::list_for_range`, `db::away::list_for_range`, one indexed query each) and the settings.
4. `day::view` clips the rows, computes totals from them, then applies the timeline settings in a
   fixed order: `transform::without_hidden`, `transform::identified` (titles, grouping), `summary::summary`,
   `transform::grouped` (continuity, noise), `lanes::lanes` (lanes and columns), `mark_covered`. A pure
   function, tested without a database ([ADR 0012](adr/0012-timeline-layout-in-rust.md)).
5. The `DayView` crosses IPC as generated types; `Timeline` positions every block from `start`, `end`, `col`
   and `cols` and computes nothing else.
6. A change to the day's data (`data_changed`) bumps its version in the `data` slice, and the page asks again.

## A write, end to end

Recording an hour of activity:

1. The user drags over the lanes or the time column (`useSelectionPress`); the outline shows the span Rust snaps
   (`api.recordingSnap`, at most once per frame). On release the `editor` slice asks Rust for the snapped span
   once more and opens `RecordingEditor` inline in the timesheet; `recording_proposal` fills in the activity
   the span covers and a project.
2. Save calls the `recordings` slice's `createRecording`, which invokes `recording_create` with `overlap:
   cancel`. An `overlap` error asks inside the editor (`OverlapNotice`); Replace sends the same input with
   `replace`.
3. `recordings::Recordings::create` validates and snaps, builds an `Op` (a `Batch` when it replaces) and
   applies it in one transaction that also proves no recordings overlap, then pushes it on the undo stack.
4. `data_changed { kind: recordings, day }` goes out for every day the op touched; the Day view reloads them,
   and the toast offers Undo, which applies the op's inverse.

## A report, end to end

1. The Reports screen (or `timetracker report`) sends a `ReportFilter`: a range preset or days, client,
   project and tag ids (the CLI sends names, which the app resolves), the grouping and the rounding.
2. `reports::run` resolves the range to days, reads the recordings sharing time with it and builds the
   result: rows cut to the range and rounded one by one, grouped with their distinct notes, totals and
   counts, the donuts' slices (by client, project and tag), the client/project table and each project's
   notes as the text "Copy notes" copies.
3. `reports::text` renders the plain text the screen copies and the CLI prints; the screen draws the same
   `ReportResult` itself and copies a project's notes as given. A `data_changed` event runs the report again.

## A suggestion, end to end

1. `day::load` builds the day's view, then `rules::day_suggestions` runs the active rules over its
   blocks: first matching rule per block, runs across short gaps, recorded and dismissed time cut away, each
   placed among the timesheet's cards (`nextCard`).
2. The Day view shows each `Suggestion` as a ghost card in the timesheet and hatched in the time column; it
   is not a recording until the owner approves it. ✓ (or ↵) sends `suggestion_approve`; Rust rebuilds the
   suggestions, finds the id and creates the recording through the recordings service (undoable). Edit opens
   the inline editor prefilled; its Approve sends `suggestion_approve_edited`. ✕ (or ⌫) sends
   `suggestions_dismiss`, which stores the span in `dismissed_suggestions` as an undoable `Op::Dismiss`.
   Approve all (`suggestions_approve`) and Dismiss all send the shown ids.
3. `data_changed` reloads the day; the approved time is now recorded and the dismissed time is cut away, so
   the suggestion is gone.

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
  [ADR 0017](adr/0017-cli-asks-the-running-app.md)). Every subcommand is a message the running app answers;
  other apps (a planner) add activity this way or through `timetracker://` links, each in its own lane
  ([ADR 0009](adr/0009-external-activity-with-a-source.md), [data: external activity](data.md#external-activity)).
- **Window.** Closing the window hides it; the app keeps running and keeps its Dock icon. Quit only from the
  tray menu, ⌘Q, or `timetracker quit` ([backend: window lifecycle](backend.md#window-lifecycle),
  [ADR 0007](adr/0007-tracker-in-process-menu-bar-app.md)).
- **Tray.** A template-image menu bar item with the tracker status line, Pause Tracking (or Resume Tracking),
  Start Timer… (or Stop Timer), Recent Timers, Open TimeTracker and Quit TimeTracker
  ([backend: tray](backend.md#tray)).
- **Launch at login** through `tauri-plugin-autostart` (a LaunchAgent) with `--hidden`, so the app starts
  without showing the window.
- **Launch requests are pulled, not pushed.** Rust queues launch events and emits a bare `launch` event;
  the page calls `take_launch_events`, so it can't miss the request that started the app.
- **The native menu and the tray emit `command` events** carrying a command id. The page runs that id
  through the same registry that handles shortcuts and the palette. The page reports back which menu items
  can run and which are checked (`menu_sync`), so Rust greys them out.
- **Data changes** are announced with `data_changed { kind, day }`; views showing that day reload.

## Repository map

The shipped shape.

```
src-tauri/
  Cargo.toml                 crate `timetracker`, lib `timetracker_lib`, edition 2024
  Info.plist                 merged into the bundle's Info.plist (usage descriptions)
  migrations/NNNN_*.sql      sqlx migrations
  src/lib.rs                 startup: config dir, CLI parse, AppState, then app::start
  src/instance.rs            socket handoff to the running app, instance lock, subcommand replies
  src/app.rs handlers.rs     Tauri builder (plugins, setup, run loop); every registered command
  src/main.rs
  src/cli/                   CLI: command tree (mod.rs), time words (time.rs), report words (report.rs), human replies (reply.rs)
  src/ipc.rs remote.rs pending.rs launch.rs   socket, dispatch and requests the page answers, launch queue
  src/events.rs              every event name and payload the backend emits
  src/state.rs               AppState
  src/error.rs lock.rs       AppError (+ ErrorKind), poison-tolerant lock helpers
  src/store/                 JsonFile, JsonStore, settings/, keybindings.rs, ui_state.rs
  src/commands.rs            settings, keybindings, ui state, misc; feature commands sit in their module
  src/tray.rs window.rs deep_link.rs   tray, window helpers, timetracker:// links
  src/menu/                  native menu: items.rs (MENU_ITEMS), mod.rs (build, forward), sync.rs (enabled and checked items)
  src/help.rs                Help menu targets outside the app (the README)
  src/dev.rs                 debug builds only: window frame, background launch, eval, dev seed
  src/test_support/          test helpers behind cfg(test) / feature test-util; seed.rs also in debug builds
  src/db/                    Db (sqlx pool), migrations, time helpers, repositories, backup, relocate.rs (moving the data folder)
  src/capture/               Source trait, Snapshot types, macos.rs (objc2), fake.rs (test-util)
  src/app_icons.rs           installed apps' icons as cached PNGs, served as data URLs
  src/tracker/               state machine: spans, away, pause, status; the tokio task
  src/activity/              external activity (rules, commands), deletion with undo (delete.rs)
  src/autostart.rs           launch at login
  src/day/ calendar/ projects/ recordings/ rules/ reports/ timers/ away_prompt/ mcp/
  tests/                     integration tests on a temp SQLite file (performance.rs)
src/
  main.tsx modules.ts App.tsx
  api/index.ts types.gen.ts types.ts
  state/store.ts requests.ts slices/
  commands/                  registry, scopes, index (APP_COMMANDS)
  components/                shell: Toolbar, Sidebar, StatusBar, Modal, CommandPalette, Dialogs, settings/
  registry/settingsPages.ts
  lib/ styles/ test/
docs/                        this folder; adr/ holds the decision records
scripts/dev/ scripts/install-*.sh   dev loop; links for the app and the command (release, or --debug)
.claude/                     rules, agents, skills, settings
```

## Platform

The code base targets **macOS on Apple Silicon** only. The platform-specific parts are listed in
[adapting.md](adapting.md#platform-specific-code): the config path, Unix sockets, window vibrancy, the tray,
the LaunchAgent, the global shortcut, the dev scripts and the capture module (objc2, Accessibility, Apple
Events).
