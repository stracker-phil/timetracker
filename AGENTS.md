# AGENTS.md

Instructions for a coding agent that clones, builds or adapts TimeTracker. Humans: see [README.md](README.md).

TimeTracker is one developer's personal tool, published as source. There are no releases, binaries or
support. As shipped it builds and runs on **macOS on Apple Silicon** only; everything else is a port.

## Repository facts

- Tauri 2 app: Rust backend in `src-tauri/`, React 19 + TypeScript frontend in `src/`, bun as package manager.
- Rust owns all logic; React only renders. Read [docs/architecture.md](docs/architecture.md) before changing
  code, and the page of the layer you touch ([docs/README.md](docs/README.md) is the index).
- [CLAUDE.md](CLAUDE.md) and [.claude/rules/](.claude/rules/) are the maintainer's conventions (tests,
  commits). Follow them in this repo; in a fork, keep what serves you.
- IPC types in `src/api/types.gen.ts` are generated from Rust with ts-rs (`bun run types`); never edit them.
- No network: the app has no HTTP client and its CSP allows IPC only ([ADR 0013](docs/adr/0013-no-network.md)).

## Prerequisites (macOS)

| Tool | Check | Notes |
|---|---|---|
| Xcode Command Line Tools | `xcode-select -p` | `xcode-select --install` |
| rustup, stable toolchain | `rustup show` | `rust-toolchain.toml` pins `stable` with rustfmt and clippy. A Homebrew rustup is keg-only: put `/opt/homebrew/opt/rustup/bin` on `PATH` |
| bun | `bun --version` | |

## Clone and build

```sh
git clone <this repository>
cd timetracker
bun install
bun run build:app        # debug bundle: src-tauri/target/debug/bundle/macos/TimeTracker.app
bun run release          # release bundle: src-tauri/target/release/bundle/macos/TimeTracker.app and a .dmg
```

The first Rust build takes several minutes. Optional links for the user: `scripts/install-cli.sh` (the
`timetracker` command in `~/.local/bin`) and `scripts/install-app.sh` (the app in `~/Applications`); both
link the release build, or the debug build with `--debug`.

## Verify

| Command | Expect |
|---|---|
| `bun run test` | Vitest and Rust unit tests pass |
| `bun run lint` | exits 0: types drift check, `tsc`, ESLint, `clippy -D warnings` |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check` | no differences |
| `bun run test:db` | the `#[ignore]` integration tests on temp SQLite files pass (no Docker needed) |

Try the app: `scripts/dev/launch.sh` starts the debug build next to the terminal without taking focus.
Its data and settings live in `~/Library/Application Support/com.philippstracker.timetracker/`.
[docs/testing.md](docs/testing.md) covers the suites and the scripts that drive the running app.

## Adapt

[docs/adapting.md](docs/adapting.md) lists every place that is specific to macOS or to the name, with the
replacement for Linux and Windows, and when a port counts as done.

Starting points for common requests:
- **Linux:** move the config dir to `dirs::config_dir()`, map `Mod` to Ctrl, drop the macOS window chrome and
  traffic-light padding, add `deb`/`appimage` targets, write a capture `Source` for X11 or Wayland. The Unix
  socket handoff works as is.
- **Windows 10/11:** as Linux, plus a named pipe instead of the Unix socket and `#[cfg(unix)]` around file
  permissions.
- **Rename the app:** the Identity table in docs/adapting.md (bundle id, binary, URL scheme, socket, paths).

When done, run the Verify table on the target platform and update the docs pages your change affects.
