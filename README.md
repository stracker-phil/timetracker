# TimeTracker

My personal automatic time tracker for macOS. It records which app and window I work in, in the background
and from login, and lets me turn that activity into recordings for projects by arranging blocks on a
timeline. Reports list a day or a range by project and by tag, ready to paste into a timesheet.

It is local only: no account, no server, no telemetry, no update check. No feature needs the network, and
it never captures keystrokes, screenshots, audio, the clipboard or page content.

It runs on macOS on Apple Silicon. Published as source, as-is: no releases, no binaries, no support. To
build it or port it, point a coding agent at [AGENTS.md](AGENTS.md).

![The Day view in Ayu Light](docs/screens/ayu-light-day.png)

![The Day view in Ayu Mirage](docs/screens/ayu-mirage-day.png)

The Day view in the default light (Ayu Light) and dark (Ayu Mirage) schemes; more in [docs/screens/](docs/screens/).

## Install

Build from source (needs Xcode Command Line Tools, rustup and bun; details in [AGENTS.md](AGENTS.md)):

```sh
bun install
bun run release          # src-tauri/target/release/bundle/macos/TimeTracker.app and a .dmg
scripts/install-app.sh   # links the app into ~/Applications (Spotlight, Launchpad)
scripts/install-cli.sh   # links the `timetracker` command into ~/.local/bin
```

Both scripts link the release build; `--debug` links the debug build of `bun run build:app` instead. The links
point at the build output, so a rebuild is picked up without installing again. Updates are a new build.

On first launch a welcome sheet explains what is recorded, asks for the **Accessibility** permission (window
titles and document paths) and offers **launch at login**. Without Accessibility the tracker records nothing.
Browsers ask once each whether TimeTracker may read their address bar (Automation).

- **Launch at login** (Settings → General, or the welcome sheet) installs a LaunchAgent that starts
  `timetracker --hidden`: no window, only the menu bar icon.
- **The Accessibility grant belongs to the app's code signature.** Builds are signed ad hoc, so every rebuild
  is a new app to macOS and may ask again. A rebuilt debug app asks after every build; the installed release
  app keeps the grant across launches and restarts until you rebuild it.

## The Day view

- Time runs top to bottom: the recordings column on the left, activity lanes on the right (one for the
  tracker, one per external source), away time shaded.
- Select activity (drag, click a block, ⇧-click to extend; or J/K and V) and drop it on the recordings column:
  the editor asks for a project, a one-line note and a tag.
- Recordings never overlap. Drag to move or resize, ⌘Z undoes; rules (⌥-drag a block onto a project) suggest
  recordings you accept with a click.
- The sidebar shows the day (or week, month, report period) with ‹ › and Today; P, N and O do the same, and a
  click on the date opens a calendar. ⌘1 to ⌘6 switch to Day, Week, Month, Reports, Projects and Rules.
- Everything works from the keyboard: ⌘K opens the command palette, ? lists the shortcuts.

## CLI

