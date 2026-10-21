# Backend (Rust core)

This page covers `src-tauri/`, the Rust side of TimeTracker.

## Startup

`main.rs` calls `timetracker_lib::run()` (`lib.rs`):

1. **Config dir.** `config_dir()` resolves `~/Library/Application Support/<IDENTIFIER>` (no home dir: exit 2)
   and `create_private_dir` (`store/json_file.rs`) makes it mode 0700. This happens before Tauri starts,
   because the CLI and the socket need the path first. `Settings::peek` reads the stored log level without
   ever quarantining a corrupt file.
2. **CLI.** `cli::parse(std::env::args_os())` returns:
   - `Exit { message }`: `--help` or `--version`; printed, exit 0.
   - `Run { hidden, log_level }`: becomes `IpcMessage::Activate { hidden }`.
   - an error: printed to stderr and turned into `IpcMessage::Error`; the exit code after forwarding is 2,
     and a newly started app shows it.
3. **Single-instance handoff.** `forward` sends the message with `ipc::try_forward`. `Delivered` exits;
   `NoInstance` continues. Then `lock_instance` takes `timetracker.lock`; if another process holds it,
   `forward_to_starting_instance` retries the socket 25 × 200 ms while that process starts up.
4. **`AppState`.** `AppState::open(&config_dir)` opens every JSON store and the data store
   (`DataStore::open`, blocking on Tauri's async runtime; a failure is logged and the app runs without a
   database, see [data.md](data.md#the-data-folder)); the CLI `--log-level` is kept in
   `AppState::log_override`. The effective level (override, else the stored `logLevel`) is computed here, so
   logging never starts at `Trace`.
5. **`start_app`** builds Tauri with `.manage(state)` first, then `remote::Bridge`:
   - plugins: `log_plugin()` (stdout plus a rotating file in the log dir, 3 × 5 MiB, `tao`/`wry` capped at
     Warn), `init_logging(level)` (sets the effective level right after the log plugin's setup, whose
     dispatch level caps every later change), opener, dialog, deep-link, autostart
     (`MacosLauncher::LaunchAgent` with `--hidden`);
   - `on_menu_event` → `menu::forward`;
   - `on_window_event`: `CloseRequested` on `main` prevents the close and hides the window.

   Then `setup` runs: it logs the version, sets the native menu (`menu::build`), queues the initial launch
   message (`LaunchQueue::push`), calls `commands::apply_settings(app, None, &settings)`, installs the tray
   (`tray::install`), starts the tracker (`start_tracker`: only with an open data store; see
   [capture.md](capture.md)), applies `TIMETRACKER_WINDOW_FRAME` in debug builds (`apply_dev_frame`), binds and
   serves the socket (`start_ipc`; a bind error is logged and the app runs without handoff), registers
   `deep_link::listen`, and hides the window when started with `--hidden`.

   The run loop prevents `RunEvent::ExitRequested { code: None }`, which macOS sends when the last window
   closes; an explicit `app.exit(0)` carries a code and passes. On `RunEvent::Exit` it closes the database
   pool.

### Settings side effects

`update_settings` and startup both call `commands::apply_settings(app, previous, saved)`. The decision is the
pure `settings_effects(previous, saved, log_override) -> SettingsEffects { theme, log_level, launch_at_login }`:
without `previous` everything applies; otherwise only what changed. A CLI `--log-level` wins for the app's
lifetime, so a settings change never resets it. `launch_at_login` enables or disables the autostart
LaunchAgent (`~/Library/LaunchAgents`) only when its state differs.

The window is declared in `tauri.conf.json`: label `main`, 1200 × 800 (minimum 900 × 560), overlay title
bar with hidden title, traffic lights at 16/20, transparent background with the `sidebar` vibrancy effect.
No Rust code creates windows yet.

## Window lifecycle

| Event | Result |
|---|---|
| close button / ⌘W (`window.hide`, `hide_window`) | the window hides; the app, the tray and the Dock icon stay |
| last window closed (`ExitRequested { code: None }`) | prevented |
| tray "Open TimeTracker", a second `timetracker` launch | `window::show_main`: show, unminimize, focus |
| `timetracker --hidden` (launch at login) | first instance: the window hides right after setup; running instance: nothing |
| ⌘Q (`app.quit`), tray "Quit TimeTracker", `quit_app` | `app.exit(0)` |

`window.rs` also has `apply_window_theme` (the window's appearance follows the theme setting, so the
vibrancy matches) and `bring_to_front`, which does nothing with `TIMETRACKER_BACKGROUND=1` in debug builds.

## AppState

`state.rs`. Tauri manages it; every command receives it as `State<'_, AppState>`. Each store locks inside
itself. Panics unwind ([ADR 0014](adr/0014-panic-unwind.md)), so code takes `std::sync` locks with
`lock::MutexExt::lock_unpoisoned` and `lock::RwLockExt::{read_unpoisoned, write_unpoisoned}`.

| Field | Type | Holds |
|---|---|---|
| `settings` | `SettingsStore` | `settings.json` |
| `keybindings` | `KeybindingsStore` | `keybindings.json`; also feeds the menu |
| `ui_state` | `UiStateStore` | `ui_state.json` |
| `launches` | `LaunchQueue` | launch events the page has not taken yet |
| `data` | `DataStore` | the database file and the open `Db`, or why it could not be opened ([data.md](data.md)) |
| `config_dir` | `PathBuf` | the config dir |
| `log_override` | `Option<LogLevel>` | the CLI `--log-level` |

The tracker is managed beside `AppState` as `Arc<Tracker>` (`tracker/mod.rs`), and only when the data store
opened. Its commands look it up with `try_state` and fail with `db` "The data store is not available"
otherwise. `update_settings` hands the new `tracker` and `capture` settings to `Tracker::set_settings` and
`Tracker::set_capture`.

## Tauri commands

Registered in `start_app` (`generate_handler!`). Commands stay thin; logic lives in its module.

| Command | Args | Returns | Errors |
|---|---|---|---|
| `get_settings` | – | `Settings` | – |
| `update_settings` | `settings: Settings` | `Settings` (sanitized) | `io` |
| `get_keybindings` / `set_keybindings` | – / `bindings: Keybindings` | `Keybindings` (sanitized; the menu is rebuilt) | `io` |
| `get_ui_state` / `set_ui_state` | – / `ui: UiState` | `UiState` | `io` |
| `open_log_folder` | – | – | `io` |
| `open_url` | `url: String` | – | `invalid_input` unless `http(s)` without credentials (`is_web_url`) |
| `log_frontend_error` | `message: String` | – | – |
| `take_launch_events` | – | `LaunchEvent[]` | – |
| `remote_reply` | `id, result, error` | – | – |
| `hide_window` | – | – | – |
| `quit_app` | – | never returns | – |
| `choose_save_path` | `fileName, extension` | `string \| null` (the save panel; `null` when cancelled) | `io` |
| `reveal_path` | `path: String` | – (Finder shows the enclosing folder with `path` selected) | `io` |
| `data_info` (`db/commands.rs`) | – | `DataInfo { dir, file, sizeBytes, activities, recordings, projects, firstActivity, available, error }` | – (errors are reported inside) |
| `backup_export` (`db/commands.rs`) | `path: String` (absolute) | `BackupWritten { bytes }` | `db`, `io`, `invalid_input` |
| `day_view` (`day/commands.rs`) | `day: String` | `DayView` (see [Day view](#day-view)); logs the time it took at debug level | `invalid_input`, `db` |
| `app_settings_set` (`commands.rs`) | `bundleId: String, patch: AppSettingPatch { ownLane?, hidden? }` | `Settings`; an override that ends up default is removed (`Settings::with_app`); emits `data_changed { kind: settings, day: null }` | `invalid_input` (blank bundle id), `io` |
| `activity_list` / `away_list` (`db/commands.rs`) | `day: Day` | `ActivityRow[]` / `AwayRow[]` of the day, cut to its bounds; open rows end now | `db`, `invalid_input` |
| `apps_seen` (`db/commands.rs`) | `days: u16` | `AppSeen[] { bundleId, app, lastSeen, totalMs }`: tracker apps of the last `days` days, one per bundle id, most time first | `db` |
| `tracker_status` (`tracker/commands.rs`) | – | `TrackerStatus` (the last one sent) | `db` (tracker off) |
| `tracker_pause` | `until: PauseUntil` | `TrackerStatus` | `db` |
| `tracker_resume` | – | `TrackerStatus` | `db` |
| `request_permission` | – | `PermissionStatus` (after the system prompt, shown once per run) | `db` |
| `open_privacy_settings` | – | – (System Settings → Privacy & Security → Accessibility) | `io` |
| `clients_list` (`projects/mod.rs`) | `includeArchived: bool` | `Client[]` by name | `db` |
| `clients_save` | `id: string \| null, input: ClientInput` | `Client`; `null` creates; a new color recolors its projects | `invalid_input` (`field`), `not_found`, `db` |
| `clients_archive` | `id, archived: bool` | `Client`; archiving archives its projects | `not_found`, `db` |
| `projects_list` | `includeArchived: bool` | `ProjectListItem[]` | `db` |
| `projects_save` | `id: string \| null, input: ProjectInput` | `Project`; `null` creates | `invalid_input` (`field`), `not_found`, `db` |
| `projects_archive` | `id, archived: bool` | `Project` | `not_found`, `db` |
| `projects_recent` | `limit: u32` | `Project[]` by latest recording | `db` |
| `tags_list` | – | `TagListItem[]` by name | `db` |
| `tags_save` | `id: string \| null, name` | `Tag`; `null` creates or returns the tag with that name | `invalid_input` (`field`), `not_found`, `db` |
| `tags_delete` | `id` | – | `not_found`, `db` |
| `activity_add` (`activity/commands.rs`) | `items: ActivityInput[]` | `number[]` (the new ids) | `invalid_input`, `overlap`, `db` |
| `activity_start` | `input: ActivityInput` | `ActivityRow` (open) | `invalid_input`, `overlap`, `db` |
| `activity_stop` | `source: String, end: Ms \| null` | `ActivityRow \| null` | `invalid_input`, `db` |
| `activity_sources` | – | `string[]`: external source names, the latest first | `db` |

To add a command: write it in its module (or `commands.rs` for app-wide ones), register it in
`generate_handler!`, export its types with ts-rs, add the wrapper to `src/api/index.ts` and a default to
`src/test/apiMock.ts`.

## Day view

`day/mod.rs`: `view(&DayInput, &ViewSettings) -> DayView` is a pure function ([ADR
0012](adr/0012-timeline-layout-in-rust.md)); `day::commands::load` reads the rows (`list_for_range`, spans
from the day before included) and calls it. `ViewSettings` is a copy of `Settings.timeline` and
`Settings.apps`. In order:

1. `clipped_blocks`: every activity cut to the day's bounds (an open span ends at `now`); private rows keep
   the app and read "Private" without URL or path. Block ids are `a<activity id>`, `members` the activity ids.
2. `without_hidden_apps`: apps with `apps[bundleId].hidden` vanish (lanes, totals, gaps).
3. Seams for timeline settings, each a named function that returns its input for now: `group_by_app`,
   `bridge_gaps` (continuity), `drop_noise`, `choose_titles`, and `summary_lane` (returns no lane).
4. `lanes_of`: the `activity` lane (always), one `app:<bundleId>` lane per app with `ownLane`, one
   `source:<name>` lane per external source; app and source lanes sorted by label.
5. `packed` per lane: by start, longer first; each block takes the lowest free column; a cluster of
   overlapping blocks shares its width (`col` of `cols`).
6. Away blocks (clipped), gaps (`GAP_MIN` 2 min or longer, between the first activity's start and the last
   one's end, without activity or away) and totals: `trackedMs` the union of tracker spans, `awayMs` the
   union of away spans, `recordedMs` 0 until recordings. `covered` is false until recordings.

## Events to the page

`events.rs` names every event and its payload, so all emitting goes through one place.

| Constant | Name | Payload |
|---|---|---|
| `LAUNCH_EVENT` | `launch` | none; the page calls `take_launch_events` |
| `COMMAND_EVENT` | `command` | a command id (menu or tray) |
| `REMOTE_EVENT` | `remote` | `RemoteEvent { id, request }`, answered with `remote_reply` |
| `DATA_CHANGED_EVENT` | `data_changed` | `DataChanged { kind, day }` |
| `TRACKER_STATUS_EVENT` | `tracker_status` | `TrackerStatus` on every change and at least every 10 s ([capture.md](capture.md#status)) |

`DataKind` is `activity | recordings | projects | tags | rules | settings`; `day` is a `Day` (`YYYY-MM-DD`)
or `null` when every day may have changed. `events::data_changed(app, kind, day)` emits it; external
activity writes emit one per local day the span touches (`activity::external::days_touched`). The page bumps
that day's version (or every day's for `null`), so views showing the day reload
([frontend.md](frontend.md)).

## CLI and single-instance handoff

The `timetracker` command is the app binary. Without a subcommand it starts the app, or activates the
running one; every subcommand is a second process that sends one request over the socket, prints the reply
and exits ([ADR 0017](adr/0017-cli-asks-the-running-app.md)). It never opens the database itself.

### Command tree

`cli/mod.rs` (clap derive). Global flags: `--hidden` (start without the window; a running instance stays
as it is), `--log-level <error|warn|info|debug|trace>` (overrides the setting for this run), `--json`
(replies as JSON), `--help`, `--version`.

| Subcommand | `IpcMessage` | Prints |
|---|---|---|
| `show [DAY]` | `show { day, focus: false }`; `DAY` is `YYYY-MM-DD`, `today` or `yesterday` (default today) | – |
| `focus` | `activate { hidden: false }` | – |
| `hide`, `quit` | `hide`, `quit` | – |
| `status` | `status` | five lines, below |
| `pause <15m\|1h\|tomorrow\|off>` | `pause { until: PauseUntil }`; `<n>m` or `<n>h` up to 24 h, `tomorrow` = next local midnight, `off` = until resumed | `tracker: paused until 14:30` |
| `resume` | `resume` | – |
| `activity add --source S --title T --start ISO --end ISO [--app A] [--url U] [--path P]` | `activityAdd { items: [ActivityInput] }` | `added activity 4711` |
| `activity add --json` (no other flags) | `activityAdd`, items from a JSON array of `ActivityInput` on stdin (times in UTC ms) | `added 3 activities` |
| `activity start --source S --title T [--start ISO] [--app A] [--url U] [--path P]` | `activityStart { input }`, start default now | `started planner at 10:00` |
| `activity stop --source S [--end ISO]` | `activityStop { source, end }`, end default now | `stopped planner at 10:45 (45 min)` or `nothing to stop for planner` |

`parse(args, now, tz, stdin) -> Result<CliOutcome, CliError>` is pure: `CliOutcome` is `Run { hidden,
log_level }`, `Exit { message }` (help, version) or `Remote { message, json, starts_app }`. The CLI checks
what it can before sending (times, `activity::external::validate`); the app checks again. `CliError.in_app`
is true for a plain launch with bad arguments, which also shows the error in the app (it may have been opened
from Finder); a subcommand's error only prints.

`cli/time.rs`: `instant` reads `2026-10-21T09:30[:15]` (local), the same with `Z` or `±hh:mm`, `09:30`
(today, local) and `now`; `day` reads `today`, `yesterday` and `YYYY-MM-DD`; `pause` reads the pause words.
The zone is a parameter, so tests use fixed zones.

`cli/reply.rs`: `render(message, result, now, tz)` is the human text. `status` prints one fact per line, in
this order, so scripts can grep:

```
TimeTracker 0.1.0
tracker: running          (paused | paused until 14:30 | paused until tomorrow | away since 13:02 (idle) | needs permission | off)
current: Safari · Pull request #42 · 13 min        (current: none)
timer: none
window: hidden
```

`current` is the tracker's open activity only; an open external span (`activity start`) does not show here.
`--json` prints the reply's `result` instead (`status --json` is a `StatusReport { version, tracker,
timer, windowVisible }`; `tracker` is `null` while the data store is unavailable); an error prints
`{ "kind", "message" }`.

**Exit codes:** 0 ok; 1 the app answered with an error; 2 bad arguments, or TimeTracker is not running.
With no instance, `show` and `focus` start the app with their message queued (`starts_app`); every other
subcommand exits 2 with `TimeTracker is not running`.

**Adding a subcommand:** a `Command` variant and its arm in `cli::message`, an `IpcMessage` variant, its arm
in `remote::handle`, a `reply::render` arm when it prints something, and parser tests.

### Socket and protocol

`ipc.rs`:

- **Socket:** a Unix domain socket at `<config dir>/timetracker.sock`, mode 0600 in a 0700 dir. `serve` drops
  peers with another uid (`same_user`, `libc::getuid`).
- **Protocol:** one line of JSON per connection, at most 64 KiB (`MAX_MESSAGE_BYTES`), within 5 s. Any process
  of the same user may speak it:
  - one-way (a bare `IpcMessage`): answered with `ok\n` before it is handled (`try_forward`);
  - request (`IpcRequest { id, message }`, sent by `ipc::request`): answered once handled with one
    `IpcReply { id, ok, result, error }` line, `error` being `IpcError { kind, message }` (`kind` as in
    [Errors](#errors)); `IpcReply::into_result` turns it into a `HandlerResult`.

  ```
  → {"id":"7f0c…","message":{"type":"activityStop","source":"planner","end":null}}
  ← {"id":"7f0c…","ok":true,"result":{"id":4711,"source":"external:planner",…},"error":null}
  ```

  Malformed, oversized or incomplete lines get no answer. `cli::split` cuts an `activity add` batch into
  requests of at most 200 items that each fit the limit; such a batch is written part by part, and a failing
  part stops the rest (the CLI prints the ids already written).
- **`Forward` outcomes** of `try_forward`: `Delivered` (exit 0, 2 for a CLI error), `NoInstance` (socket
  missing or refused: start the app), `Rejected` (no `ok`) or an error such as a timeout (print and exit 2,
  so a hung instance never leads to a second one).
- **Messages** (`IpcMessage`, tagged by `type`): `activate { hidden }`, `error { message }`, the subcommand
  messages above, and `eval { js }` (debug builds only, sent by `scripts/dev/js.sh`).
- **Stale sockets:** `bind` removes a leftover socket only when connecting to it is refused; it refuses to
  bind while a listener is alive.

### Dispatch

`remote::handle(app, message)` maps each message to the module that does it; it is the one table to extend.
Every message is logged by type (`IpcMessage::kind`, never the contents).

| Message | Does | Result |
|---|---|---|
| `activate`, `error` | `launch::queue`: pushed to `LaunchQueue` (`error` → `LaunchEvent::Error`, `activate { hidden: false }` → `Activate`), `launch` emitted, the window shown unless `hidden` | `null` |
| `show { day, focus }` | shows the window (`window::show_main`, or `show_main_in_background` without `focus`), then asks the page `ShowDay { day }` (today when `null`) | `null` |
| `hide` | `window::hide_main` | `null` |
| `quit` | `app.exit(0)` 200 ms after replying | `null` |
| `status` | `remote::status_report` | `StatusReport` |
| `pause { until }`, `resume` | `Tracker::pause`, `Tracker::resume` | `TrackerStatus` |
| `activityAdd { items }` | `activity::add` | `{ ids }` |
| `activityStart { input }` | `activity::start` | `ActivityRow` |
| `activityStop { source, end }` | `activity::stop` | `ActivityRow` or `null` |
| `eval { js }` | runs in the window | `null` |

Messages that start the app are pushed to `LaunchQueue` in `setup` instead: `show` becomes
`LaunchEvent::ShowDay { day }`, so the page navigates once it has loaded.

### Requests the page answers

`remote.rs`: `Bridge::ask` emits `remote` with a fresh id (`pending::PendingReplies`) and waits up to 4 s
(below the IPC client's 5 s) for `remote_reply`. `RemoteRequest` is `showDay { day }`;
`src/commands/remote.ts` (`answerRemote`) always replies, with an error for a request it does not know.

### Deep links

`deep_link.rs`: `listen` registers `on_open_url` for `timetracker://` (scheme in `tauri.conf.json`
`plugins.deep-link`). macOS delivers a link to the running app, or starts the app and then delivers it.
`link_message(url, now, tz)` maps a link to a message, reading times as the CLI does:

| Link | Message |
|---|---|
| `timetracker://show?day=2026-10-21` | `show { day, focus: true }` |
| `timetracker://activity?source=planner&title=Lunch&start=2026-10-21T12:00&end=2026-10-21T12:45[&app=&url=&path=]` | `activityAdd` with one item |
| `timetracker://activity/start?source=…&title=…[&start=…]` | `activityStart` |
| `timetracker://activity/stop?source=…[&end=…]` | `activityStop` |

Query values are percent-decoded and `+` reads as a space, so an offset like `+05:30` must be written
`%2B05:30`.

`show` links are queued like a launch (`launch::queue`), so a link that starts the app waits for the page.
Activity links run through `remote::handle` at once and do not show the window. A link that does not parse
or fails becomes `IpcMessage::Error`: the window shows a toast, nothing is written.

## Tray

`tray.rs`: `tray::install(app)` builds the menu bar item `main` as a template image (macOS tints it for the
menu bar), tooltip `TimeTracker`, menu on left click:

| Item | Id | Action |
|---|---|---|
| the tracker's status text (disabled) | `status` | relabelled on every status |
| Pause Tracking ▸ For 15 Minutes, For 1 Hour, Until Tomorrow, Until Resumed | `pause-15`, `pause-60`, `pause-tomorrow`, `pause-resumed` | `Tracker::pause` (hidden while paused) |
| Resume Tracking | `resume` | `Tracker::resume` (only while paused) |
| Open TimeTracker | `open` | `window::show_main` |
| Quit TimeTracker | `quit` | `app.exit(0)` |

`tracker::start` passes every tracker status to `tray::sync(app, &status)`. The icon
follows `TrayIcon::of(state)`: `icons/tray/tray@2x.png` (running or away), `paused@2x.png` (a pause sign in
the clock) and `attention@2x.png` (an exclamation mark: the permission is missing). `sync` only relabels the
status item while the icon stays the same, and rebuilds menu and icon when it changes, so an open menu is not
replaced every few seconds.

## Persistence

Everything lives in the config dir, `~/Library/Application Support/com.philippstracker.timetracker/`.
JSON stores wrap a `JsonStore<T>` (`store/json_store.rs`): the parsed value behind an `RwLock`, with
`read(|t| ..)`, `update` and `update_async(|t| ..) -> AppResult<R>`. `update_async` runs on a blocking thread:
under a write gate it applies the closure to a clone, saves the clone and only then swaps it in, so a failed
save leaves memory unchanged. `JsonStore::open_with` normalizes the loaded value (settings use
`Settings::sanitized`). The file itself is `JsonFile` (`store/json_file.rs`), which:

- writes pretty-printed JSON to `<name>.tmp` (mode 0600), `sync_all`s it and renames it over the target;
- loads defaults when a file is missing;
- moves a file it cannot read or parse aside to `<name>.<UTC yyyymmddTHHMMSS>.corrupt` (with a counter
  when taken) and logs a warning. If that rename fails, the file is poisoned: `save` refuses with
  `AppError::Io` instead of overwriting it.

| File | Contents |
|---|---|
| `settings.json` | `Settings` (camelCase, `#[serde(default)]`; nested `tracker`, `capture`, `timeline`, `apps`, `reports` each `#[serde(default)]`). Enum, free-text and nested fields use `deserialize_with = "lenient"`: an unknown value falls back to that field's default instead of quarantining the file. `sanitized()` clamps on load and on update: `uiFontSize` 10..=24, `dataFontSize` 9..=28, `tracker.idleMinutes` 1..=120, `tracker.awayPromptMinutes` 1..=240, `tracker.pollSeconds` 1..=10, `timeline.zoom` 0.5..=4.0 (non-finite: 1.0); snaps `timeline.summaryMinutes` to `SUMMARY_MINUTES` and `timeline.snapMinutes` to `SNAP_MINUTES` (nearest, smaller on a tie); resets unknown accents (`ACCENTS`) and schemes (`SCHEMES`) to `orange`, `ayu-light`, `ayu-mirage`; turns blank `fontUi`, `fontMono` and `dataDir` into `null`; a blank `timerShortcut` becomes `Mod+Shift+T`. `SettingsStore::update` returns `(previous, saved)` from one lock. |
| `keybindings.json` | `{ "commandId": "Mod+Shift+X" \| null }` overrides; `null` unbinds. `keybindings::sanitize` drops (and logs) entries whose id is not `[A-Za-z0-9.]+` or whose binding is malformed. A trailing `++` is the `+` key |
| `ui_state.json` | `UiState`: `sidebarHidden`, `sidebarWidth` (px, default 200), `lastDay`, `timelineZoom` (the Day view's px per minute, default 2.5), `paneSizes` (px by pane id) |
| `data/timetracker.sqlite` (+ `-wal`, `-shm`) | clients, projects, tags, activity, away spans, recordings ([data.md](data.md)); `dataDir` in `settings.json` points elsewhere |
| `timetracker.sock`, `timetracker.lock` | single instance |

`src/api/types.ts` mirrors the constants the page needs (`ACCENTS`, `SCHEMES`, `SETTINGS_LIMITS`,
`TEXT_SIZES`); a Rust test compares `ACCENTS` and `SCHEMES` with it.

**Adding a setting:**
1. Add the field to `Settings` (or its nested struct) in `store/settings.rs`, with a default.
2. Clamp it in `sanitized`; give an enum field `#[serde(deserialize_with = "lenient")]`.
3. Regenerate `src/api/types.gen.ts` (`bun run types`), mirror a numeric clamp in `SETTINGS_LIMITS`, and
   add the default to `src/test/settings.ts`.
4. Render it on its page in `src/components/settings/pages.tsx`.

## Logging

`tauri-plugin-log` writes to stdout and to Tauri's `app_log_dir`,
`~/Library/Logs/com.philippstracker.timetracker/`, rotating 3 files of 5 MiB, local timestamps. The level
comes from `logLevel` (Settings → System → Logging) or `--log-level`. `open_log_folder` opens the folder.
Text from `log_frontend_error` is cut to 4 KiB and its `\n`/`\r` escaped (`sanitize_log_text`), so the page
cannot flood or forge log lines. Window titles, URLs and paths are user data: log them at Debug at most.

## Errors

`error.rs`: `AppError` serializes through `ErrorJson` as `{ kind, message, code, field?, details? }`:

- `message`: the `Display` text.
- `code`: SQLite's extended result code of `AppError::Db`, e.g. `2067` for a unique constraint (`null` otherwise).
- `field`: `{ path, message }` for `AppError::InvalidField` (kind `invalid_input`).
- `details`: what an `overlap` error collides with (`AppError::Overlap`).

| kind | Raised for |
|---|---|
| `db` | database errors (`From<sqlx::Error>`, `From<MigrateError>`); the data store is not available |
| `io` | file errors (`From<std::io::Error>`, message `File error: …`) |
| `invalid_input` | bad arguments (`From<serde_json::Error>` too); a form field (`InvalidField`) |
| `not_found` | an id that names nothing |
| `overlap` | a recording that would overlap others |
| `permission_required` | capture needs a macOS permission |
| `tracker_paused` | an action that needs the running tracker |
| `session_not_found` | a stale session id |
| `cancelled` | the user cancelled |

ts-rs generates `AppError`, `AppErrorField` and `ErrorKind` into `src/api/types.gen.ts`.

## Native menu

`menu.rs` builds TimeTracker (About, Settings…, Services, Hide, Hide Others, Show All, Quit TimeTracker),
Edit (the native text items), View, Window and Help. Items that map to app commands are listed once in
`MENU_ITEMS` (id, label, default binding); `build()` places them (`submenu` takes groups of ids). Their
bindings come from `KeybindingsStore::menu_binding`: the user override, else the default, which mirrors the
`defaultBinding` of the TypeScript command.

| Id | Label | Default |
|---|---|---|
| `settings.toggle` | Settings… | ⌘, |
| `app.quit` | Quit TimeTracker | ⌘Q |
| `palette.toggle` | Command Palette… | ⌘K |
| `sidebar.toggle` | Toggle Sidebar | ⌘⌃S |
| `view.day` … `view.projects` | Day, Week, Month, Reports, Projects | ⌘1 … ⌘5 |
| `zoom.in` / `zoom.out` / `zoom.reset` | Zoom In / Out / Reset Zoom | ⌘= / ⌘- / ⌘0 |
| `window.hide` | Close Window | ⌘W |
| `shortcuts.show` | Keyboard Shortcuts | ⇧? |
| `logs.open` | Open Log Folder | – |

A click calls `menu::forward`: `app.quit` exits right there, so quitting works even when the page is broken;
every other listed id is emitted as `command` (`is_menu_command` filters out the native items).

`label_and_accelerator` turns a binding with ⌘ or ⌃ (`keybindings::is_chord`) into an accelerator; any other
binding is appended to the label (`Keyboard Shortcuts (⇧?)`), because a bare-key accelerator would take its
key from text fields. `keybindings::accelerator` converts to Tauri's form (`Mod` → `CmdOrCtrl`, `-` →
`Minus`, `=` → `Equal`, `+` → `Shift+Equal`). `set_keybindings` rebuilds the menu (`menu::rebuild`).

To add a menu item: add the TypeScript command, add the id, label and default to `MENU_ITEMS`, place it in
`build()`.

## Dependencies

| Crate | Use |
|---|---|
| `tauri` (features `macos-private-api`, `tray-icon`, `image-png`), `tauri-build` | app framework, transparent window, tray |
| `tauri-plugin-log`, `log` | logging |
| `tauri-plugin-opener` | open web links and the log folder |
| `tauri-plugin-dialog` | native panels |
| `tauri-plugin-deep-link` | `timetracker://` links |
| `tauri-plugin-autostart` | launch at login (LaunchAgent) |
| `tokio` | runtime, IPC socket, the tracker loop |
| `objc2`, `block2`, `objc2-foundation`, `objc2-app-kit`, `objc2-core-foundation`, `objc2-core-graphics`, `objc2-application-services` (macOS only) | the capture source ([capture.md](capture.md#macos-apis)) |
| `sqlx` (`sqlite` with bundled libsqlite3, `runtime-tokio`, `migrate`, `macros` for `migrate!` and `FromRow`) | the data store |
| `serde`, `serde_json` (`preserve_order`) | IPC, stores |
| `clap` | CLI |
| `thiserror` | `AppError` |
| `chrono`, `uuid`, `url` | time stamps, ids, `is_web_url` |
| `ts-rs` | TypeScript types for IPC |
| `libc` | `getuid` for the IPC peer check |
| `tempfile` (dev, and optional behind `test-util`) | store, socket and database tests |
| `chrono-tz` (dev) | fixed time zones in the calendar tests |

No HTTP client or network crate ([ADR 0013](adr/0013-no-network.md)); a test checks the macOS dependency
graph. The library builds as `rlib`. The release profile uses LTO, `codegen-units = 1` and strip, and keeps
`panic = "unwind"`. The toolchain is pinned to stable in `rust-toolchain.toml`; edition 2024.

## Content security policy

`app.security.csp` in `tauri.conf.json`:

- `default-src 'self'`: scripts, fonts and frames come from the bundle only;
- `img-src 'self' data:`: no view loads a remote image, even if a window title contains a URL;
- `style-src 'self' 'unsafe-inline'`: React `style` attributes;
- `connect-src ipc: http://ipc.localhost`: Tauri IPC only.

`app.security.devCsp` applies under `tauri dev` and adds inline scripts and Vite's HMR websocket on
`localhost:1420`/`1421`.
