# Testing and dev tooling

## Commands

Coding agents don't run these themselves. They hand test writing to the `write-ts-unit-test` and
`write-rust-unit-test` agents and runs to the `ci` agent (`.claude/agents/`), so test code and test output
stay out of the main context ([`.claude/rules/testing.md`](../.claude/rules/testing.md)).

| Command | Runs |
|---|---|
| `bun run test` | Vitest UI tests (`test:ui`), then the Rust tests (`test:rust`) |
| `bun run test:db` | `cargo test -- --ignored`: the integration tests on temp SQLite files (no Docker) |
| `bun run lint` | regenerates the IPC types (`types`: the ts-rs `export_bindings` tests) and fails when `src/api/types.gen.ts` changed, then `tsc --noEmit` (app and tests), `eslint .` (`eslint.config.js`: `@eslint/js` and type-checked `typescript-eslint` recommended plus `react-hooks/rules-of-hooks` and `exhaustive-deps`; test files and `apiMock.ts` turn off the rules that fight mocks) and `cargo clippy --lib --bins -- -D warnings` (production code only) |
| `cargo fmt --manifest-path src-tauri/Cargo.toml` | Rust formatting (run before lint) |

The project rules for tests:
- Every change ships with tests of its observable behavior, and they run green before every commit.
- Tests pin behavior, not internals: command results, CLI parse results, stored files, rendered UI.
- No log assertions, no call counts, no private helpers.

## Agents

| Agent | Does |
|---|---|
| `write-rust-unit-test` | writes Rust unit tests (`mod tests`) and integration tests; never runs them |
| `write-ts-unit-test` | writes Vitest tests next to components, mocking only `src/api`; never runs them |
| `ci` | runs one suite (UI, Rust unit, integration, lint, format check, all unit tests) and replies with a pass/fail summary |
| `review-code-cleanliness` | reviews a changed file for structure, comment noise, boundaries and over-engineering |

The `ship-step` skill (`.claude/skills/ship-step/`) chains cleanup, review, `cargo fmt`, `ci`, build,
launch and commit at the end of every step.

## Rust

- **Unit tests** sit next to the code in `#[cfg(test)] mod tests`. Stores and sockets use `tempfile`
  directories; nothing touches the real config dir, the user's data or the network.
- **Database tests.** Repository tests run on `Db::open_memory()`; tests of opening, WAL and the data folder
  use `temp_db` or a `DataStore` on a temp dir. Calendar tests pass a fixed zone (`chrono-tz`) to
  `Day::bounds_in` / `Day::of_in`, never the machine's.
- **Integration tests** live in `src-tauri/tests/` and use the public `timetracker_lib` API on a temp SQLite
  file; slow ones are `#[ignore]` and run with `test:db` (none yet).
- **Helpers** are in `src/test_support.rs`: `temp_config_dir`, `temp_db` (a migrated SQLite file in a temp
  dir) and the fixtures `activity`, `project` and `recording` ([data.md](data.md#tests)); later the fake capture source,
  compiled with `cfg(test)` or the `test-util` feature ([ADR 0015](adr/0015-test-doubles-behind-test-util.md)).
  The crate lists itself as a dev-dependency with `test-util`, so `tests/` sees them.
- **Generated types:** each `#[ts(export)]` type gets an `export_bindings_*` test that writes
  `src/api/types.gen.ts` (`bun run types`).
- **Contracts:** `identifier_matches_the_tauri_config` (`lib.rs`); the ADR 0013 test checks the macOS
  dependency graph for network crates; a settings test compares `ACCENTS` and `SCHEMES` with
  `src/api/types.ts`.

## Frontend

The setup, mocking and fixtures are described in [frontend.md](frontend.md#tests). In short:
- Tests are `*.test.tsx` files next to their components.
- They mock the `src/api` module with `createApiMock()` (`src/test/apiMock.ts`).
- They use Testing Library queries by role or text and `userEvent`.

## Driving the app

The dev scripts work without stealing focus, so the app can run on a second screen while you or an agent
keep typing in the terminal. They are macOS-only (zsh, Swift, `screencapture`, and `python3` in `js.sh`).

| Script | Does |
|---|---|
| `bun run build:app` | builds the debug `.app` bundle (`src-tauri/target/debug/bundle/macos/TimeTracker.app`) |
| `scripts/dev/launch.sh [cli args]` | kills a running `timetracker` and starts the debug build next to the terminal window, without focus. The log goes to `$TMPDIR/timetracker-dev.log`. The terminal app is set by `TIMETRACKER_DEV_TERMINAL` (default `Terminal`) |
| `scripts/dev/js.sh '<js>'` | runs JavaScript in the running debug app over the IPC socket. Fire-and-forget: no return value, and top-level `const` declarations collide across calls, so wrap them in an IIFE |
| `scripts/dev/screenshot.sh out.png` | captures the TimeTracker window, also while it is in the background |
| `bun run app` | builds and `open`s the app; this takes focus |

The store is available as `window.timetracker` in the debug bundle: `bun run build:app` sets
`VITE_DEV_HOOKS=1` at build time, and `main.tsx` exposes the store only then. A release build has no
`window.timetracker`.

```sh
scripts/dev/js.sh 'timetracker.getState().openDialog({ type: "settings", page: "shortcuts" })'
scripts/dev/js.sh 'timetracker.getState().setView("week")'
```

`launch.sh` sets `TIMETRACKER_WINDOW_FRAME` and `TIMETRACKER_BACKGROUND=1`. Rust reads both in `dev.rs`, a
module compiled into debug builds only. JavaScript eval over the socket is compiled out of release builds
as well. Never use AppleScript clicks or keystrokes to drive the app; they need focus.
