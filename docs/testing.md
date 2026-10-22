# Testing and dev tooling

## Commands

Coding agents don't run these themselves. They hand test writing to the `write-ts-unit-test` and
`write-rust-unit-test` agents and runs to the `ci` agent (`.claude/agents/`), so test code and test output
stay out of the main context ([`.claude/rules/testing.md`](../.claude/rules/testing.md)).

| Command | Runs |
|---|---|
| `bun run test` | Vitest UI tests (`test:ui`), then the Rust tests (`test:rust`) |
| `bun run test:db` | `cargo test -- --ignored`: the integration tests on temp SQLite files (no Docker), including the performance test `tests/performance.rs` |
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
  file; slow ones are `#[ignore]` and run with `test:db`. `tests/performance.rs` seeds two years and fails
  with the measured medians when a view gets slower than its limit ([data.md](data.md#seed-fixture-and-performance)).
  Run it optimized with `cargo test --release --test performance -- --ignored --nocapture`.
- **Seed fixture:** `test_support/seed.rs` (`seed_years`, `seed_days`): a deterministic multi-year data set,
  also compiled into debug builds for `timetracker dev seed`.
- **Helpers** are in `src/test_support/` (`fixtures.rs`): `temp_config_dir`, `temp_db` (a migrated SQLite file in a temp
  dir) and the fixtures `activity`, `project` and `recording` ([data.md](data.md#tests)); later the fake capture source,
  compiled with `cfg(test)` or the `test-util` feature ([ADR 0015](adr/0015-test-doubles-behind-test-util.md)).
  The crate lists itself as a dev-dependency with `test-util`, so `tests/` sees them.
- **MCP server:** tests drive `TimeTrackerServer` over JSON-RPC in process: `tokio::io::duplex` split into
  halves, `server.serve((read, write))` spawned, a small client that writes one JSON line per request
  (`initialize`, `notifications/initialized`, `tools/list`, `tools/call`) and reads one reply line with a
  timeout. The config dir is a temp dir with a written `settings.json` and a seeded database file; the socket
  path points nowhere, so `status` and `open_day` answer `TimeTracker is not running`.
- **Generated types:** each `#[ts(export)]` type gets an `export_bindings_*` test that writes
  `src/api/types.gen.ts` (`bun run types`).
- **Contracts:** `identifier_matches_the_tauri_config` (`lib.rs`); the ADR 0013 test checks the macOS
  dependency graph for network crates; a settings test compares `ACCENTS` with
  `src/api/types.ts`.

### The tracker

Tracker tests build a `Tracker` on `Db::open_memory()` with `capture::fake::FakeSource` (granted, nothing
frontmost, no idle time) and drive it directly with explicit times: `tick(now)`, `on_event(event, now)`,
`pause(until, now)`, `resume(now)`, `recover(now)`. The test sets what the system shows with
`show(app, title)` / `set_snapshot(None)`, `set_idle(seconds)` and `set_permission(..)`, then checks the rows
(`activities::list_for_range`, `away::list_for_range`) and `status()`. The background loop and the macOS
source are not unit-tested; they are checked in the running app.

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
| `scripts/dev/screenshot-looks.sh <dir> [view]` | captures a view (default `day`) in all six Mode × Scheme combinations as `<scheme>-<mode>-<view>.png`, then restores the settings ([ux.md](ux.md#scheme-audit)) |
| `bun run app` | builds and `open`s the app; this takes focus |
| `bun run release` | builds the release `.app` and `.dmg` (`src-tauri/target/release/bundle/`), signed ad hoc |

The store is available as `window.timetracker` in the debug bundle: `bun run build:app` sets
`VITE_DEV_HOOKS=1` at build time, and `main.tsx` exposes the store only then. A release build has no
`window.timetracker`.

```sh
scripts/dev/js.sh 'timetracker.getState().openDialog({ type: "settings", page: "shortcuts" })'
scripts/dev/js.sh 'timetracker.getState().setView("week")'
```

### Accessibility in dev builds

The tracker needs Accessibility for window titles. macOS keys the grant to the app's code signature. An ad
hoc signature is a hash of the build, so every rebuild asks again: remove the old TimeTracker entry in
System Settings → Privacy & Security → Accessibility and grant the new one. `launch.sh` signs the bundle
before starting it, with the identifier `com.philippstracker.timetracker` and the keychain identity in
`TIMETRACKER_SIGN_IDENTITY`, or ad hoc without it. With a certificate (for example a self-signed
"Code Signing" certificate made in Keychain Access), the grant survives rebuilds.

### Sample data

`timetracker dev seed [--years N] [--yes]` (debug builds only, N from 1 to 10, default 2) asks the running
debug app to write N years of generated workdays up to yesterday into its database (`test_support::seed`,
seed 42) and prints the counts. It refuses when the database already holds activity before today, so a real
history never gets sample data mixed in; `--yes` seeds anyway. Run it against a dev data folder, never your
own: there is no undo short of restoring a backup.

```sh
timetracker dev seed --years 2
# seeded 522 workdays: 261000 activity spans, 1566 away spans, 3132 recordings over 5 projects
```

`launch.sh` sets `TIMETRACKER_WINDOW_FRAME` and `TIMETRACKER_BACKGROUND=1`. Rust reads both in `dev.rs`, a
module compiled into debug builds only. JavaScript eval over the socket is compiled out of release builds
as well. Never use AppleScript clicks or keystrokes to drive the app; they need focus.
