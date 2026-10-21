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
bun run release          # src-tauri/target/release/bundle/macos/TimeTracker.app and a .dmg
bun run build:app        # debug bundle: src-tauri/target/debug/bundle/macos/TimeTracker.app
```

`scripts/install-app.sh` links the release app into `~/Applications`, `scripts/install-cli.sh` links the
`timetracker` command into `~/.local/bin`; both take `--debug` to link the debug build instead.

- **Accessibility:** the grant is tied to the bundle's signature. A rebuilt debug app may ask again; the
  release build keeps it.
- **Launch at login:** the LaunchAgent points at the bundle path at the time it is enabled. After moving or
  installing the app, turn launch at login off and on again in Settings.

## CLI

| Command | Does |
|---|---|
| `timetracker` | starts TimeTracker, or brings the running one to the front |
| `timetracker --hidden` | starts without opening the window (used for launch at login); a running instance stays as it is |
| `timetracker --log-level <error\|warn\|info\|debug\|trace>` | overrides the log level for this run |
| `timetracker --version`, `--help` | prints and exits |
| `timetracker show [DAY]` | shows the window at a day (`YYYY-MM-DD`, `today`, `yesterday`), starting TimeTracker if needed |
| `timetracker focus`, `hide`, `quit` | brings the window to the front, hides it, quits |
| `timetracker status [--json]` | the tracker state, the current activity and whether the window shows |
| `timetracker pause <15m\|1h\|tomorrow\|off>`, `resume` | pauses tracking (`off`: until resumed), resumes it |
| `timetracker activity add\|start\|stop …` | writes another app's activity, see [Scripting](#scripting) |
| `timetracker activity delete <day>` / `activity delete <from> <to>` | deletes the activity and away spans of a day or range (whole when they cross it; recordings stay) |
| `timetracker activity undo` | restores the last delete, within 30 s of it |
| `timetracker report [RANGE] [--by project\|tag\|day\|week\|month] [--client C]… [--project P]… [--tag T]… [--round N [--up]]` | prints the hours and notes of today (or yesterday, week, last-week, month, last-month, a day, or two days), ready to paste into a timesheet; `--json` for the data |
| `timetracker start PROJECT [--note N] [--tag T]…` / `timetracker stop` | starts a timer now (a running one stops) / stops it |
| `timetracker record --project P --from T --to T [--note N] [--tag T]… [--replace]` | records time for a project (by id, name or a unique start of its name; tags by name); refuses to overlap another recording unless `--replace` |
| `timetracker mcp` | serves the read-only MCP server on stdin/stdout, see [MCP](#mcp) |

Every command except `show`, `focus` and `mcp` needs TimeTracker running and exits 2 with "TimeTracker is not
running" otherwise. Exit codes: 0 ok, 1 TimeTracker refused the request (the error is printed), 2 bad
arguments or not running. `--json` prints replies as JSON.

Closing the window keeps TimeTracker running in the menu bar; quit from the menu bar item or with ⌘Q.

## Scripting

Other apps can add their own activity, such as a planner's meetings and breaks. Each source gets its own
lane in the Day view, next to the tracker's; spans of one source never overlap. Times without an offset are
local.

```sh
timetracker activity add --source planner --title "Meeting: weekly sync" --app "Calendar" \
  --start 2026-10-21T10:00 --end 2026-10-21T10:45
timetracker activity start --source planner --title "Break"
timetracker activity stop  --source planner
printf '[{"source":"planner","title":"Focus block","start":%d,"end":%d}]' "$from" "$to" | timetracker activity add --json
open "timetracker://activity?source=planner&title=Lunch&start=2026-10-21T12:00&end=2026-10-21T12:45"
timetracker show yesterday
timetracker status --json | jq .tracker.state
```

`--json` on `activity add` reads an array of `{ source, title, start, end, app?, url?, path? }` from stdin,
with times in UTC milliseconds. Links also take `timetracker://show?day=…`, `timetracker://activity/start?…`
and `timetracker://activity/stop?source=…`; encode a `+` in a time offset as `%2B`. The protocol behind the CLI is in
[docs/backend.md](docs/backend.md#cli-and-single-instance-handoff).

## MCP

`timetracker mcp` lets Claude (or any MCP client) read your recordings, projects, tags and reports, also while
the app is closed. It only reads; nothing leaves the Mac except what the client sends to its model. Add it to
Claude Code (Settings → Data shows this line with the path of your binary, ready to copy; add `--scope user`
to use it in every project):

```sh
claude mcp add timetracker -- ~/.local/bin/timetracker mcp
```

Then ask "what did I record yesterday by project?" or "how many hours of development this month?". The tools:
`report` (the report text, or JSON), `recordings`, `projects`, `tags`, `day_summary`, `status` and `open_day`
(these two need the app running), and `activity` (window titles and URLs), which stays off until you enable
"Expose activity to the MCP server" in Settings → Data.

## Docs

Technical documentation for developers and coding agents: [docs/](docs/README.md).

## License

MIT, see [LICENSE](LICENSE).
