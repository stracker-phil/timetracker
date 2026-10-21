# TimeTracker

Local-first automatic time tracker for macOS: one person, one computer, no network. Activity is captured in
the background; the user turns it into recordings for projects on a Day view timeline; reports are text
built for copying. See README.md for the product and CLI.

## Rules

- Every session or step ends in a commit, so it can be reverted.
- Never push. Only commit locally.
- Write unit tests for every change and run them (through the `ci` agent) before every commit; tests and
  lint must be green.
- Tests pin behavior, not internals: public interfaces only, no log assertions, no call counts.
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
- Tauri 2 + React 19 + TypeScript in `src/`, bun as package manager, zustand, lucide-react
- macOS 27, Apple Silicon only
- Blueprint: `/Users/philipp/Coding/Personal/databee` (same author, stack and conventions)

## Architecture
- Rust owns all logic; React only renders. Only `api.ts` files call `invoke`/`listen` (`src/api/index.ts`
  for the core). Tests mock them.
- `src-tauri/src/`: `lib.rs` (startup), `cli.rs` (argv), `ipc.rs` (single-instance socket), `launch.rs`,
  `remote.rs`, `events.rs` (every event name and payload), `store/` (JSON files), `commands.rs`, `menu.rs`,
  `tray.rs`, `window.rs`, `error.rs`, `lock.rs`; later `db/`, `capture/`, `tracker/` and the feature modules.
- Errors cross the boundary as `{ kind, message, code }` (`error.rs`). IPC types are generated with ts-rs.
- Timestamps are UTC milliseconds; days are local `YYYY-MM-DD`, computed in Rust.
- No network crate, no HTTP; the CSP allows IPC only.
- Frontend state: zustand store in `src/state/store.ts`; components in `src/components/`.

## Commands
Test, lint and format-check commands run through the `ci` agent, never in the main context.
- `bun run test`: Vitest and Rust unit tests
- `bun run test:db`: `#[ignore]` integration tests on temp SQLite files (no Docker)
- `bun run lint`: IPC types drift check (`src/api/types.gen.ts`), tsc, ESLint and clippy on production code (warnings are errors)
- `scripts/install-cli.sh` / `scripts/install-app.sh`: symlink the `timetracker` command and the app
- `bun run build:app` + `scripts/dev/launch.sh`: debug build, restarted next to the terminal without stealing focus
- `scripts/dev/js.sh` / `scripts/dev/screenshot.sh`: drive (store: `window.timetracker`) and capture the running window in the background

## Workflow
Finish every step with the `ship-step` skill. Rules in `.claude/rules/`; `testing.md` says which agents in
`.claude/agents/` write and run tests.
