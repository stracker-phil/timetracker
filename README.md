# TimeTracker

My personal automatic time tracker for macOS. It records which app and window I work in, in the background
and from login, and lets me turn that activity into recordings for projects by arranging blocks on a
timeline. Reports list a day or a range by project and by tag, ready to paste into a timesheet.

It is local only: no account, no server, no telemetry, no update check. No feature needs the network, and
it never captures keystrokes, screenshots, audio, the clipboard or page content.

**Status:** in development. It runs on macOS on Apple Silicon. Published as source, as-is: no releases, no
binaries, no support. To build it or port it, point a coding agent at [AGENTS.md](AGENTS.md).

## Build

```sh
bun install
bun run build:app        # debug bundle: src-tauri/target/debug/bundle/macos/TimeTracker.app
```

`scripts/install-app.sh` links the app into `~/Applications`, `scripts/install-cli.sh` links the
`timetracker` command into `~/.local/bin`.

## CLI

| Command | Does |
|---|---|
| `timetracker` | starts TimeTracker, or brings the running one to the front |
| `timetracker --hidden` | starts without opening the window (used for launch at login); a running instance stays as it is |
| `timetracker --log-level <error\|warn\|info\|debug\|trace>` | overrides the log level for this run |
| `timetracker --version`, `--help` | prints and exits |

Closing the window keeps TimeTracker running in the menu bar; quit from the menu bar item or with ⌘Q.

## Docs

Technical documentation for developers and coding agents: [docs/](docs/README.md).

## License

MIT, see [LICENSE](LICENSE).
