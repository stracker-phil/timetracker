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
   - `Mcp`: `timetracker mcp` exits with `mcp::run(&config_dir, socket)` right here, before the handoff, the
     instance lock and Tauri, so it runs beside the app ([MCP server](#mcp-server)).
   - an error: printed to stderr and turned into `IpcMessage::Error`; the exit code after forwarding is 2,
     and a newly started app shows it.
3. **Single-instance handoff.** `forward` sends the message with `ipc::try_forward`. `Delivered` exits;
   `NoInstance` continues. Then `lock_instance` takes `timetracker.lock`; if another process holds it,
   `forward_to_starting_instance` retries the socket 25 × 200 ms while that process starts up.
4. **`AppState`.** `AppState::open(&config_dir)` opens every JSON store and the data store
   (`DataStore::open`, blocking on Tauri's async runtime; a failure is logged and the app runs without a
   database, see [data.md](data.md#the-data-folder); a `dataDir` that is gone falls back to the default folder
   with a launch error toast); the CLI `--log-level` is kept in
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
   `deep_link::listen`, and shows the window (`window::show_main_in_background`) unless started with
   `--hidden`. The window is declared `"visible": false`, so a hidden start never flashes it.

   The run loop prevents `RunEvent::ExitRequested { code: None }`, which macOS sends when the last window
   closes; an explicit `app.exit(0)` carries a code and passes. On `RunEvent::Exit` it closes the database
   pool. On `RunEvent::Reopen` (a Dock icon click, or `open` on the running app) it calls
   `window::show_main`.

### Settings side effects

`update_settings` and startup both call `commands::apply_settings(app, previous, saved)`. The decision is the
pure `settings_effects(previous, saved, log_override) -> SettingsEffects { theme, log_level, launch_at_login }`:
without `previous` everything applies; otherwise only what changed. A CLI `--log-level` wins for the app's
lifetime, so a settings change never resets it. `launch_at_login` goes to `autostart::apply` (below), which
changes the LaunchAgent only when its state differs; at startup this reconciles a LaunchAgent that went
missing while the setting says on. A failure is logged.

### Launch at login

`autostart.rs` wraps `tauri-plugin-autostart` (`MacosLauncher::LaunchAgent`, started with `--hidden`; the
agent lives in `~/Library/LaunchAgents`). The LaunchAgent is the truth and `settings.launchAtLogin` mirrors it:
`status(app)` asks the plugin, `apply(app, enabled)` installs or removes the agent (`io` on failure), and
`set(app, state, enabled)` applies, then saves the setting through `commands::save_settings` and returns
`status`. The agent records the bundle path at enable time: after moving the app, turn it off and on again.

A login launch runs `timetracker --hidden`: the window stays invisible and the tray icon is the only sign the
app runs; a later launch or the tray shows it.

### Onboarding

`UiState.onboardingDone` (`ui_state.json`, default `false`) says the welcome sheet was finished.
`onboarding_done` sets it (`UiStateStore::finish_onboarding`); the page clears it with `set_ui_state` for
"Show welcome again".

## Window lifecycle

The main window is declared in `tauri.conf.json`: label `main`, 1200 × 800 (minimum 900 × 560), invisible
until setup shows it, overlay title bar with hidden title, traffic lights at 16/20, transparent background with
the `sidebar` vibrancy effect. The away prompt (`away_prompt/window.rs`) and the timer picker
(`timers/picker.rs`) are built in Rust with `WebviewWindowBuilder`.

| Event | Result |
|---|---|
| close button / ⌘W (`window.hide`, `hide_window`) | the window hides; the app, the tray and the Dock icon stay |
| last window closed (`ExitRequested { code: None }`) | prevented |
| tray "Open TimeTracker", a second `timetracker` launch, Dock icon click (`RunEvent::Reopen`) | `window::show_main`: show, unminimize, focus |
| `timetracker --hidden` (launch at login) | first instance: the window is never shown; running instance: nothing |
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
| `deleted_activity` | `activity::delete::DeleteBuffer` | the rows of the last activity delete, for its 30 s undo ([Activity deletion](#activity-deletion)) |
| `config_dir` | `PathBuf` | the config dir |
| `log_override` | `Option<LogLevel>` | the CLI `--log-level` |

The tracker is managed beside `AppState` as `Arc<Tracker>` (`tracker/mod.rs`), and only when the data store
opened. Its commands look it up with `try_state` and fail with `db` "The data store is not available"
otherwise. `update_settings` hands the new `tracker` and `capture` settings to `Tracker::set_settings` and
`Tracker::set_capture`, and keeps the stored `dataDir` (only `data_move` changes it, through
`SettingsStore::set_data_dir`). The tracker's `Db` sits behind an `RwLock`: a data folder move holds the
tracker with `Tracker::suspend` and hands it the new database with `Suspended::resume`.

## Tauri commands

Registered in `start_app` (`generate_handler!`). Commands stay thin; logic lives in its module.

| Command | Args | Returns | Errors |
|---|---|---|---|
| `get_settings` | – | `Settings` | – |
| `update_settings` | `settings: Settings` | `Settings` (sanitized) | `io` |
| `get_keybindings` / `set_keybindings` | – / `bindings: Keybindings` | `Keybindings` (sanitized; the menu is rebuilt) | `io` |
| `get_ui_state` / `set_ui_state` | – / `ui: UiState` | `UiState` | `io` |
| `onboarding_done` | – | – (sets `UiState.onboardingDone`) | `io` |
| `autostart_set` (`autostart.rs`) | `enabled: bool` | `bool`: whether the LaunchAgent is installed now; also saves `settings.launchAtLogin` | `io` |
| `autostart_status` (`autostart.rs`) | – | `bool`: whether the LaunchAgent is installed | – |
| `open_log_folder` | – | – | `io` |
| `open_url` | `url: String` | – | `invalid_input` unless `http(s)` without credentials (`is_web_url`) |
| `log_frontend_error` | `message: String` | – | – |
| `take_launch_events` | – | `LaunchEvent[]` | – |
| `remote_reply` | `id, result, error` | – | – |
| `hide_window` | – | – | – |
| `quit_app` | – | never returns | – |
| `choose_save_path` | `fileName, extension` | `string \| null` (the save panel; `null` when cancelled) | `io` |
| `choose_folder` | – | `string \| null` (the folder panel, which can create a folder; `null` when cancelled) | `io` |
| `reveal_path` | `path: String` | – (Finder shows the enclosing folder with `path` selected) | `io` |
| `data_info` (`db/commands.rs`) | – | `DataInfo { dir, file, sizeBytes, activities, recordings, projects, firstActivity, available, error, binaryPath }` (`binaryPath`: `~/.local/bin/timetracker` when that link exists, else this executable; for the `claude mcp add` line) | – (errors are reported inside) |
| `data_move` (`db/commands.rs`) | `path: String` | `DataInfo` of the new folder ([data.md](data.md#moving-the-data-folder)) | `invalid_input` (a refused folder), `io`, `db` |
| `backup_export` (`db/commands.rs`) | `path: String` (absolute) | `BackupWritten { bytes, recordings, activities }` | `db`, `io`, `invalid_input` |
| `report_run` (`reports/commands.rs`) | `filter: ReportFilter` | `ReportResult` (see [Reports](#reports)) | `invalid_input` (a range that ends before it starts), `db` |
| `report_text` | `filter: ReportFilter` | the report as text (`reports::text`) | the same |
| `saved_reports_list` | – | `SavedReport[] { id, name, filter, position }`, by position | `db` |
| `saved_report_save` | `name, filter` | `SavedReport` (appended) | `invalid_input` (`field: name`), `db` |
| `saved_report_rename` | `id, name` | `SavedReport` | `not_found`, `invalid_input` |
| `saved_report_delete` | `id` | – | `not_found` |
| `day_view` (`day/commands.rs`) | `day: String` | `DayView` (see [Day view](#day-view)) with the saved rules' `suggestions`; logs the time it took at debug level | `invalid_input`, `db` |
| `list_view` (`day/commands.rs`) | `day: String` | `ListView { day, rows: ListRow[], totalMs }` (see [List view](#list-view)) | `invalid_input`, `db` |
| `away_prompt_current` (`away_prompt/commands.rs`) | – | `AwayPrompt \| null` | – |
| `away_prompt_answer` | `input: RecordingInput` | `Recording`, created with `overlap: cancel` (undoable); closes the window | `not_found` (no prompt), `invalid_input` (`field`: `start` outside the last 7 days, or the recording's own fields), `overlap` (the prompt stays open) |
| `away_prompt_skip` | – | – (clears the prompt, closes the window; nothing is recorded) | – |
| `rules_list` (`rules/commands.rs`) | – | `Rule[] { id, name, projectId, position, enabled, note, tagIds, conditions }`, by position | `db` |
| `rule_save` | `id: string \| null, input: RuleInput` | `Rule`; `null` appends a new rule; an existing one keeps its position and gets the new conditions | `invalid_input` (`field`: `name`, `projectId`, `note`, `tagIds`, `conditions`, `conditions.<i>.value`), `not_found`, `db` |
| `rule_delete` | `id` | – (its conditions go with it) | `not_found` |
| `rules_reorder` | `ids: string[]` (every rule once) | `Rule[]` in the new order | `invalid_input` |
| `rule_set_enabled` | `id, enabled: bool` | `Rule` | `not_found` |
| `rule_preview` | `input: RuleInput, days: u32` | `RulePreview { hits: PreviewHit[] { block, day }, totalMs, days }`: what the conditions alone match over the last `days` days, today included (see [Rules and suggestions](#rules-and-suggestions)) | `invalid_input` (`field: conditions`), `db` |
| `suggestions` | `day: Day` | `Suggestion[]` (the same as in `DayView`) | `db` |
| `suggestion_accept` | `day: Day, id` | `Recording`, created like `recording_create` with `overlap: cancel` (undoable) | `not_found` (the suggestion is gone), `overlap`, `invalid_input`, `db` |
| `suggestions_accept_rule` | `day: Day, ruleId` | `Recording[]`: every suggestion of the rule on the day, one undo entry (`Recordings::create_all`; one that would overlap is left out) | `not_found` (no suggestions), `db` |
| `app_settings_set` (`commands.rs`) | `bundleId: String` (the app key: bundle id, else the app name), `patch: AppSettingPatch { groupByApp?, titles?, ownLane?, hidden? }` (`null` returns `groupByApp`/`titles` to the timeline's setting) | `Settings`; applied under the settings store's lock (`SettingsStore::update_app`), so concurrent changes never undo each other; an override that ends up default is removed (`Settings::with_app`) | `invalid_input` (blank key), `io` |
| `apps_list` (`day/commands.rs`) | – | `KnownApp[] { appKey, app, totalMs, lastSeen }` (`day::apps::known_apps`): every tracker app and every app with an override, by name | `db` |
| `recording_create` (`recordings/commands.rs`) | `input: RecordingInput { projectId, start, end, note, tagIds }, overlap: Overlap` | `Recording` (snapped) | `invalid_input` (`field`), `overlap` (`details`: the recordings in the way), `not_found`, `db` |
| `recording_update` | `id, patch: RecordingPatch, overlap` | `Recording` | the same |
| `recording_delete` | `id` | – | `not_found`, `db` |
| `recording_move` / `recording_copy` | `id, day: Day, overlap` | `Recording`: moved, or a new copy, to `day` at the same local time (`same_time_on`, DST-safe) with the same length, through `update` / `create` (undoable; a recording over midnight moves with its start day) | `invalid_input` (the running timer), `overlap`, `not_found`, `db` |
| `week_view` (`calendar/commands.rs`) | `day: Day` | `WeekView { weekStart, label, days: DayColumn[], totalMs, trackedMs }`; `DayColumn { day, bounds, label, isToday, recordings, totalMs, trackedMs }` | `invalid_input`, `db` |
| `month_view` | `day: Day` | `MonthView { monthStart, label, weeks: MonthDay[][], totalMs }` (whole weeks Monday to Sunday; `totalMs` counts the month's own days); `MonthDay { day, inMonth, isToday, recordedMs, projects }` (the top three by time, with color and shade) | `invalid_input`, `db` |
| `recordings_list` | `from: Ms, to: Ms` | `Recording[]` sharing time with the range | `db` |
| `recording_snap` | `start: Ms, end: Ms` | `Span` as a write would snap it (the drag ghost) | – |
| `undo` / `redo` | – | the label undone or redone, `null` when there was none | `overlap`, `db` |
| `undo_status` | – | `UndoStatus { undoLabel, redoLabel }` | – |
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
| `activity_count` | `from: Ms, to: Ms` | `ActivityCount { activities, away }`: the spans `activity_delete` would remove | `invalid_input` (from ≥ to, over a year), `db` |
| `activity_delete` | `from: Ms, to: Ms` | `DeleteResult { activities, away, undoUntil }` | `invalid_input` (from ≥ to, over a year), `db` |
| `activity_delete_undo` | – | `DeleteResult` of the restored rows | `not_found` "Nothing to undo", `db` |

To add a command: write it in its module (or `commands.rs` for app-wide ones), register it in
`generate_handler!`, export its types with ts-rs, add the wrapper to `src/api/index.ts` and a default to
`src/test/apiMock.ts`.

## Activity deletion

`activity/delete.rs`, called through `activity::{count, delete_range, undo_delete}` (which emit the events)
by the commands and the CLI. Activity is never edited (ADR 0005), so deletion works on whole rows:

- `delete(db, buffer, tracker, from, to, now)` removes, in one transaction, every `activities` and
  `away_spans` row with `start < to AND (end IS NULL OR end > from)` (`activities::take_range_on`,
  `away::take_range_on`, `DELETE … RETURNING`). Rows crossing the bounds go whole; recordings are untouched.
  The range must be non-empty and at most 366 days (`invalid_input`). When the tracker's open span was
  deleted, `Tracker::reset_span(ids, now)` forgets it, so the next poll opens a new one.
- The rows go into `AppState.deleted_activity` (`DeleteBuffer`, one `DeletedActivity`; a new delete replaces
  it). `DeleteResult.undo_until` is `now + UNDO_WINDOW_MS` (30 s).
- `undo(db, buffer, now)` re-inserts the rows under their original ids (`restore_on`; ids are never reused,
  `AUTOINCREMENT`) in one transaction and clears the buffer. Rows that were open come back closed at the
  time of the delete, since the tracker has opened a new span since. After the window, or with nothing
  deleted, it is `not_found` "Nothing to undo"; a failed undo keeps the buffer.
- Both emit `data_changed { kind: activity, day }` for every local day from the earliest start to the latest
  end of the range and its rows. `count(db, from, to)` is the dialog's preview (`ActivityCount`).

## Day view

`day/mod.rs`: `view(&DayInput, &ViewSettings) -> DayView` is a pure function ([ADR
0012](adr/0012-timeline-layout-in-rust.md)); `day::commands::load` reads the rows (`list_for_range`, spans
from the day before included) and calls it. `ViewSettings` is a copy of `Settings.timeline` and
`Settings.apps`. In order:

1. `clipped_blocks`: every activity cut to the day's bounds (an open span ends at `now`); private rows keep
   the app and read "Private" without URL or path. `appKey` is the bundle id, else the app name: what
   `Settings.apps` overrides are keyed by.
2. Totals, gaps, `firstActivity` and `sources` come from these raw rows, so hiding and merging never change
   them: `trackedMs` the union of tracker spans, `awayMs` the union of away spans, `recordedMs` the sum of
   the recordings' parts in the day; gaps are holes of `GAP_MIN` (2 min) or longer between the first
   activity's start and the last one's end, without activity or away.
3. External rows go to their source lanes as they are. Tracker rows run through `day/transform.rs`:
   `without_hidden` drops apps with `hidden`; `identified` labels each row (`titles`: the title, or the app
   when empty or in `app` mode; private rows "Private") and keys it by app (`groupByApp`) or by app and
   label, a per-app override winning over `timeline`; private rows get a key of their own.
4. `lanes::summary` (with `summaryLane`): the day in intervals of `summaryMinutes` from its start; each goes
   to the key with the most time in it (ties: the earlier start); a run of equal winners is one block from
   the winner's first start to its last end inside the run, id `s<start>`.
5. `transform::grouped`: rows of one key that touch, or whose gap is shorter than the continuity (off 0,
   short 2, regular 8, extended 16 min), become one block unless a sleep, lock or pause lies between
   (`breaks_continuity`; idle is bridged). Other keys' rows in the gap stay their own blocks, beside it.
   Blocks shorter than the noise threshold (off 0, low 2, medium 4, high 6 min) are dropped. A block shows
   its longest row (`title`, `label`, URL, path); its id is `a<first member id>`, `members` every row.
6. `lanes::lanes`: the `activity` lane (unless `showAllActivity` is off), one `app:<appKey>` lane per app
   with `ownLane` in the order of first appearance, one `source:<name>` lane per external source by name;
   `packed` per lane: by start, longer first, each block in the lowest free column, a cluster of overlapping
   blocks sharing its width (`col` of `cols`).
7. `mark_covered`: a block of any lane is `covered` when at least half of it lies inside recordings.

`DayView` also carries `compact` (from `timeline`) and `hiddenApps` (every hidden override, named from the
day's rows, else by its key) for the header's popover.

## Recordings

`recordings/` turns spans into recordings ([ADR 0010](adr/0010-recordings-never-overlap-ops-with-undo.md)).
`Recordings { db, undo, snap_minutes }` (`mod.rs`) is the service; each write takes the `AppState.undo` lock
for its whole run, so writes never interleave. `create`, `update` and `delete` validate, snap, clear the span,
build an `ops::Op`, apply it and push it on the undo stack; `commands.rs` then emits `data_changed { kind:
recordings, day }` for every day the op touched, before and after (`Change.days`).

- **Snapping** (`snap`, `snap_span`): to the nearest multiple of `timeline.snapMinutes` counted from local
  midnight, read at call time; an end that snaps onto or before the start lands one step after it. `update`
  snaps only the times it changes, so editing a note keeps the times.
- **Validation** (`InvalidField`): `end` after `start` and at most 24 h later; `note` one line, at most 500
  characters, trimmed; `projectId` not archived (unless the recording already belongs to it); `tagIds` known.
  A missing project is `not_found`; a running recording is not edited here ("Stop the timer first").
- **Overlap** (`Overlap::Cancel | Replace`): with `Cancel` a span sharing time with others is refused with
  `overlap` and the recordings in `details`; with `Replace` each one in the way is deleted (inside), trimmed
  (one side out) or split (both sides out; the right part is a new recording with the same project, note and
  tags), all in one `Op::Batch` with the new recording last. A running recording in the way is
  `invalid_input`.
- **Operations** (`ops.rs`): `Op::Create | Update { before, after } | Delete | Batch`; `inverse()` swaps them
  (a batch reversed). `apply` runs the op in one transaction through `db::recordings::put_on` (the row
  exactly as given, timestamps and tags included) and `remove_on`, then refuses with `overlap` if any
  recording it wrote shares time with another; then nothing is written.
- **Undo** (`undo.rs`): `UndoStack` keeps the last 100 changes of this run with their labels (`Record 45 min
  for Acme`, `Move recording`, `Resize recording`, `Edit recording`, `Delete recording`); a new change clears
  the redo list. `undo` applies the inverse, `redo` the op again; one that would now overlap is refused and
  the stack stays as it was.

The Day view gets the day's recordings too (`day::commands::load` reads them with the projects, clients and
tags): `DayView.recordings` (`RecordingBlock`: clipped span, project, client, color and shade, note, tag
names, `running`, `clippedStart`/`clippedEnd`), `ActivityBlock.covered` when at least half of a block lies
inside recordings, and `totals.recordedMs`.

## Week and month

`calendar/mod.rs` reads the range's recordings (and, for the week, activity) once and cuts them per day with
the Day view's own `day::recording_blocks` and `day::tracked_ms`, so a recording over midnight shows in both
days' columns with its parts.

## Reports

`reports/mod.rs`: `run(db, filter, today, now)` resolves the range (`resolve`: today, yesterday, this or
last week from Monday, this or last month, or custom days), reads the recordings sharing time with it and
calls the pure `build`. A `ReportFilter` reuses the settings' `Group` and `Rounding`, so the Reports screen
starts from `settings.reports`.

- Rows: each recording's part in each day of the range (the running one ends now), so a recording over
  midnight counts on both days, as in the Day view. Client, project and tag lists narrow
  together (empty: all; a tag filter keeps recordings with any chosen tag); archived projects only with
  `includeArchived`. Notes are trimmed.
- Rounding is per recording (`rounding_step`: nearest, half up, or up), at least one step for a recording
  longer than zero; totals sum the rounded rows, each recording once.
- Groups: by project (`Client / Project`, ordered by client then project, client-less last, with color and
  shade), by tag (by name, "No tag" last; a recording with several tags counts in each, which sets
  `tagOverlap`; a tag filter limits the groups to the chosen tags), by day (`Mon 2026-10-20`), week
  (`Week 43 · 20 – 26 Oct 2026`) or month (`October 2026`). A group lists its distinct notes in the order
  they first occur, each with its days, and its rows by start.
- `reports/text.rs` renders the text the screen copies and the CLI prints (format in [ux.md](ux.md#reports)).
- `reports/saved.rs`: saved reports in `saved_reports` (the filter as JSON; a filter this version can't read
  opens as the default). Presets stay relative, so a saved "this month" follows the calendar.

## List view

`day/list.rs`: `list(&DayView) -> ListView` groups the blocks of every lane but the summary by source, app
key and label (the timeline's grouping, titles, hidden apps and noise already applied, so with titles set to
the app a whole app is one row). A row has the merged spans of its blocks (touching ones joined), their total, the time between them (`gapMs`),
the part inside recordings (`coveredMs`), the first start and last end, and the longest block's app, title,
URL and path. Rows sort by total, longest first, then by first start. `list_view` builds the view without
suggestions (`day::commands::view_of`).

## Away prompt

`away_prompt/`: after a long away span the `away` window asks what the user did.

- `Tracker::subscribe_away` broadcasts each away span `leave_away` closes because the user is back (input
  after idle, wake, unlock, resume). Spans closed at startup (`recover`) or by switching to another away
  reason are not sent, so a prompt only follows a live return.
- `watch` (started with the tracker) checks `should_prompt`: the setting is on, the span is closed, at least
  `tracker.awayPromptMinutes` long, and not a pause (the user chose it). It stores the `AwayPrompt { span,
  suggested }` (`suggestion`: both ends snapped, at least one snap step) in `AwayPromptState`, replacing an
  open one, sends `away_prompt_changed` to an open window, and opens the window.
- `window.rs`: label `away`, `src/windows/away/index.html`, 420×380, no decorations, always on top, centered,
  built on the main thread (`run_on_main_thread`, since `watch` runs on a tokio task). It stays open when it
  loses focus; closing it clears the prompt. Capability `capabilities/away.json`.
- `answer` records through the recordings service with `overlap: cancel`; an overlap keeps the prompt so the
  times can change; success clears it.

## Rules and suggestions

`rules/`: a rule says "activity like this belongs to project X"; it never writes a recording. Its suggestions
fill `DayView.suggestions`, and accepting one is an ordinary undoable create.

- `store.rs`: `rules` and `rule_conditions` (migration `0005_rules.sql`), in position order. `validated`
  checks the name, note, project (existing, not archived), tags and conditions (at least one; a value
  unless the op is `is`). `active` is what the Day view uses: enabled rules whose project isn't archived.
- `matching.rs`: `compile` turns enabled rules into `Compiled` matchers once per view; `matches` holds when
  every condition does (case-insensitive; `contains`, `is`, `begins_with`, or `pattern`: a glob with `*` and
  `?` anchored at both ends, no regex). Fields read the block's app, title, URL, domain (the URL's host
  without `www.`), path or source; an empty value only matches `is ""`; a private block matches no rule.
  `first_match` takes the first rule by position.
- `suggest.rs`: every block of the activity, app and source lanes (after hiding, grouping, continuity and
  noise; not the summary lane) goes to its first matching rule. A rule's blocks join into runs while the gap
  to the next is shorter than the continuity threshold and no sleep, lock or pause lies in it. Matched time
  belongs to its rule first; a run's bridged gaps only take time no rule matched (earlier rules first), so
  suggestions never overlap. Recorded time is cut away; pieces under `MIN_SUGGESTION_MS` (5 min) are
  dropped. Ids are `<rule id>:<start>:<end>`, so a suggestion keeps its id across reloads; `accept` and
  `accept_rule` (`rules/mod.rs`) rebuild the day's suggestions and find it again. Tags deleted since the
  rule was saved are left out.
- `preview` builds the views of the last days (`day::commands::view_of`, without suggestions) and lists the
  blocks the input's conditions match on their own, most recent first (`PREVIEW_HITS`, 200), with the time
  of every hit and the number of days with one.
- Every rule write emits `data_changed { kind: rules, day: null }`, so open views rebuild their
  suggestions. Backups include the rules.

## Events to the page

`events.rs` names every event and its payload, so all emitting goes through one place.

| Constant | Name | Payload |
|---|---|---|
| `LAUNCH_EVENT` | `launch` | none; the page calls `take_launch_events` |
| `COMMAND_EVENT` | `command` | a command id (menu or tray) |
| `REMOTE_EVENT` | `remote` | `RemoteEvent { id, request }`, answered with `remote_reply` |
| `DATA_CHANGED_EVENT` | `data_changed` | `DataChanged { kind, day }` |
| `TRACKER_STATUS_EVENT` | `tracker_status` | `TrackerStatus` on every change and at least every 10 s ([capture.md](capture.md#status)) |
| `TIMER_STATUS_EVENT` | `timer_status` | `TimerStatus` at start, on every change and every minute while a timer runs |
| `AWAY_PROMPT_CHANGED_EVENT` | `away_prompt_changed` | `AwayPrompt`, only to the `away` window, when a new span replaces the open prompt's |

`DataKind` is `activity | recordings | projects | tags | rules | settings`; `day` is a `Day` (`YYYY-MM-DD`)
or `null` when every day may have changed. `events::data_changed(app, kind, day)` emits it; external
activity writes emit one per local day the span touches (`activity::external::days_touched`). The page bumps
that day's version (or every day's for `null`), so views showing the day reload
([frontend.md](frontend.md)).

## CLI and single-instance handoff

The `timetracker` command is the app binary. Without a subcommand it starts the app, or activates the
running one; every subcommand is a second process that sends one request over the socket, prints the reply
and exits ([ADR 0017](adr/0017-cli-asks-the-running-app.md)). It never opens the database itself; only
`timetracker mcp` does, read-only ([MCP server](#mcp-server)).

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
| `activity delete DAY` / `activity delete FROM TO` | `activityDelete { from, to }`: `DAY` (`YYYY-MM-DD`, `today`, `yesterday`) is its local bounds; `FROM`/`TO` as `instant` | `deleted 412 activity spans and 3 away spans` and `undo within 30 s: timetracker activity undo` |
| `activity undo` | `activityDeleteUndo` | `restored 412 activity spans and 3 away spans`; nothing to undo exits 1 with `Nothing to undo` |
| `report [today\|yesterday\|week\|last-week\|month\|last-month\|DAY\|FROM TO] [--by project\|tag\|day\|week\|month] [--client C]… [--project P]… [--tag T]… [--round 5\|6\|10\|15\|30] [--up]` | `report { request: ReportRequest }` (`cli/report.rs` parses the range and flags); the app resolves names (`reports::cli`: an id, an exact name ignoring case, or a unique start; several matches list them) and runs the report | the report text; `--json`: the `ReportResult` plus `text` |
| `start PROJECT [--note N] [--tag T]…` | `timerStart { request: TimerStartRequest }`; the app resolves the project and tags like `record` | `started Acme at 09:15`; `--json`: the `Recording` plus `projectName` |
| `stop` | `timerStop` | `stopped 09:15–10:40 (1:25)` or `no timer running` (exit 0) |
| `mcp` | none: serves MCP on stdin/stdout (`CliOutcome::Mcp`) | only JSON-RPC on stdout |
| `record --project P --from ISO --to ISO [--note N] [--tag T]… [--replace]` | `record { request: RecordRequest }` (`overlap: replace` with `--replace`, else `cancel`); the app resolves the project (`projects::resolve`) and tags by name (`tags::resolve_names`, unknown → `not_found`) | `recorded 09:00–10:30 (1:30) for Acme · fix login`; an overlap exits 1 with `overlaps 09:15–09:45 Acme · standup (use --replace)`. `--json`: the `Recording` plus `projectName` |

`parse(args, now, tz, stdin) -> Result<CliOutcome, CliError>` is pure: `CliOutcome` is `Run { hidden,
log_level }`, `Exit { message }` (help, version), `Mcp` or `Remote { message, json, starts_app }`. The CLI checks
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
  messages above, `eval { js }` (debug builds only, sent by `scripts/dev/js.sh`) and `devSeed { years, yes }`
  (debug builds only, sent by `timetracker dev seed`; its sender waits up to 120 s instead of 5 s,
  `IpcMessage::reply_timeout`).
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
| `activityDelete { from, to }` | `activity::delete_range` | `DeleteResult` |
| `activityDeleteUndo` | `activity::undo_delete` | `DeleteResult` |
| `eval { js }` | runs in the window | `null` |
| `devSeed { years, yes }` | `dev::seed`: refused unless `yes` when activity before today exists, then `test_support::seed::seed_years` (seed 42) and `data_changed` for activity, recordings, projects and tags | `SeedStats` |

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

## MCP server

`timetracker mcp` (`mcp/`) is a read-only [MCP](https://modelcontextprotocol.io) server on stdio, built on
`rmcp`, so Claude and other MCP clients can query recordings and reports. It is the one subcommand that opens
the database itself, so it works while the app is closed; stdout carries only JSON-RPC, errors go to stderr,
nothing is logged. stdio is not the network ([ADR 0013](adr/0013-no-network.md) holds).

- `mcp/mod.rs`: `TimeTrackerServer` (`#[tool_router]`, `#[tool_handler]`, args as `Parameters<…Args>`
  deriving `JsonSchema`), `INSTRUCTIONS`, and `run(config_dir, socket) -> i32`, which builds its own
  current-thread tokio runtime. Exit codes: 0 when the client disconnects, 1 on a handshake or transport
  error, 2 when the runtime cannot start.
- `mcp/data.rs`: `Core`, the headless data access. The database opens on first use with
  `Db::open_read_only` (path from `Settings::peek` and `db::data_dir`), so the handshake succeeds on a Mac
  that never ran the app, and a failed open is tried again on the next call. Settings are read on every call.
- `mcp/ui.rs`: `ask(socket, message)` sends an `IpcMessage` like the CLI; no listener answers
  `NOT_RUNNING` (`TimeTracker is not running`).

| Tool | Args | Returns |
|---|---|---|
| `report` | `range` (`today` default, `yesterday`, `week`, `last-week`, `month`, `last-month`, a day) or `from`/`to`; `group_by` (`project` default, `tag`, `day`, `week`, `month`); `client`, `project`, `tag`; `format` (`text` default, `json`) | `reports::text`, or `{ from, to, group_by, rounding, total_min, total_rounded_min, tag_overlap, groups: [{ label, minutes, rounded_min, notes }] }`; rounding from `settings.reports.rounding` |
| `recordings` | `from`, `to` (days, inclusive), `project`, `tag`, `limit` (default 500, at most 5000) | `[{ id, day, start_iso, end_iso, duration_min, project, client?, note, tags }]` by start, one row per recording and day, archived projects included |
| `projects` | – | `[{ id, name, color, client?, archived }]` |
| `tags` | – | `[{ id, name }]` |
| `day_summary` | `day` | `{ day, recorded_min, tracked_min, away_min, by_project, by_tag }`; totals from `day::commands::view_of`, the groups `[{ name, minutes, notes }]` from `reports::run` for that day |
| `activity` | `day` | `{ activity: [{ start_iso, end_iso, app, title, url?, path?, source }], away: [{ start_iso, end_iso, reason }] }`; private rows keep only the app. Refused (`Activity is not exposed to MCP. Enable it in Settings → Data.`) unless `settings.mcp.exposeActivity` |
| `status` | – | `IpcMessage::Status` to the app: `{ version, tracker: { state, text, current: { app, title?, url?, since_iso } }, timer: { project, note, start_iso, elapsed_min }, window_visible }`; the current title and URL only when activity is exposed |
| `open_day` | `day` | `IpcMessage::Show { day, focus: true }` to the app |

Days are `YYYY-MM-DD`, `today` or `yesterday` (`cli::time::day`); ranges parse with `cli::report::range`, the
CLI's parser. `client`, `project` and `tag` resolve like `timetracker report` (`reports::cli::filter_of`: an
id, an exact name ignoring case, or a unique start; several matches are an error naming them). Times are
local ISO 8601 with offset (`db::time::to_local_iso`), durations minutes to one decimal. Results are pretty
JSON; an `AppError` becomes a tool error with its message.

**Adding a tool:** a `#[tool]` method on `TimeTrackerServer` with a description, its args struct, a `Core`
method returning an MCP-only output struct (no `TS`), and a test over the JSON-RPC duplex
([testing.md](testing.md#rust)).

## Tray

`tray.rs`: `tray::install(app)` builds the menu bar item `main` as a template image (macOS tints it for the
menu bar), tooltip `TimeTracker`, menu on left click:

| Item | Id | Action |
|---|---|---|
| the tracker's status text (disabled) | `status` | relabelled on every status |
| Pause Tracking ▸ For 15 Minutes, For 1 Hour, Until Tomorrow, Until Resumed | `pause-15`, `pause-60`, `pause-tomorrow`, `pause-resumed` | `Tracker::pause` (hidden while paused) |
| Resume Tracking | `resume` | `Tracker::resume` (only while paused) |
| Start Timer… | `timer.start` | `timers::picker::open` |
| Stop Timer (Acme, 1:23) | `timer.stop` | `timers::commands::stop` (disabled without a timer) |
| Recent Timers ▸ `Project · note` | `timer.recent.<n>` | starts that combination |
| Open TimeTracker | `open` | `window::show_main` |
| Quit TimeTracker | `quit` | `app.exit(0)` |

`tracker::start` passes every tracker status to `tray::sync(app, &status)`. The icon
follows `TrayIcon::of(state)`: `icons/tray/tray@2x.png` (running or away), `paused@2x.png` (a pause sign in
the clock) and `attention@2x.png` (an exclamation mark: the permission is missing). `sync` only relabels the
status item while the icon stays the same, and rebuilds menu and icon when it changes, so an open menu is not
replaced every few seconds. `tray::set_timer` shows a running timer as the item's title, `⏱ 1:23 Acme` (the
project cut to 24 characters), and rebuilds the timer items. The tray's click events go to
`tauri_plugin_positioner::on_tray_event`, so the picker can open under the icon.

## Timers

A timer is a recording without an end, so the overlap rule, undo and reports need nothing new.

- `recordings/timer.rs`: `Recordings::start_timer(TimerInput, now)` starts at `snap(now)`, moved forward past
  any recording that ended since (`free_start`); a running timer stops at that same instant, in one batch
  ("Start timer for Acme"). `stop_timer(now)` ends it at `stop_time(start, now, snap)`: on the grid, and at
  least a minute long. Both go through `apply`'s overlap check and onto the undo stack; undoing a stop makes
  the timer run again.
- `timers/mod.rs`: `status(db, now) -> TimerStatus { running: Option<RunningTimer { recording, projectName,
  color, shade, elapsedMs }> }`, `recent(db, 8) -> RecentTimer[]` (distinct project, note and tags of finished
  recordings, newest first, archived projects left out), `publish(app)` (the `timer_status` event and the tray),
  `start_ticking(app)` (publishes at startup and every minute while a timer runs).
- `timers/commands.rs`: `timer_start(projectId, note, tagIds)`, `timer_stop` (`null` without a timer),
  `timer_status`, `timers_recent`, `timer_picker_open`, `timer_picker_close`; `start_by_name` serves the CLI.
- `timers/picker.rs`: the `timer` window (`src/windows/timer/index.html`, 380×320, no decorations, transparent,
  always on top, hidden on blur), under the tray icon or centered; capability `capabilities/timer.json`.
- `timers/shortcut.rs`: `register(app, binding)` makes `Settings.timerShortcut` (default `Mod+Shift+T`) open
  or close the picker from any app (`tauri-plugin-global-shortcut`). Only a chord with a Tauri key name
  (`keybindings::is_global`) qualifies; `sanitized` resets others. `update_settings` registers a changed
  shortcut before saving it and returns `invalid_input` when the system refuses.
- `status` (CLI, `StatusReport.timer`) prints `timer: Acme · standup · 0:25`.

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
| `settings.json` | `Settings` (camelCase, `#[serde(default)]`; nested `tracker`, `capture`, `timeline`, `apps`, `reports`, `mcp` (`McpSettings { exposeActivity: false }`) each `#[serde(default)]`). Enum, free-text and nested fields use `deserialize_with = "lenient"`: an unknown value falls back to that field's default instead of quarantining the file. `sanitized()` clamps on load and on update: `uiFontSize` 10..=24, `dataFontSize` 9..=28, `tracker.idleMinutes` 1..=120, `tracker.awayPromptMinutes` 1..=240, `tracker.pollSeconds` 1..=10, `timeline.zoom` 0.5..=4.0 (non-finite: 1.0); snaps `timeline.summaryMinutes` to `SUMMARY_MINUTES` and `timeline.snapMinutes` to `SNAP_MINUTES` (nearest, smaller on a tie); resets unknown accents (`ACCENTS`) and schemes (`SCHEMES`) to `orange`, `ayu-light`, `ayu-mirage`; turns blank `fontUi`, `fontMono` and `dataDir` into `null`; a blank `timerShortcut` becomes `Mod+Shift+T`. `SettingsStore::update` returns `(previous, saved)` from one lock. A save that changes `timeline` or `apps` emits `data_changed { kind: settings, day: null }`, so every open day reloads. `apps` maps an app key to its `AppOverride { groupByApp?, titles?, hidden, ownLane }`. |
| `keybindings.json` | `{ "commandId": "Mod+Shift+X" \| null }` overrides; `null` unbinds. `keybindings::sanitize` drops (and logs) entries whose id is not `[A-Za-z0-9.]+` or whose binding is malformed. A trailing `++` is the `+` key |
| `ui_state.json` | `UiState`: `sidebarHidden`, `sidebarWidth` (px, default 200), `lastDay`, `timelineZoom` (the Day view's px per minute, default 2.5), `paneSizes` (px by pane id), `dayMode`, `onboardingDone` (default `false`: the welcome sheet shows) |
| `data/timetracker.sqlite` (+ `-wal`, `-shm`) | clients, projects, tags, activity, away spans, recordings ([data.md](data.md)); `dataDir` in `settings.json` points elsewhere (written only by `data_move`) |
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
| `tauri-plugin-global-shortcut` | the system-wide timer shortcut |
| `tauri-plugin-positioner` | the timer picker under the tray icon |
| `tokio` | runtime, IPC socket, the tracker loop |
| `rmcp` (`server`, `transport-io`, `macros`, `schemars`) | `timetracker mcp` |
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
