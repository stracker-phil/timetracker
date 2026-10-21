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
- `src-tauri/src/`: `lib.rs` (startup), `cli/` (argv, replies, time and report words), `ipc.rs`
  (single-instance socket), `pending.rs`, `launch.rs`, `remote.rs` (requests the page answers), `deep_link.rs`,
  `events.rs` (every event name and payload), `state.rs` (`AppState`), `store/` (JSON files), `db/` (SQLite,
  repositories, backup, `relocate.rs`), `capture/` (`Source`, `macos.rs`), `tracker/`, `activity/` (external
  activity, deletion), `day/`, `calendar/` (week, month), `projects/`, `recordings/`, `timers/`, `rules/`,
  `reports/`, `away_prompt/`, `mcp/`, `autostart.rs`, `commands.rs`, `menu.rs`, `tray.rs`, `window.rs`,
  `error.rs`, `lock.rs`, `dev.rs` (debug builds), `test_support/` (fixtures, seed; tests and debug builds).
- Errors cross the boundary as `{ kind, message, code }` (`error.rs`). IPC types are generated with ts-rs.
- Timestamps are UTC milliseconds; days are local `YYYY-MM-DD`, computed in Rust.
- No network crate, no HTTP; the CSP allows IPC only.
- Frontend state: zustand store in `src/state/store.ts`; components in `src/components/`.

## Commands
Test, lint and format-check commands run through the `ci` agent, never in the main context.
- `bun run test`: Vitest and Rust unit tests
- `bun run test:db`: `#[ignore]` integration tests on temp SQLite files (no Docker), including
  `src-tauri/tests/performance.rs` (two seeded years, median limits per view)
- `bun run lint`: IPC types drift check (`src/api/types.gen.ts`), tsc, ESLint and clippy on production code (warnings are errors)
- `bun run release`: release `.app` and `.dmg` in `src-tauri/target/release/bundle/`, signed ad hoc
- `scripts/install-cli.sh` / `scripts/install-app.sh` `[--debug]`: symlink the `timetracker` command into
  `~/.local/bin` and the app into `~/Applications`; release build by default, `--debug` for the debug one
- `timetracker dev seed --years N` (debug builds): sample history in the running app's database
- `bun run build:app` + `scripts/dev/launch.sh`: debug build, restarted next to the terminal without stealing focus
- `scripts/dev/js.sh` / `scripts/dev/screenshot.sh`: drive (store: `window.timetracker`) and capture the running window in the background

## Workflow
Finish every step with the `ship-step` skill. Rules in `.claude/rules/`; `testing.md` says which agents in
`.claude/agents/` write and run tests.
