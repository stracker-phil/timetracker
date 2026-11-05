# TimeTracker

Local-first automatic time tracker for macOS: one person, one computer, no network. Activity is captured in
the background; the user turns it into recordings for projects on a Day view timeline; reports are text
built for copying. See README.md for the product and CLI.

The app looks native: it reads as a macOS app beside Calendar and System Settings, with the system font,
inset groups, a unified toolbar and AppKit control metrics (ADR 0024, `docs/ux.md`). No editor color schemes,
monospace text outside machine text, outlined cards or badges. Taskboard (`../taskboard`) and Daybook
(`../daybook`), the author's sibling apps, are the references for the look, the process and newer decisions
(`docs/README.md` → Sibling apps).

## Rules

- Every session or step ends in a commit, so it can be reverted. Commit each finished part of a step as soon as
  it is green, not only at the end.
- Never push. Only commit locally.
- Write unit tests for every change and run them (through the `ci` agent) before every commit; tests and
  lint must be green.
- Tests pin behavior, not internals: public interfaces only, no log assertions, no call counts.
- Demo data only: code, docs, tests, commits and screenshots never contain the owner's real data
  (`.claude/rules/demo-data.md`). The debug build runs on its own data folder, made-up capture and the showcase.
- Refactor continuously: when a file grows large (over ~400 lines of production code) or complex, simplify it
  and split it into focused modules in the same step, without asking.
- Keep modules small, isolated and replaceable: by feature, pure cores, a trait only where a second
  implementation exists today (`Source`, `IconSource`). No speculative abstractions.
- UI changes keep controls stable (`.claude/rules/stable-controls.md`).
- The implementation plan lives in `docs/plan/` (gitignored): `README.md` plus one file per step. Work one
  step per session, in its order; when a step is done, set its status from PENDING to DONE in
  `docs/plan/README.md` (the only change a step makes there).

## Docs
Technical docs live in `docs/` (index: `docs/README.md`). Read `docs/architecture.md` and the page of the
layer you change before changing it; `docs/adapting.md` lists everything macOS- or name-specific. Decisions
are in `docs/adr/` (never edited once accepted; a new record supersedes). Update the affected docs page in the
same commit when behavior, commands, files or extension points change. Document by file and symbol, never by
line number.

## Stack
- Rust backend in `src-tauri/` (rustup stable; Homebrew rustup is keg-only: `/opt/homebrew/opt/rustup/bin`)
- Tauri 2 + React 19 + TypeScript in `src/`, bun as package manager, zustand, TanStack Query, lucide-react
- macOS 27, Apple Silicon only
- Sibling apps (read-only): Taskboard `../taskboard`, Daybook `../daybook`, DataBee `../databee` (same author,
  stack and conventions; `docs/README.md` → Sibling apps)

## Architecture
- Rust owns all logic; React only renders. Only `api.ts` files call `invoke`/`listen` (`src/api/index.ts`
  for the core). Tests mock them.
- `src-tauri/src/`: `lib.rs` (startup), `instance.rs` (handoff to the running app), `app.rs` (Tauri builder),
  `handlers.rs` (command registration), `cli/` (argv, replies, time and report words), `ipc/`
  (single-instance socket), `pending.rs`, `launch.rs`, `remote.rs` (requests the page answers), `deep_link.rs`,
  `events.rs` (every event name and payload), `state.rs` (`AppState`), `store/` (JSON files), `db/` (SQLite,
  repositories, backup, `relocate.rs`), `capture/` (`Source`, `macos.rs`), `tracker/`, `activity/` (external
  activity, deletion), `day/`, `calendar/` (week, month), `projects/`, `recordings/`, `timers/`, `rules/`,
  `reports/`, `away_prompt/`, `mcp/`, `app_icons.rs`, `autostart.rs`, `commands.rs`, `bus/` (command bus: app commands by id, `page:<id>`), `menu/` (menu bar from `MENUS`, each window's enabled state, palette list), `help.rs`, `tray.rs`, `window/` (helpers, navigation guard),
  `error.rs`, `lock.rs`, `dev.rs` (debug builds), `test_support/` (fixtures, seed; tests and debug builds).
- Errors cross the boundary as `{ kind, message, code }` (`error.rs`). IPC types are generated with ts-rs.
- Timestamps are UTC milliseconds; days are local `YYYY-MM-DD`, computed in Rust.
- No network crate, no HTTP; the CSP allows IPC only.
- Frontend state: reads from Rust go through TanStack Query (`src/queries/`, invalidated by Rust's events; ADR
  0040); UI state and writes in the zustand store (`src/state/store.ts`); components in `src/components/`.

## Commands
Test, lint and format-check commands run through the `ci` agent, never in the main context.
- `bun run test`: Vitest and Rust unit tests
- `bun run test:db`: `#[ignore]` integration tests on temp SQLite files (no Docker), including
  `src-tauri/tests/performance.rs` (two seeded years, median limits per view)
- `bun run lint`: IPC types drift check (`src/api/types.gen.ts`), tsc, ESLint and clippy on production code (warnings are errors)
- `bun run release major|minor|patch`: bumps the version (ADR 0043), release `.app` and `.dmg` in `src-tauri/target/release/bundle/`, then signed (Apple Development certificate first, hardened runtime), installed, committed as "Release X.Y.Z" and tagged `vX.Y.Z`
- `scripts/install.sh` (the end of `bun run release`): signs the release build (`scripts/sign-app.sh`), links it
  as the app and the `timetracker` command (`--replace-cli` replaces a foreign one) and restarts a running TimeTracker on it; debug builds are never
  linked. `scripts/create-signing-cert.sh` (once per Mac) makes the fallback certificate for a Mac without an Apple Development one
- `timetracker dev seed --years N` (debug builds): sample history in the running app's database; `--showcase`
  writes the made-up history `scripts/dev/screenshot-readme.sh` captures for the README
- `bun run build:app` + `scripts/dev/launch.sh`: debug build, restarted next to the terminal without stealing focus
- `scripts/dev/js.sh` / `scripts/dev/screenshot.sh`: drive (store: `window.timetracker`) and capture the running window in the background

## Workflow
Finish every step with the `ship-step` skill. Rules in `.claude/rules/`; `testing.md` says which agents in
`.claude/agents/` write and run tests.