| Command | Does |
|---|---|
| `timetracker` | starts TimeTracker, or brings the running one to the front |
| `timetracker --hidden` | starts without opening the window (used for launch at login); a running instance stays as it is |
| `timetracker --log-level <error\|warn\|info\|debug\|trace>` | overrides the log level for this run |
| `timetracker --version`, `--help` | prints and exits |
| `timetracker show [DAY]` | shows the window at a day (`YYYY-MM-DD`, `today`, `yesterday`), starting TimeTracker if needed |
| `timetracker focus`, `hide`, `quit` | brings the window to the front (starting TimeTracker if needed), hides it, quits |
| `timetracker status [--json]` | the tracker state, the current activity and whether the window shows |
| `timetracker pause <15m\|1h\|tomorrow\|off>`, `resume` | pauses tracking (durations up to 24h; `tomorrow`: until midnight; `off`: until resumed), resumes it |
| `timetracker activity add\|start\|stop …` | writes another app's activity, see [Scripting](#scripting) |
| `timetracker activity delete <DAY>` / `activity delete <FROM> <TO>` | deletes the activity and away spans of a day or range (whole when they cross it; recordings stay) |
| `timetracker activity undo` | restores the last delete, within 30 s of it |
| `timetracker record --project P --from T --to T [--note N] [--tag T] [--replace]` | records time for a project (by id, name or a unique start of its name; one tag by name); refuses to overlap another recording unless `--replace` |
| `timetracker record --project P --span 14:25-15:00 --span 16:05-20:15 …` | one recording with several separate spans (`--from`/`--to` counts as one more) |
| `timetracker report [RANGE] [--by project\|tag\|day\|week\|month] [--client C]… [--project P]… [--tag T]… [--round 5\|6\|10\|15\|30 [--up]]` | prints the hours and notes of today (or `yesterday`, `week`, `last-week`, `month`, `last-month`, a day, or two days `FROM TO`), ready to paste into a timesheet |
| `timetracker start PROJECT [--note N] [--tag T]` / `timetracker stop` | starts a timer now (a running one stops) / stops it |
| `timetracker mcp` | serves the read-only MCP server on stdin/stdout, see [MCP](#mcp) |
| `timetracker dev seed [--years N] [--yes]` | debug builds only: writes N years (default 2) of generated workdays into the running app's database ([docs/testing.md](docs/testing.md#sample-data)) |

Times are `2026-10-21T09:30` (local, or with `Z` or `±hh:mm`), `09:30` (today) or `now`. Every command except
`show`, `focus` and `mcp` needs TimeTracker running. Exit codes: 0 ok, 1 TimeTracker refused the request (the
error is printed), 2 bad arguments or "TimeTracker is not running". `--json` prints replies as JSON.

Closing the window keeps TimeTracker running in the menu bar; quit from the menu bar item, with ⌘Q or with
`timetracker quit`.

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
timetracker report last-week --by tag --round 15
```

`--json` on `activity add` reads an array of `{ source, title, start, end, app?, url?, path? }` from stdin,
with times in UTC milliseconds. Links also take `timetracker://show?day=…`, `timetracker://activity/start?…`
and `timetracker://activity/stop?source=…`; encode a `+` in a time offset as `%2B`. The protocol behind the
CLI is in [docs/backend.md](docs/backend.md#cli-and-single-instance-handoff).

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

## Privacy

**Captured**, while tracking runs: the frontmost app (name and bundle id), its window title, the page address
in browsers that allow it, the document path of the focused window, and away time (no input for the idle
minutes, sleep, screen lock, a switched user, a pause). Settings → Capture turns addresses and document paths
off and excludes apps and websites, which are then never written.

**Never captured:** keystrokes, screenshots, screen contents, audio, the clipboard and page content. Private
browser windows are recorded as "Private", without title or address.

**Where:** one SQLite file in the data folder, by default
`~/Library/Application Support/com.philippstracker.timetracker/data/`, next to the settings (`settings.json`,
`keybindings.json`, `ui_state.json`). Logs are in `~/Library/Logs/com.philippstracker.timetracker/`.
Settings → Data shows the folder, its size and counts, with Reveal in Finder.

- **Backup:** Settings → Data → Export backup… writes every client, project, tag, activity span, recording, rule
  and saved report to one JSON file. Copying the data folder while TimeTracker is quit works too. When an
  update merges several tags of a recording into one, the data before it is saved as `pre-0006-<date>.json`
  in the data folder.
- **Delete activity:** the Day header's ⋯ menu deletes a day's or a range's activity (or
  `timetracker activity delete`). Spans that cross the range go entirely; recordings stay. Undo within 30 s.
- **Move the data folder:** Settings → Data → Move… copies the database to the chosen folder, checks the copy
  and switches to it while the app keeps running. The old files stay where they were; delete them in Finder.

## Troubleshooting

- **Tracking stopped after a rebuild** ("Needs permission", the menu bar icon shows the attention mark). macOS
  forgot the grant for the new signature. Remove TimeTracker in System Settings → Privacy & Security →
  Accessibility, then click Allow… in the app (or the welcome sheet's Grant access) and turn it on again.
- **"TimeTracker is not running" from the CLI** (exit 2). Only `show`, `focus` and a bare `timetracker`
  start the app; `mcp` works without it. Run `timetracker` (or open the app), then repeat the command. Debug
  and release builds share the socket, so the command reaches whichever one runs.
- **Launch at login stopped after moving or reinstalling the app.** The LaunchAgent records the app's path
  when it is turned on. Turn launch at login off and on again in Settings → General.
- **"Data folder … not found; using the default"** on launch: the moved data folder is missing (an unmounted
  volume). That run reads and writes the default folder instead. Mount it and restart TimeTracker.

## Docs

Technical documentation for developers and coding agents: [docs/](docs/README.md).

## License

MIT, see [LICENSE](LICENSE).
