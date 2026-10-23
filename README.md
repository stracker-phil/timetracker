# TimeTracker

My personal automatic time tracker for macOS. It records which app and window I work in, in the background
and from login, and lets me turn that activity into recordings for projects by arranging blocks on a
timeline. Reports list a day or a range by project and by tag, ready to paste into a timesheet.

It is local only: no account, no server, no telemetry, no update check. No feature needs the network, and
it never captures keystrokes, screenshots, audio, the clipboard or page content.

It runs on macOS on Apple Silicon. Published as source, as-is: no releases, no binaries, no support. To
build it or port it, point a coding agent at [AGENTS.md](AGENTS.md).

![The Day view in the Default scheme](docs/screens/default-light-day.png)

![The Day view in the Dark scheme](docs/screens/dark-light-day.png)

![The Reports screen](docs/screens/default-light-reports.png)

The Day view in the Default scheme (dark sidebar, light content) and in the Dark scheme, then Reports: a
recording in the timesheet on the left, the month calendar in the sidebar, the activity lanes on the right.
Settings → Appearance picks one of three schemes per macOS mode, and `scripts/dev/screenshot-looks.sh`
captures every look.

## Install

Build from source (needs Xcode Command Line Tools, rustup and bun; details in [AGENTS.md](AGENTS.md)):

```sh
bun install
bun run release          # builds and installs the app (Spotlight, Launchpad) and the `timetracker` command
```

`bun run release` ends with `scripts/install.sh`: `~/Applications/TimeTracker.app` links to the release build,
`~/.local/bin/timetracker` links through that app, and a running TimeTracker restarts on the new build, so one
version runs on the machine. Updates are a new build. Debug builds (`bun run build:app`) are never installed;
`scripts/dev/launch.sh` runs one by its path.

On first launch a welcome sheet explains what is recorded, asks for the **Accessibility** permission (window
titles and document paths) and offers **launch at login**. Without Accessibility the tracker records nothing.
Browsers ask once each whether TimeTracker may read their address bar (Automation).

- **Launch at login** (Settings → General, or the welcome sheet) installs a LaunchAgent that starts
  `timetracker --hidden`: no window, only the menu bar icon.
- **The Accessibility grant belongs to the app's code signature.** Run `scripts/create-signing-cert.sh` once:
  it adds a self-signed "TimeTracker Local Signing" certificate to the login keychain, and every build is
  signed with it (`scripts/sign-app.sh`), so a grant survives rebuilds. Without it builds are signed ad hoc,
  and every rebuild is a new app to macOS that asks again. The debug app is a separate entry in Privacy &
  Security, "TimeTracker (Debug)", with its own grant.

## The Day view

- The timesheet on the left lists the day's recordings as cards in the project's color, with the day's total
  on top. Beside it the timeline runs top to bottom: the hour axis, the time column with the recordings'
  spans, a summary lane (the main app of every 5 minutes) and the activity lanes (one for the tracker, one per app
  given its own lane, one per external source); the lanes show only activity, away time adds to the
  day's totals.
- Drag over time or click activity (or J/K, V and ↵): the editor opens in the timesheet, snapped to the grid,
  for a one-line note, a project (often proposed), a tag and the spans. Each further click on a block adds its
  span or takes it out again, so scattered blocks make one recording with the time between them left out.
- Recordings never overlap. Drag to move or resize, ⌘Z undoes; rules (⌥-drag a block onto a project) suggest
  recordings that wait in the timesheet as ghost cards until you approve, edit or dismiss them.
- The sidebar opens with a month calendar that is the navigation: a click on a day shows that day, on a week
  number that week, on the month's name that month; the shown day, week or month is highlighted and a dot
  marks days with recordings. P, N and O step back, ahead and to today. Below it: Hours (today's Day view),
  Reports, then Projects, Tags and Rules, and at the bottom the tracking status with play / pause and the
  Settings gear (in the status bar while the sidebar is hidden, ⌘⌃S).
- ⌘1 to ⌘7 switch to Day, Week, Month, Reports, Projects, Tags and Rules. Week and Month list the recordings
  per day (Week as cards, Month as one line each, Monday first); a card drags to another day.
- Each recording has at most one tag (Tags has a page of its own), for kinds of work across projects.
- Everything works from the keyboard: ⌘K opens the command palette, ? lists the shortcuts.
- A project is billable or not (a switch in its sheet; no rates). The status bar shows the billable share of
  the recorded time for the shown day, week, month or report period, e.g. `Billable 80 %`.

## Reports

Reports show a period (today, a week, a month or custom dates, P and N to step it) filtered by client,
project and tag: the total on top, donuts by client and by tag, then a table of every recording grouped by
client and project with their hours. Each project has **Copy notes**: its notes of the period, one per line in
time order, for pasting into a company dashboard. ⌘⇧C copies the whole report as text, ⌥⌘C all its notes;
reports can be saved under a name and sit under Reports in the sidebar.

## CLI

| Command | Does |
|---|---|
| `timetracker` | starts TimeTracker, or brings the running one to the front |
| `timetracker --hidden` | starts without opening the window (used for launch at login); a running instance stays as it is |
| `timetracker --log-level <error\|warn\|info\|debug\|trace>` | overrides the log level for this run |
| `timetracker --version`, `--help` | prints and exits |
| `timetracker show [DAY]` | shows the window at a day (`YYYY-MM-DD`, `today`, `yesterday`), starting TimeTracker if needed |
| `timetracker focus`, `hide`, `quit` | brings the window to the front (starting TimeTracker if needed), hides it, quits |
| `timetracker status [--json]` | the tracker state, the current activity, the timer, today's billable time (`billable today: 6:24 of 8:00 (80 %)`) and whether the window shows |
| `timetracker tracker pause <15m\|1h\|tomorrow\|off>`, `tracker resume`, `tracker status` | pauses activity tracking (durations up to 24h; `tomorrow`: until midnight; `off`: until resumed), resumes it, prints its state; the timer is not affected |
| `timetracker activity add\|start\|stop …` | writes another app's activity, see [Scripting](#scripting) |
| `timetracker activity delete <DAY>` / `activity delete <FROM> <TO>` | deletes the activity and away spans of a day or range (whole when they cross it; recordings stay) |
| `timetracker activity undo` | restores the last delete, within 30 s of it |
| `timetracker record --project P --from T --to T [--note N] [--tag T] [--source S]` | suggests time for a project (by id, name or a unique start of its name; one tag by name), to review in the Day view ("Suggested by cli", or `S`); see [Suggestions and recordings](#suggestions-and-recordings) |
| `timetracker record … --final [--replace]` | records it instead; refuses to overlap another recording unless `--replace` |
| `timetracker record --project P --span 14:25-15:00 --span 16:05-20:15 …` | several separate spans (`--from`/`--to` counts as one more) |
| `timetracker edit ID [--project P] [--note N] [--tag T\|--no-tag] [--span …] [--final [--replace]]` | changes a suggestion; with `--final` changes a recording, or records the suggestion with the changes |
| `timetracker delete ID [--final]` | deletes a suggestion (a rule's is dismissed); with `--final` a recording |
| `timetracker approve ID…` | records suggestions as they are |
| `timetracker suggestions [RANGE]`, `recordings [RANGE]` | lists them with their ids (`RANGE` as for `report`; suggestions of at most 62 days) |
| `timetracker projects [--archived]`, `clients [--archived]`, `tags` | lists the names a command takes, with their ids |
| `timetracker undo`, `redo` | undoes or redoes the last change to recordings or suggestions, as ⌘Z in the app |
| `timetracker report [RANGE] [--by project\|tag\|day\|week\|month] [--client C]… [--project P]… [--tag T]… [--round 5\|6\|10\|15\|30 [--up]] [--notes]` | prints the hours and notes of today (or `yesterday`, `week`, `last-week`, `month`, `last-month`, a day, or two days `FROM TO`), ready to paste into a timesheet, then `Billable: 6:24 of 8:00 (80 %)`; `--json` has both numbers in `billable`. `--notes` prints only the notes, one per line in time order (`report today --notes --project P`: that project's, as the Reports screen's Copy notes) |
| `timetracker timer start PROJECT [--note N] [--tag T]`, `timer stop` | starts a timer now (a running one stops; the same project, note and tag continue today's recording) / stops it |
| `timetracker timer status [--json]` | the running timer: `--json` has `running`, `project`, `client`, `tag`, `note`, `startedAt` (ms), `startedAtIso`, `elapsedMs` and the ids |
| `timetracker timer update [--project P] [--note N] [--tag T\|--no-tag]` | changes the running timer in place |
| `timetracker mcp` | serves the MCP server on stdin/stdout, see [MCP](#mcp) |
| `timetracker dev seed [--years N] [--yes]` | debug builds only: writes N years (default 2) of generated workdays into the running app's database ([docs/testing.md](docs/testing.md#sample-data)) |

Times are `2026-10-21T09:30` (local, or with `Z` or `±hh:mm`), `09:30` (today) or `now`. Every command except
`show`, `focus` and `mcp` needs TimeTracker running. Exit codes: 0 ok, 1 TimeTracker refused the request (the
error is printed), 2 bad arguments or "TimeTracker is not running". `--json` prints replies as JSON. The
older `start`, `stop`, `pause` and `resume` still work.

Closing the window keeps TimeTracker running in the menu bar, and ⌘Q also hides the Dock icon; recording
goes on. Quit with "Quit TimeTracker" (app menu, menu bar item or palette) or `timetracker quit`.

### Suggestions and recordings

Writes from other apps are suggestions you review, unless they say `--final` (ADR
[0028](docs/adr/0028-outside-writes-are-suggestions.md)). A suggestion shows in the Day view like a rule's;
approve it there or with `approve`, and every change is undoable. `--draft` keeps a write a suggestion even
next to `--final`, so a wrapper script can't be talked into changing recordings. The timer is direct: `timer
start`, `stop` and `update` change the running recording.

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

A reminder that asks "still working on it?", and a planner whose blocks wait for review:

```sh
note=$(timetracker timer status --json | jq -r 'select(.running) | .note')
if [ -n "$note" ]; then
  answer=$(osascript -e "text returned of (display dialog \"Still on it?\" default answer \"$note\")")
  timetracker timer update --note "$answer"   # the same note changes nothing
fi
timetracker record --project acme --span 09:00-09:30 --note Planning --source planner
timetracker suggestions --json | jq -r '.[] | select(.by == "planner") | .id' | xargs timetracker approve
```

`--json` on `activity add` reads an array of `{ source, title, start, end, app?, url?, path? }` from stdin,
with times in UTC milliseconds. Links also take `timetracker://show?day=…`, `timetracker://activity/start?…`
and `timetracker://activity/stop?source=…`; encode a `+` in a time offset as `%2B`. The protocol behind the
CLI is in [docs/backend.md](docs/backend.md#cli-and-single-instance-handoff).

## MCP

`timetracker mcp` lets Claude (or any MCP client) read your recordings, projects, tags and reports, also while
the app is closed, and suggest time for you to review. Nothing leaves the Mac except what the client sends to
its model. Add it to
Claude Code (Settings → Data shows this line with the path of your binary, ready to copy; add `--scope user`
to use it in every project):

```sh
claude mcp add timetracker -- ~/.local/bin/timetracker mcp
```

Then ask "what did I record yesterday by project?" or "how many hours of development this month?". The tools:
`report` (the report text, or JSON with the billable minutes and percent), `recordings`, `projects` (with
`billable`), `tags`, `day_summary`, `status` and `open_day`
(these two need the app running), `clients`, `suggestions`, and `activity` (window titles and URLs), which
stays off until you enable "Expose activity to the MCP server" in Settings → Data.

Writes need the app running and work like the CLI's ([Suggestions and recordings](#suggestions-and-recordings)):
`record` and `edit` leave suggestions in the Day view, "Suggested by" the client's name (`claude-code`), until
you approve or dismiss them; `final: true` changes the timesheet itself (`draft: true` keeps it a suggestion).
`approve` records suggestions, `delete` dismisses one (a recording only with `final`), and `timer_start`,
`timer_stop` and `timer_update` drive the timer directly. Ask "suggest 9 to 10:30 today for Acme with the note
from this ticket".

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
