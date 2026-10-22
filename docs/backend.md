# Backend (Rust core)

This page covers `src-tauri/`, the Rust side of TimeTracker.

## Startup

`main.rs` calls `timetracker_lib::run()` (`lib.rs`):

1. **Config dir.** `config_dir()` resolves `~/Library/Application Support/<IDENTIFIER>` (no home dir: exit 2)
   and `create_private_dir` (`store/json_file.rs`) makes it mode 0700. This happens before Tauri starts,
   because the CLI and the socket need the path first.
2. **CLI.** `cli::parse(std::env::args_os())` returns:
   - `Exit { message }`: `--help` or `--version`; printed, exit 0.
   - `Run { hidden, log_level }`: becomes `IpcMessage::Activate { hidden }`.
   - `Mcp`: `timetracker mcp` exits with `mcp::run(&config_dir, socket)` right here, before the handoff, the
     instance lock and Tauri, so it runs beside the app ([MCP server](#mcp-server)).
   - an error: printed to stderr and turned into `IpcMessage::Error`; the exit code after forwarding is 2,
     and a newly started app shows it.
3. **Single-instance handoff.** `instance::forward_or_lock` (`instance.rs`) sends the message with
   `ipc::try_forward`. `Delivered` exits; `NoInstance` continues. Then it takes `timetracker.lock`; if
   another process holds it, it retries the socket 25 × 200 ms while that process starts up. A subcommand
   that talks to the app asks it first with `instance::ask`, which prints the reply.
4. **`AppState`.** `AppState::open(&config_dir)` opens every JSON store and the data store
   (`DataStore::open`, blocking on Tauri's async runtime; a failure is logged and the app runs without a
   database, see [data.md](data.md#the-data-folder); a `dataDir` that is gone falls back to the default folder
   with a launch error toast). The log level is the CLI `--log-level`, else Info, so logging never starts at
   `Trace`.
5. **`app::start`** (`app.rs`) builds Tauri with `.manage(state)` first, then `remote::Bridge`:
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
pure `settings_effects(previous, saved) -> SettingsEffects { launch_at_login }`:
without `previous` everything applies; otherwise only what changed. `launch_at_login` goes to `autostart::apply` (below), which
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
until setup shows it, overlay title bar with hidden title, traffic lights at 20/28 (centered in the 52px toolbar), transparent background with
the `sidebar` vibrancy effect. The away prompt (`away_prompt/window.rs`) and the timer picker
(`timers/picker.rs`) are built in Rust with `WebviewWindowBuilder`.

| Event | Result |
|---|---|
| close button / ⌘W (`window.hide`, `hide_window`) | the window hides; the app, the tray and the Dock icon stay |
| last window closed (`ExitRequested { code: None }`) | prevented |
| tray "Open TimeTracker", a second `timetracker` launch, Dock icon click (`RunEvent::Reopen`) | `window::show_main`: show, unminimize, focus |
| `timetracker --hidden` (launch at login) | first instance: the window is never shown; running instance: nothing |
| ⌘Q (`app.quit`), tray "Quit TimeTracker", `quit_app` | `app.exit(0)` |

`window.rs` never sets the window theme, so its native appearance follows macOS (the page draws every surface
opaquely). It also has `bring_to_front`, which does nothing with `TIMETRACKER_BACKGROUND=1` in debug builds.

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

The tracker is managed beside `AppState` as `Arc<Tracker>` (`tracker/mod.rs`), and only when the data store
opened. Its commands look it up with `try_state` and fail with `db` "The data store is not available"
otherwise. `update_settings` hands the new `tracker` and `capture` settings to `Tracker::set_settings` and
`Tracker::set_capture`, and keeps the stored `dataDir` (only `data_move` changes it, through
`SettingsStore::set_data_dir`). The tracker's `Db` sits behind an `RwLock`: a data folder move holds the
tracker with `Tracker::suspend` and hands it the new database with `Suspended::resume`.

`app_icons::AppIcons` is managed beside it as `Arc<AppIcons>` ([App icons](#app-icons)).

## Tauri commands

Registered in `handlers.rs` (`handlers()`, one `generate_handler!` list). Commands stay thin; logic lives in its module.

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
| `take_launch_events` | – | `LaunchEvent[]` (`error`, `notice`, `activate`, `showDay`) | – |
| `remote_reply` | `id, result, error` | – | – |
| `hide_window` | – | – | – |
| `quit_app` | – | never returns | – |
| `choose_save_path` | `fileName, extension` | `string \| null` (the save panel; `null` when cancelled) | `io` |
| `choose_folder` | – | `string \| null` (the folder panel, which can create a folder; `null` when cancelled) | `io` |
| `reveal_path` | `path: String` | – (Finder shows the enclosing folder with `path` selected) | `io` |
| `open_readme` (`help.rs`) | – | – (opens the README, see [Native menu](#native-menu)) | `not_found` (none in this build), `io` |
| `menu_sync` (`menu/sync.rs`) | `state: MenuState { enabled, checked }` | – (greys out and checks the menu items) | – |
| `data_info` (`db/commands.rs`) | – | `DataInfo { dir, file, sizeBytes, activities, recordings, projects, firstActivity, firstActivityLabel, available, error, binaryPath }` (`binaryPath`: `~/.local/bin/timetracker` when that link exists, else this executable; for the `claude mcp add` line) | – (errors are reported inside) |
| `data_move` (`db/commands.rs`) | `path: String` | `DataInfo` of the new folder ([data.md](data.md#moving-the-data-folder)) | `invalid_input` (a refused folder), `io`, `db` |
| `backup_export` (`db/commands.rs`) | `path: String` (absolute) | `BackupWritten { bytes, recordings, activities }` | `db`, `io`, `invalid_input` |
| `report_run` (`reports/commands.rs`) | `filter: ReportFilter` | `ReportResult` (see [Reports](#reports)) | `invalid_input` (a range that ends before it starts), `db` |
| `report_text` | `filter: ReportFilter` | the report as text (`reports::text`) | the same |
| `period_totals` | `range: Range` | `PeriodTotals { label, recordedMs, billableMs, billablePercent }` (`reports/period.rs`): every recording of the range, archived projects included, no other filter (`reports::billable`); `billablePercent` is `null` when nothing is recorded. `label` (`period_label`) names the days: Today, Yesterday, This week, Last week, This month, Last month, else `Mon, Oct 19`, `Week 43`, `October` or `Oct 1 – 14` (`titles::range_label`, in the region's date order), with the year when it is not the current one. The status bar's numbers for the shown period | `invalid_input`, `db` |
| `report_range_shift` | `range: Range, anchor: Day \| null, by: i32` | `Range`: the period of the range's unit `by` steps away (`reports::shift`, see [Reports](#reports)); P, N, O and the date picker in Reports | `invalid_input` |
| `saved_reports_list` | – | `SavedReport[] { id, name, filter, position }`, by position | `db` |
| `saved_report_save` | `name, filter` | `SavedReport` (appended) | `invalid_input` (`field: name`), `db` |
| `saved_report_rename` | `id, name` | `SavedReport` | `not_found`, `invalid_input` |
| `saved_report_delete` | `id` | – | `not_found` |
| `day_view` (`day/commands.rs`) | `day: String` | `DayView` (see [Day view](#day-view)) with the saved rules' `suggestions` and `label`, the toolbar title in the `dayTitle` format; logs the time it took at debug level | `invalid_input`, `db` |
| `day_totals` (`day/commands.rs`) | `day: Day` | `DayTotals { recordedMs, trackedMs, awayMs }` (`day::totals`, without building the view) | `invalid_input`, `db` |
| `away_prompt_current` (`away_prompt/commands.rs`) | – | `AwayPrompt \| null` | – |
| `away_prompt_answer` | `input: RecordingInput` | `Recording`, created with `overlap: cancel` (undoable); closes the window | `not_found` (no prompt), `invalid_input` (`field`: `start` when a span lies outside the last 7 days, or the recording's own fields), `overlap` (the prompt stays open) |
| `away_prompt_skip` | – | – (clears the prompt, closes the window; nothing is recorded) | – |
| `rules_list` (`rules/commands.rs`) | – | `Rule[] { id, name, projectId, position, enabled, note, tagId, conditions }`, by position | `db` |
| `rule_save` | `id: string \| null, input: RuleInput` | `Rule`; `null` appends a new rule; an existing one keeps its position and gets the new conditions | `invalid_input` (`field`: `name`, `projectId`, `note`, `tagId`, `conditions`, `conditions.<i>.value`), `not_found`, `db` |
| `rule_delete` | `id` | – (its conditions go with it) | `not_found` |
| `rules_reorder` | `ids: string[]` (every rule once) | `Rule[]` in the new order | `invalid_input` |
| `rule_set_enabled` | `id, enabled: bool` | `Rule` | `not_found` |
| `rule_preview` | `input: RuleInput, days: u32` | `RulePreview { hits: PreviewHit[] { block, day }, totalMs, days }`: what the conditions alone match over the last `days` days, today included (see [Rules and suggestions](#rules-and-suggestions)) | `invalid_input` (`field: conditions`), `db` |
| `suggestions` | `day: Day` | `Suggestion[]` (the same as in `DayView`) | `db` |
| `suggestion_approve` | `day: Day, id` | `SuggestionApproved { recording, durationMs }`: the recording, created like `recording_create` with `overlap: cancel` (undoable), and its length for the toast | `not_found` (the suggestion is gone), `overlap`, `invalid_input`, `db` |
| `suggestion_approve_rule` | `day: Day, ruleId` | `Recording[]`: every suggestion of the rule on the day, one undo entry `Approve suggestion` / `Approve n suggestions` (`Recordings::create_all`; one that would overlap is left out) | `not_found` (no suggestions), `db` |
| `suggestions_approve` | `day: Day, ids: string[]` | `Recording[]`: the suggestions `ids` still on the day (the shown ones; others are not touched), one undo entry `Approve suggestion` / `Approve n suggestions` | `not_found` (none of them left), `db` |
| `suggestion_approve_edited` | `day: Day, id, input: RecordingInput, overlap` | `Recording`: `input` (the editor's fields) created and the suggestion's whole span dismissed, one undo entry (`Recordings::create_settling`) | `not_found` (the suggestion is gone), `overlap`, `invalid_input`, `db` |
| `suggestions_dismiss` | `day: Day, ids: string[]` | `number` dismissed: each one's rule, day and span stored in `dismissed_suggestions`, one undo entry `Dismiss suggestion` / `Dismiss n suggestions` | `not_found` (none of them left), `db` |
| `rules_today` | – | `RuleDay[] { ruleId, matchedMs, pending, pendingMs }` for each active rule today (`rules::day_of_rules`): the activity it matched (first match wins) and its suggestions waiting for review; the Rules screen | `db` |
| `app_settings_set` (`commands.rs`) | `bundleId: String` (the app key: bundle id, else the app name), `patch: AppSettingPatch { ownLane?, hidden? }` | `Settings`; applied under the settings store's lock (`SettingsStore::update_app`), so concurrent changes never undo each other; an override that ends up default is removed (`Settings::with_app`) | `invalid_input` (blank key), `io` |
| `apps_list` (`day/commands.rs`) | – | `KnownApp[] { appKey, app, totalMs, lastSeen }` (`day::apps::known_apps`): every tracker app and every app with an override, by name | `db` |
| `recording_create` (`recordings/commands.rs`) | `input: RecordingInput { projectId, spans: Span[], note, tagId }, overlap: Overlap` | `Recording` (snapped) | `invalid_input` (`field`: `spans` empty or overlapping each other, `end`, `note`, …), `overlap` (`details`: the recordings in the way), `not_found`, `db` |
| `recording_update` | `id, patch: RecordingPatch, overlap` | `Recording` | the same |
| `recording_delete` | `id` | – | `not_found`, `db` |
| `recording_add_span` | `id, span: Span, overlap` | `Recording` with the span added (snapped) | the same, `invalid_input` for the running timer |
| `recording_update_span` | `id, spanId, span: Span, moving: SnapEdge \| null, overlap` | `Recording` with the span's new times as a drag left them (on `DayView.ticks`), through `Recordings::drag_span`: `snap_moving` with `moving` as the dragged end, which keeps the other edge's time (`null` when the span moved whole: both snapped), checked but not snapped again | the same; `not_found` for an unknown span |
| `recording_remove_span` | `id, spanId` | `Recording`, or `null` when it was the last span and the recording was deleted | `invalid_input` (the running timer), `not_found`, `db` |
| `recording_split_span` | `id, spanId, at: Ms` | `Recording` whose span is cut in two at the snapped `at`; both halves stay in it | `invalid_input` (`field`: `at` not inside the span; the running timer), `not_found`, `db` |
| `recording_merge` | `into, from` | `Recording` `into` with the spans of `from` (which keeps its project, note and tag); `from` is deleted | `invalid_input` (`field`: `from` the same recording; a running one), `not_found`, `db` |
| `recording_nudge` | `id, spanId, startSteps: i32, endSteps: i32, overlap` | `Recording` whose span's start and end moved by that many snap steps (`timeline.snapMinutes`; ⌥↑ ⌥↓ ⌥⇧↑ ⌥⇧↓) from their nearest grid marks; an end moved alone stays a step from the other (`snap_moving`), through `update_span` | as `recording_update_span` |
| `recording_move` / `recording_copy` | `id, from: Day \| null, day: Day, overlap` | `Recording`: moved, or a new copy, to `day` at the same local time (`same_time_on`, DST-safe) with the same lengths (undoable). With `from` the spans touching that day (the Week card's drag; the copy is a new recording of them), shifted by the days between `from` and `day`; without, every span, shifted by the days between the first span's day and `day` | `invalid_input` (the running timer), `overlap`, `not_found`, `db` |
| `week_view` (`calendar/commands.rs`) | `day: Day` | `WeekView { weekStart, label, days: DayColumn[], totalMs, trackedMs }`; `DayColumn { day, weekday, dayOfMonth, isToday, isWeekend, entries: CalendarEntry[], totalMs, trackedMs }`; `CalendarEntry` is a `TimesheetCard` (one per recording, the day's parts of its spans added up, by start) plus `heightUnits` (the Week card's height, see Week and month) and `showNote` (whether the card fits its note) | `invalid_input`, `db` |
| `month_view` | `day: Day` | `MonthView { monthStart, label, weeks: MonthDay[][], totalMs }` (whole weeks Monday to Sunday; `totalMs` counts the month's own days); `MonthDay { day, weekday, dayOfMonth, inMonth, isToday, isWeekend, recordedMs, entries: CalendarEntry[] }` (every recording of the day, by start) | `invalid_input`, `db` |
| `calendar_period` (`calendar/commands.rs`) | `unit: PeriodUnit` (`day`, `week`, `month`), `day: Day` | `Period { from, to, holdsToday }`: the day, Monday-to-Sunday week or month holding `day` (`calendar::period::period`); the billable ratio and the date picker's selection | `invalid_input` |
| `calendar_mini_month` | `shownDay: Day`, `offset: i32` (months from `shownDay`'s, the ‹ › paging), `shown: Shown` (`{ kind: "period", unit, day }` for the Day, Week or Month view, `{ kind: "report", range }`, `{ kind: "none" }`) | `MiniMonth { monthStart, monthName, year, monthSelected, weeks: MiniWeek[] }` (`monthName` short or full by `calendarTitle`), six weeks from the Monday on or before the 1st; `MiniWeek { number (ISO), start, selected, days: MiniDay[] }`; `MiniDay { day, dayOfMonth, inMonth, isToday, hasRecordings, selected }` (see [Week and month](#week-and-month)); the sidebar's calendar | `invalid_input` (a report range that ends before it starts, a month out of range), `db` |
| `calendar_picker` | `month: Day` (any day of it), `selected: [Day, Day] \| null` | `PickerMonth { monthStart, weeks: PickerDay[][] }` (whole weeks Monday to Sunday); `PickerDay { day, outside, isToday, selected }` (`calendar::period::picker_month`); the date picker | `invalid_input` |
| `calendar_jumps` | `text: string` | `DateJump[] { name: string \| null, unit: PeriodUnit, from: Day, to: Day, label: string }` (`calendar::jump::jumps`): a typed date (`2026-10-12`, `12.10.`, `12.10.2026`, `oct 12`, `12 october 2026`; without a year the latest on or before today) as one day without a name, else the date words starting with `text` or one of whose words does (Today, Yesterday, This week, Last week, This month, Last month, a weekday: the latest on or before today, a month name: the latest on or before this month); `""` gives Today, Yesterday, This week, This month; the palette | – |
| `calendar_step` | `unit: PeriodUnit, day: Day, by: i32` | `Day`: `by` days, weeks or months from `day` (a month step lands on the 1st; `calendar::period::step`); P and N, the date picker's keys, moving a recording a day | `invalid_input` (out of the date range) |
| `calendar_parse_time` | `text: string, on: Ms` | `Ms \| null` (`cli::time::typed`): a clock time (`9:30`, `09:30`, `930`, `0930`) on the local day of `on`, or a date and time (`2026-10-21T09:30`) as the CLI reads it; `null` while it is neither; the editor's and the away prompt's From / To, the Delete activity range | – |
| `recordings_list` | `from: Ms, to: Ms` | `Recording[]` with a span sharing time with the range, each with all its spans | `db` |
| `recording_snap` | `start: Ms, end: Ms` | `Span` on the grid (`snap_span`), as a write snaps: a block selection (a click or ⇧J ⇧K) and what `recordSelection` opens; a drag's selection is on the grid already | – |
| `recording_draft_spans` (`recordings/draft.rs`) | `spans: Span[]` | `DraftSpans { spans, totalMs }`: each end moved to the first instant with its clock time after its start (an end at or before the start is on the next day, DST-safe through `same_time_on_in`), in the order given, and their sum; the editor's From / To | – |
| `recording_draft_total` | `spans: Span[], totalMs` | `DraftSpans` with the last span's end moved so they add up to `totalMs`; unchanged when that would end it at or before its start or a day or more after it; the editor's duration | – |
| `recording_draft_add` | `spans: Span[]` | `DraftSpans` with a 30-minute span after the last (without spans, from the start of `default_span`); Add span | – |
| `recording_draft_include` | `spans: Span[], span` | `DraftSpans` by start with `span` (a snapped selection) in them, merged with the spans it touches or overlaps; a drag or ↵ while the editor is open | – |
| `recording_draft_resize` | `spans: Span[], index, span, moving: SnapEdge \| null` | `DraftSpans` by start with the span at `index` replaced by `span` snapped with `snap_moving` (`draft::resized_on_grid`; ticks stay, the other edge keeps its time), merged with the spans it then touches or overlaps; unchanged without a span at `index`; a dragged edge of a draft block in the Day view | – |
| `recording_draft_toggle` | `spans: Span[], block: Span, others: Span[]` | `DraftToggle { spans, totalMs, included }`: `block` snapped with `snap_span`; when one span covers it, its time leaves them (a span sticking out on both sides becomes two) except where it overlaps `others` (the other clicked blocks, snapped), else it joins them, merged with what it touches; by start; a click on a block in the Day view | – |
| `recording_proposal` | `spans: Span[]` | `RecordingProposal { projectId }` (`recordings/proposal.rs`): the project of the latest recording in the 14 days before the first span that covered the spans' main app (`main_app`: the app with the most time inside the spans among the blocks of every lane but the summary on the spans' days, private left out), else of the rule matching the most time inside; archived projects are never proposed. The editor's project prefill | `db` |
| `recording_default_span` | `day: Day \| null` | `Span`: the half hour up to now (`NEW_RECORDING_MINUTES`), snapped (`recordings::default_span`); on another `day` the same clock times there (`recordings::default_span_on`). File → New Recording… without a selection, and the Week's and Month's + for a day | – |
| `undo` / `redo` | – | the label undone or redone, `null` when there was none | `overlap`, `db` |
| `undo_status` | – | `UndoStatus { undoLabel, redoLabel }` | – |
| `activity_list` / `away_list` (`db/commands.rs`) | `day: Day` | `ActivityRow[]` / `AwayRow[]` of the day, cut to its bounds; open rows end now | `db`, `invalid_input` |
| `app_icons` (`app_icons.rs`) | `bundleIds: string[]` (app keys: a name that is not a bundle id finds nothing) | `Record<string, string>`: a `data:image/png;base64,…` URL per id of an installed app; ids without an icon are left out. Runs on a blocking thread | – |
| `apps_seen` (`db/commands.rs`) | `days: u16` | `AppSeen[] { bundleId, app, lastSeen, totalMs }`: tracker apps of the last `days` days, one per bundle id, most time first | `db` |
| `tracker_status` (`tracker/commands.rs`) | – | `TrackerStatus` (the last one sent) | `db` (tracker off) |
| `tracker_pause` | `until: PauseUntil` | `TrackerStatus` | `db` |
| `tracker_resume` | – | `TrackerStatus` | `db` |
| `request_permission` | – | `PermissionStatus` (after the system prompt, shown once per run) | `db` |
| `open_privacy_settings` | – | – (System Settings → Privacy & Security → Accessibility) | `io` |
| `clients_list` (`projects/mod.rs`) | `includeArchived: bool` | `Client[]` by name | `db` |
| `clients_save` | `id: string \| null, input: ClientInput` | `Client`; `null` creates; a new color recolors its projects | `invalid_input` (`field`), `not_found`, `db` |
| `clients_archive` | `id, archived: bool` | `Client`; archiving archives its projects | `not_found`, `db` |
| `projects_list` | `includeArchived: bool` | `ProjectListItem[]` (hours of this week and this month: `usage_windows` in `projects/mod.rs`) | `db` |
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
| `activity_count` | `from: Ms, to: Ms` | `ActivityCount { activities, away, rangeLabel }`: the spans `activity_delete` would remove, and the range as text | `invalid_input` (from ≥ to, over a year), `db` |
| `activity_delete` | `from: Ms, to: Ms` | `DeleteResult { activities, away, undoUntil }` | `invalid_input` (from ≥ to, over a year), `db` |
| `activity_delete_undo` | – | `DeleteResult` of the restored rows | `not_found` "Nothing to undo", `db` |

To add a command: write it in its module (or `commands.rs` for app-wide ones), register it in
`handlers.rs`, export its types with ts-rs, add the wrapper to `src/api/index.ts` and a default to
`src/test/apiMock.ts`.

## App icons

`app_icons.rs`. Every activity shows its app's icon, read from the installed app, so nothing needs the
network. `AppIcons::icon(bundle_id)` returns the PNG cached in `<config dir>/data/cache/icons/<bundle id>.png`
(the default data folder, also when the data lives elsewhere: a cache does not belong in a moved or synced
folder); on a miss it asks its `IconSource` and writes the file. An id the source has no icon for (an app
that is not installed, or an app name) is remembered for the run and not asked again. Ids that are not
bundle-id shaped (letters, digits, `.`, `-`, `_`; no leading dot) are refused before any lookup, which also
keeps paths out of the cache's file names. `AppIcons::data_urls` serves the `app_icons` command.

`app_icons::installed()` returns the macOS source (`InstalledApps`): `NSWorkspace
URLForApplicationWithBundleIdentifier:`, `iconForFile:` on the app's path, drawn into a 64×64 px
`NSBitmapImageRep` (32 pt at @2x, `ICON_PX`) through an `NSGraphicsContext` that is current on the calling
thread only, then `representationUsingType: PNG`. Tests inject a fake `IconSource`.

The page gets data URLs over IPC rather than files, so the CSP stays at `img-src 'self' data:`
([Content security policy](#content-security-policy)). Cached icons are not refreshed when an app updates
its icon; deleting the `cache/icons` folder does. External sources (CLI activity) have no bundle id and show
their initial.

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
0012](adr/0012-timeline-layout-in-rust.md)); `day::load` reads the rows (`list_for_range`, spans
from the day before included) and calls it. `ViewSettings` is a copy of `Settings.timeline` and
`Settings.apps`. The module is split by concern: `types.rs` (everything `DayView` sends), `totals.rs`
(`totals`, `tracked_ms`, span unions), `recordings.rs` (`recording_blocks`, `timesheet`), `summary.rs`
(the summary lane), `lanes.rs` (lanes and columns), `transform.rs` (the timeline settings), `title.rs` (`clean_title`). In order:

1. `clipped_blocks`: every activity cut to the day's bounds (an open span ends at `now`); private rows keep
   the app and read "Private" without URL or path. Titles go through `title::clean_title`: the symbols leading
   the title or any of its ` — ` parts (spinners and dingbats such as `◐`, `✳`, `⠋`: any non-ASCII character
   that is not a letter or digit) are dropped, so one document keeps one label; ASCII punctuation stays. `appKey` is the bundle id, else the app name: what
   `Settings.apps` overrides are keyed by.
2. Totals (`totals`, also behind `day_totals`), `firstActivity` and `sources` come from these raw rows,
   so hiding and merging never change them: `trackedMs` the union of tracker spans, `awayMs` the union of away spans, `recordedMs` the sum of
   the recordings' parts in the day. The away spans themselves are not sent: `barriers` keeps the sleep,
   lock and pause spans cut to the day (`#[serde(skip)]`) for `transform::grouped` and the rules.
3. External rows go to their source lanes as they are. Tracker rows run through `day/transform.rs`:
   `without_hidden` drops apps with `hidden`; `identified` labels each row (`titles`: the title, or the app
   when empty or in `app` mode; private rows "Private") and keys it by app (`groupByApp`) or by app and
   label, a per-app override winning over `timeline`; private rows get a key of their own.
4. `summary::summary` (with `summaryLane`; `bucket_winners`, then `run_blocks`): the day in buckets of `summaryMinutes` (default 5) from local
   midnight, so they sit on the clock (:00, :05, …); each bucket goes to the key with the most time in it
   (ties: the earlier start), a bucket without activity stays empty, and a run of buckets with one winner is
   one block over the whole run, id `s<start>`. Every summary block starts and ends on the grid (:45, :50,
   never :49); its `activeMs` is the winner's time inside it.
5. `transform::grouped`: rows of one key that touch, or whose gap is shorter than the continuity (off 0,
   short 2, regular 8, extended 16 min), become one block unless a sleep, lock or pause lies between
   (`breaks_continuity`; idle is bridged). Other keys' rows in the gap stay their own blocks, beside it.
   Blocks shorter than the noise threshold (off 0, low 2, medium 4, high 6 min) are dropped. A block shows
   its longest row (`title`, `label`, URL, path); its id is `a<first member id>`, `members` every row,
   `activeMs` the time its rows fill (without the gaps it bridges).
6. `lanes::lanes`: the `activity` lane (unless `showAllActivity` is off; without blocks shorter than a minute, which never have room for a label), one `app:<appKey>` lane per app
   with `ownLane` in the order of first appearance (`bundleId` for its icon), one `source:<name>` lane per
   external source by name;
   `packed` per lane: by start, longer first, each block in the lowest free column, a cluster of overlapping
   blocks sharing its width (`col` of `cols`).
7. `mark_covered`: a block of any lane is `covered` when at least half of it lies inside recordings.

`DayView` also carries `ticks` (every snap mark from the first instant of `bounds` to the last, both
included: `recordings::ticks`), `compact` (from `timeline`), `hiddenApps` (every hidden override, named from the
day's rows, else by its key) for the header's popover, `apps` (the day's tracked apps that are not hidden,
most time first by `activeMs`, with `ownLane`; `apps::day_apps`) for the View options popover, and `timesheet`: one `TimesheetCard` per recording,
in the order of its first span on the day, with the day's parts of its spans added up (`durationMs`), its
first start and last end on the day and its `spanIds` there (`recordings::timesheet`, from the recording
blocks).

## Recordings

`recordings/` turns spans into recordings ([ADR 0010](adr/0010-recordings-never-overlap-ops-with-undo.md),
[ADR 0020](adr/0020-recordings-have-spans.md)). A recording holds one or more spans; project, note and tag
belong to the recording. `Recordings { db, undo, snap_minutes }` (`mod.rs`) is the service; each write takes
the `AppState.undo` lock for its whole run, so writes never interleave. `create`, `update`, `delete`,
`add_span`, `update_span`, `drag_span`, `nudge_span`, `remove_span`, `split_span`, `merge`, `move_to_day` and `copy_to_day` validate, snap,
clear the spans, build an `ops::Op`, apply it and push it on the undo stack; `commands.rs` then emits
`data_changed { kind: recordings, day }` for every day a span of the op touched, before and after
(`Change.days`).

- **Snapping** (`snap.rs`: `snap`, `snap_span`, `snap_moving`, `ticks`, `grid_step`): to the nearest multiple of
  `timeline.snapMinutes` counted from local midnight, read at call time; an end that snaps onto or before the
  start lands one step after it, so a selection over activity from 13:12 to 13:38 is 13:10–13:40.
  `ticks(first, last, minutes)` lists every mark from `first` to `last`, both included (none in a skipped DST
  hour, a repeated hour's twice); it fills `DayView.ticks`, the only times a timeline drag lands on
  ([ADR 0026](adr/0026-timeline-pointer-input-snaps-to-ticks.md)). `snap_moving` is the rule for saving a
  drag: with a moving end (`SnapEdge::Start | End`) only that end snaps, to the nearest mark past the other
  end, which keeps its time even off the grid; without one both ends snap (`snap_span`). A tick stays as it is. `update` snaps only when the
  patch has `spans`, so editing a note keeps the times.
- **Validation** (`checks.rs`, `InvalidField`): at least one span (`spans`), each `end` after its `start` and at most 24 h
  later, none overlapping another of the recording (`spans`, checked by `db::recordings::check_spans` for both layers; touching is fine); `note` one line, at most 500
  characters, trimmed; `projectId` not archived (unless the recording already belongs to it); `tagId` (one
  tag or `null`, [ADR 0019](adr/0019-one-tag-per-recording.md)) known.
  A missing project is `not_found`; a running recording is not edited here ("Stop the timer first").
- **Overlap** (`overlaps.rs`, `Overlap::Cancel | Replace`), per span: with `Cancel` a recording whose spans share time with
  another's is refused with `overlap` and those recordings in `details`; with `Replace` each span in the way
  is removed (inside), trimmed (one side out) or cut in two (both sides out; both pieces stay spans of the same
  recording), and a recording left without spans is deleted (`clear_op`), all in one `Op::Batch` with the
  written recording last. A running timer's open span in the way is `invalid_input`.
- **Spans** (`spans.rs`): `add_span`, `update_span` (by span id), `remove_span` (removing the last one deletes the
  recording), `split_span(at)` (two touching spans), `merge(into, from)` (the spans of `from` join `into`,
  `from` is deleted first so its span ids carry over). None of them edits the running timer.
- **Move and copy** (`to_day.rs`: `move_to_day`, `copy_to_day`, through `placed_on_day`): the spans touching a
  `from` day (the Week card's drag: the recording's part on that day), or the whole recording (the Day view's
  menu commands); each moves by the days between `from` (else its first span's day) and the target day, at its
  own local time and length, and the other spans stay. A copy is a new recording of the moved spans.
- **Operations** (`ops.rs`): `Op::Create | Update { before, after } | Delete | Batch` on whole recordings with
  their spans, and `Op::Dismiss | Undismiss` on a dismissed suggestion (so undo covers reviewing); `inverse()`
  swaps them (a batch reversed). `dismissal_days` adds a dismissal's day to the days a change touched. `apply` runs the op in one transaction through
  `db::recordings::put_on` (the row and all its spans exactly as given, timestamps, tag and span ids included)
  and `remove_on`, then refuses with `overlap` if any span it wrote shares time with another recording's;
  then nothing is written.
- **Undo** (`undo.rs`): `UndoStack` keeps the last 100 changes of this run with their labels (`Record 45m
  for Acme`, `Move recording`, `Resize recording`, `Edit recording`, `Delete recording`, `Add time`, `Remove
  time`, `Split time`, `Merge recordings`, `Approve suggestion`, `Approve n suggestions`, `Dismiss suggestion`, `Dismiss n suggestions`, from
  `recordings::suggestions_label`); a new change clears
  the redo list. `undo` applies the inverse, `redo` the op again; one that would now overlap is refused and
  the stack stays as it was. `Recordings::undo`/`redo` live in `undo.rs`; `record_dismissals` (one `Op::Batch` of
  `Op::Dismiss`) and `create_settling` in `review.rs`.

The Day view gets the day's recordings too (`day::load` reads them with the projects, clients and
tags): `DayView.recordings` (`RecordingBlock`, one per span on the day: the recording's `id`, `spanId`,
`spanCount`, the clipped span, project, client, color and shade, note, `tag` name, `running`,
`clippedStart`/`clippedEnd`), `ActivityBlock.covered` when at least half of a block lies inside recordings,
and `totals.recordedMs` (the day's parts of the spans).

## Week and month

`calendar/mod.rs` reads the range's recordings (and, for the week, activity) once and cuts them per day with
the Day view's own `day::recording_blocks`, `day::timesheet` and `day::tracked_ms`, so a recording is one
entry per day it touches with that day's share (`calendar::entries::entries`): a span over midnight counts in
both days, several spans on one day add up. `entries::height_units` gives a Week card one unit per started
quarter hour (`CARD_UNIT_MS`), at least `CARD_MIN_UNITS` (a title over the foot) and at most `CARD_MAX_UNITS`
(six hours); the page multiplies it by its unit size. `show_note` is set from `NOTE_MIN_UNITS` (4) up, the
first height that fits the note between the title and the foot. The Month uses the same entries as equal-height lines.

`calendar/titles.rs` formats every date label the page shows, in the order of the macOS region
(`calendar/locale.rs`: `DateOrder::system()` reads `NSDateFormatter`'s `yMMMd` pattern for the current
`NSLocale` once per run, day first when its `d` comes before its `M`; month first where it cannot be read and
always in tests, so their labels do not depend on the machine). `day_title(day, DayTitle, today)` gives
`Wednesday, November 11, 2026` (Long) or `Wed, Nov 11` (Short, with `, 2025` when not today's year; day first
`Wednesday, 11 November 2026`, `Wed, 11 Nov`), which `day::view` puts in `DayView.label`
(`ViewSettings.day_title`); `range_label(from, to, DayTitle, year)` gives `Oct 19 – 25, 2026`, `Oct 26 –
Nov 1, 2026` or `Dec 29, 2025 – Jan 4, 2026` (`19 – 25 Oct 2026` day first) for `WeekView.label` (`Week 43 ·
…`); `range_title(from, to, DayTitle, today)` adds the year always in Long and in Short only outside today's
year (the status bar's custom periods); `month_title` gives `October 2026`; `period_title(from, to, DayTitle,
today)` names a day, a month or a range with them: the Reports title (`ReportResult.title`, set once by
`reports::build`; `report_run` calls `run_titled` with the Day title setting, `run` titles Long) and, in
Short, `DateJump.label`; `date_label(day)` is `October 21, 2026`
(`DataInfo.firstActivityLabel`); `moments_label(from, to, today)` is `Wed, Oct 21 09:30 – Thu, Oct 22 10:00`
(`ActivityCount.rangeLabel`). `ReportRow.dayLabel` and `PreviewHit.dayLabel` are short day titles. The `_in`
variants take the order as a parameter. `month_name(month, CalendarTitle)` gives `Nov` or `November` for
`MiniMonth.monthName`. The copied report text (`reports/text.rs`, `reports/groups.rs`) keeps its own fixed
dates.

`calendar/mini_month.rs` builds the sidebar's month calendar. `highlight(shown, today)` turns `Shown` into
the highlighted range, a `PeriodUnit` and its first day: a view's period directly, a report range (resolved
by `reports::resolve`) only when it is one day, one Monday-to-Sunday week or one calendar month.
`mini_month(month, range, days, recorded, today, title)` marks the matching day, week row or month title over
the `mini_month_days` of the month, and `recorded_days(spans, days, now)` the days a recording span shares
time with (an open span runs to `now`). `load_mini_month(db, shown_day, offset, shown, title, today, now)`
composes them: the month `offset` months from `shown_day`'s, its six weeks' spans from
`db::recordings::spans_in` (one query on `recording_spans`, start and end only), the recorded days and the
highlight; with `db: None` (no data store) the calendar comes without dots. `tests/performance.rs` holds it under 20 ms on two years of data.

## Reports

`reports/mod.rs` holds the IPC types and `run(db, filter, today, now)`: it resolves the range (`resolve` in
`reports/range.rs`: today, yesterday, this or
last week from Monday, this or last month, or custom days), reads the recordings sharing time with it and
calls the pure `build`. A `ReportFilter` reuses the settings' `Group` and `Rounding`, so the Reports screen
starts from `settings.reports`.

`shift(range, anchor, by, today)` (`reports/range.rs`) steps a range for P, N and O in Reports: its unit is a calendar month
(first to last day), a Monday-to-Sunday week, or else its own number of days. It moves `by` units from the
range, or with `anchor` from the unit that holds the anchor (for days: the days ending on it), so `by: 0`
with today is "Today", "This week" or "This month". A result equal to a preset becomes that preset (a week
back from This week is Last week), anything else `Custom`.

- Rows (`reports/rows.rs`): one per recording and day of the range, with that day's parts of its spans (`spans`, the running one
  ends now; `start`/`end` the first start and the last end; `durationMs` their sum), so a span over midnight
  counts on both days, as in the Day view. Rows (and so a group's notes) are ordered by their first span. Client, project and tag lists narrow
  together (empty: all; a tag filter keeps recordings with any chosen tag); archived projects only with
  `includeArchived`. Notes are trimmed.
- Rounding is per recording (`rounding_step`: nearest, half up, or up), at least one step for a recording
  longer than zero; totals sum the rounded rows, each recording once.
- Billable (`reports/billable.rs`): a row is `billable` when its project is (no per-recording flag). `ReportResult.billable` is
  `Billable::of(totalRoundedMs, billable rows' roundedMs)`: whole percent, rounded, `null` without recorded
  time. `billable(db, range, today, now)` runs the report over every recording of the range for the status
  bar and `timetracker status`; `billable_text` renders `6:24 of 8:00 (80 %)`.
- Groups (`reports/groups.rs`): by project (`Client / Project`, ordered by client then project, client-less last, with color,
  shade and billable flag), by tag (by name, "Untagged" last; each recording counts once, so the groups add up to the
  total), by day (`Mon 2026-10-20`), week
  (`Week 43 · 20 – 26 Oct 2026`) or month (`October 2026`). A group lists its distinct notes in the order
  they first occur, each with its days, and its rows by start.
- Overview (`reports/mod.rs` `build`): `recordingCount` and `projectCount` (distinct), `untaggedRoundedMs`, and
  `notes`, every note of the report as `CopiedNotes { text, count }` (`reports/notes.rs` `copied_notes`: by
  the rows' first spans, empty notes skipped, repeated notes kept, a recording's note once even when it
  spans days; one per line, no trailing line break).
- Charts (`reports/charts.rs`): `ReportCharts { byClient, byProject, byTag }`, each a list of `ChartSlice`
  (key, label, accent `color`, the project's `shade` by project, `roundedMs`, whole `percent`, and `offset` and
  `share` as fractions of the ring), largest first with "No client" and "Untagged" last. Clients keep their
  accent, projects their shade; tags take the accents by their place in the tag list (graphite left out), and
  what has no client or tag is graphite.
- Table (`reports/table.rs`): `clients: ReportClient[]`, by name with "No client" last, each with its
  totals and `projects: ReportProject[]` (by name: label, color, shade, billable, totals, `notes` as
  `CopiedNotes` for "Copy notes", and the rows by start). The groups above stay for the text and MCP and are
  left out of the serialized result (`#[serde(skip)]`), so the page gets every row once, under `clients`;
  the CLI's `--json` adds them back as `groups`.
- `reports/text.rs` renders the text the screen copies and the CLI prints (format in [ux.md](ux.md#reports));
  the CLI adds `Billable: 6:24 of 8:00 (80 %)` below it (`reports::cli::report`) when time is recorded, so the
  copied timesheet stays without it. With `output: notes` on `IpcMessage::Report` (`ReportOutput`; `timetracker report --notes`)
  the reply's text is `result.notes.text` instead.
- `reports/saved.rs`: saved reports in `saved_reports` (the filter as JSON; a filter this version can't read
  opens as the default). Presets stay relative, so a saved "this month" follows the calendar.
- `reports/testing.rs` (tests only): the shared report fixtures (clients, projects, tags, recordings from
  local times).

## Away prompt

`away_prompt/`: after a long away span the `away` window asks what the user did.

- `Tracker::subscribe_away` broadcasts each away span `leave_away` closes because the user is back (input
  after idle, wake, unlock, resume). Spans closed at startup (`recover`) or by switching to another away
  reason are not sent, so a prompt only follows a live return.
- `watch` (started with the tracker) checks `should_prompt`: the setting is on, the span is closed, at least
  `tracker.awayPromptMinutes` long, and not a pause (the user chose it). It stores the `AwayPrompt { span,
  endedAt, awayMs, suggested }` (`suggestion`: both ends snapped, at least one snap step) in `AwayPromptState`, replacing an
  open one, sends `away_prompt_changed` to an open window, and opens the window.
- `window.rs`: label `away`, `src/windows/away/index.html`, 420×430, no decorations, always on top, centered,
  built on the main thread (`run_on_main_thread`, since `watch` runs on a tokio task). It stays open when it
  loses focus; closing it clears the prompt. Capability `capabilities/away.json`.
- `answer` records through the recordings service with `overlap: cancel`; an overlap keeps the prompt so the
  times can change; success clears it.

## Rules and suggestions

`rules/`: a rule says "activity like this belongs to project X"; it never writes a recording. Its suggestions
fill `DayView.suggestions` and wait for the owner's review: approving one is an ordinary undoable create,
dismissing one keeps its time from that rule for good.

- `store.rs`: `rules` and `rule_conditions` (migration `0005_rules.sql`), in position order. `validated`
  checks the name, note, project (existing, not archived), tag (`tagId`, one or `null`) and conditions (at least one; a value
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
  suggestions never overlap. Recorded time, and each rule's own dismissed time (`dismissed`, read by
  `day::load` with `db::dismissed::list_for_day`), is cut away; pieces under `MIN_SUGGESTION_MS` (5 min) are
  dropped. Ids are `<rule id>:<start>:<end>`, so a suggestion keeps its id across reloads. A tag deleted since
  the rule was saved is left out; `tag` carries the tag's name, `clientName` the project's client.
  `rules::day_suggestions` sets each suggestion's `nextCard` (`next_card`: the first timesheet card starting
  after it), which the page puts it before, and `durationMs`.
- `review.rs` (re-exported from `rules/mod.rs`): `approve`, `approve_all(ids)`, `approve_rule`, `approve_edited`
  and `dismiss(ids)` rebuild the day's suggestions and find them again through one lookup (`find_where`; a
  stale id is `not_found`: "The suggestion is gone", "The suggestions are gone", or "The rule has no
  suggestions on this day"), then
  write through the recordings service, so each is one undo entry. A dismissal (`db::dismissed::Dismissal
  { rule_id, day, start, end }`) is an `Op::Dismiss`; its time stays the rule's even after the rule is edited
  or a recording over it is deleted. `approve_edited` creates the edited recording and dismisses the
  suggestion's span in one `Op::Batch` (`Recordings::create_settling`), so what the edit left out does not
  come back.
- `day_of_rules` (`rules_today`): per active rule, the matched time (`suggest::matched_ms`) and the day's
  pending suggestions.
- `preview` builds the views of the last days (`day::view_of`, without suggestions) and lists the
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

`cli/mod.rs` (clap derive; `Cli`, `Command`, `parse` and `message`), with the larger subcommands' flags and
messages in their own files: `cli/activity.rs` (`ActivityCommand`), `cli/record.rs` (`RecordArgs`),
`cli/report.rs` (`ReportArgs`); `cli/batch.rs` holds `split`; `cli/testing.rs` the tests' shared helpers.
Global flags: `--hidden` (start without the window; a running instance stays
as it is), `--log-level <error|warn|info|debug|trace>` (overrides the setting for this run), `--json`
(replies as JSON), `--help`, `--version`.

| Subcommand | `IpcMessage` | Prints |
|---|---|---|
| `show [DAY]` | `show { day, focus: false }`; `DAY` is `YYYY-MM-DD`, `today` or `yesterday` (default today) | – |
| `focus` | `activate { hidden: false }` | – |
| `hide`, `quit` | `hide`, `quit` | – |
| `status` | `status` | six lines, below |
| `pause <15m\|1h\|tomorrow\|off>` | `pause { until: PauseUntil }`; `<n>m` or `<n>h` up to 24 h, `tomorrow` = next local midnight, `off` = until resumed | `tracker: paused until 14:30` |
| `resume` | `resume` | – |
| `activity add --source S --title T --start ISO --end ISO [--app A] [--url U] [--path P]` | `activityAdd { items: [ActivityInput] }` | `added activity 4711` |
| `activity add --json` (no other flags) | `activityAdd`, items from a JSON array of `ActivityInput` on stdin (times in UTC ms) | `added 3 activities` |
| `activity start --source S --title T [--start ISO] [--app A] [--url U] [--path P]` | `activityStart { input }`, start default now | `started planner at 10:00` |
| `activity stop --source S [--end ISO]` | `activityStop { source, end }`, end default now | `stopped planner at 10:45 (45 min)` or `nothing to stop for planner` |
| `activity delete DAY` / `activity delete FROM TO` | `activityDelete { from, to }`: `DAY` (`YYYY-MM-DD`, `today`, `yesterday`) is its local bounds; `FROM`/`TO` as `instant` | `deleted 412 activity spans and 3 away spans` and `undo within 30 s: timetracker activity undo` |
| `activity undo` | `activityDeleteUndo` | `restored 412 activity spans and 3 away spans`; nothing to undo exits 1 with `Nothing to undo` |
| `report [today\|yesterday\|week\|last-week\|month\|last-month\|DAY\|FROM TO] [--by project\|tag\|day\|week\|month] [--client C]… [--project P]… [--tag T]… [--round 5\|6\|10\|15\|30] [--up]` | `report { request: ReportRequest }` (`cli/report.rs` parses the range and flags); the app resolves names (`reports::cli`: an id, an exact name ignoring case, or a unique start; several matches list them) and runs the report | the report text and `Billable: 6:24 of 8:00 (80 %)` (when time is recorded); `--json`: the `ReportResult` (with `billable { recordedMs, billableMs, percent }`) plus `text` |
| `start PROJECT [--note N] [--tag T]` | `timerStart { request: TimerStartRequest }`; the app resolves the project and tag like `record` | `started Acme at 09:15` (the open span's start; a recording of today with the same project, note and tag continues); `--json`: the `Recording` plus `projectName` |
| `stop` | `timerStop` | `stopped 09:15–10:40 (1:25)` (the span just closed) or `no timer running` (exit 0) |
| `mcp` | none: serves MCP on stdin/stdout (`CliOutcome::Mcp`) | only JSON-RPC on stdout |
| `record --project P (--from ISO --to ISO \| --span FROM-TO…) [--note N] [--tag T] [--replace]` | `record { request: RecordRequest { spans, … } }`: `--from`/`--to` is one span, each `--span` (`14:25-15:00`, ends as `instant`, joined by `-` or `–`; `time::span`) another (`overlap: replace` with `--replace`, else `cancel`; one `--tag`, a second is a parse error); the app resolves the project (`projects::resolve`) and the tag by name (`tags::resolve_name`, unknown → `not_found`) | `recorded 14:25–15:00, 16:05–20:15 (4:45) for Acme · fix login`; an overlap exits 1 with `overlaps 09:15–09:45 Acme · standup (use --replace)`. `--json`: the `Recording` plus `projectName` |

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
billable today: 6:24 of 8:00 (80 %)        (billable today: nothing recorded)
window: hidden
```

`current` is the tracker's open activity only; an open external span (`activity start`) does not show here.
`--json` prints the reply's `result` instead (`status --json` is a `StatusReport { version, tracker,
timer, billable, windowVisible }`; `tracker` and `billable` are `null` while the data store is unavailable); an error prints
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

`AppState::open` queues `LaunchEvent::Notice` (`LaunchQueue::notice`) when opening the data dropped tags
(migration 0006, [data.md](data.md#migrations)). Messages that start the app are pushed to `LaunchQueue` in
`setup` instead: `show` becomes
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
- `mcp/data/`: `Core`, the headless data access (`mod.rs`), with one file per tool area: `report.rs`,
  `recordings.rs`, `catalog.rs` (projects, tags), `day_summary.rs`, `activity.rs`, `status.rs`; their tests
  share the seeded day in `fixtures.rs`. The database opens on first use with
  `Db::open_read_only` (path from `Settings::peek` and `db::data_dir`), so the handshake succeeds on a Mac
  that never ran the app, and a failed open is tried again on the next call. Settings are read on every call.
- `mcp/ui.rs`: `ask(socket, message)` sends an `IpcMessage` like the CLI; no listener answers
  `NOT_RUNNING` (`TimeTracker is not running`).

| Tool | Args | Returns |
|---|---|---|
| `report` | `range` (`today` default, `yesterday`, `week`, `last-week`, `month`, `last-month`, a day) or `from`/`to`; `group_by` (`project` default, `tag`, `day`, `week`, `month`); `client`, `project`, `tag`; `format` (`text` default, `json`) | `reports::text`, or `{ from, to, group_by, rounding, total_min, total_rounded_min, billable_min, billable_percent, groups: [{ label, minutes, rounded_min, notes }] }` (`billable_min` out of `total_rounded_min`; `billable_percent` `null` without recorded time); rounding from `settings.reports.rounding` |
| `recordings` | `from`, `to` (days, inclusive), `project`, `tag`, `limit` (default 500, at most 5000) | `[{ id, day, start_iso, end_iso, spans: [{ start_iso, end_iso }], duration_min, project, client?, note, tag }]` (`tag`: the name or `null`; `spans` the day's parts, `duration_min` their sum) by start, one row per recording and day, archived projects included |
| `projects` | – | `[{ id, name, color, client?, billable, archived }]` |
| `tags` | – | `[{ id, name }]` |
| `day_summary` | `day` | `{ day, recorded_min, tracked_min, away_min, by_project, by_tag }`; totals from `day::view_of`, the groups `[{ name, minutes, notes }]` from `reports::run` for that day |
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
| Stop Timer (Acme, 1h 23m) | `timer.stop` | `timers::commands::stop` (disabled without a timer) |
| Recent Timers ▸ `Project · note` | `timer.recent.<n>` | starts that combination |
| Open TimeTracker | `open` | `window::show_main` |
| Quit TimeTracker | `quit` | `app.exit(0)` |

`tracker::start` passes every tracker status to `tray::sync(app, &status)`. The icon
follows `TrayIcon::of(state)`: `icons/tray/tray@2x.png` (running or away), `paused@2x.png` (a pause sign in
the clock) and `attention@2x.png` (an exclamation mark: the permission is missing). `sync` only relabels the
status item while the icon stays the same, and rebuilds menu and icon when it changes, so an open menu is not
replaced every few seconds. `tray::set_timer` shows a running timer as the item's title, `⏱ 1h 23m Acme` (the
project cut to 24 characters), and rebuilds the timer items. The tray's click events go to
`tauri_plugin_positioner::on_tray_event`, so the picker can open under the icon.

## Timers

A timer is a recording with an open span, so the overlap rule, undo and reports need nothing new.

- `recordings/timer.rs`: `Recordings::start_timer(TimerInput, now)` starts at `snap(now)`, moved forward past
  any span that ended since (`free_start`); a running timer stops at that same instant, in one batch
  ("Start timer for Acme"). A finished recording of today with the same project, note and tag whose spans
  all end by then continues: it gets the open span instead of a new recording being made (the latest such
  one). When the running timer already is that project, note and tag, nothing changes. `stop_timer(now)`
  closes the open span at `stop_time(start, now, snap)`: on the grid, and at least a minute long. Both go through `apply`'s overlap check and onto the undo stack; undoing a stop makes
  the timer run again.
- `timers/mod.rs`: `status(db, now) -> TimerStatus { running: Option<RunningTimer { recording, projectName,
  color, shade, elapsedMs }> }` (`elapsedMs` since the open span started, `open_start`), `recent(db, 8) -> RecentTimer[]` (distinct project, note and tag of finished
  recordings, newest first, archived projects left out), `publish(app)` (the `timer_status` event and the tray),
  `start_ticking(app)` (publishes at startup and every minute while a timer runs).
- `timers/commands.rs`: `timer_start(projectId, note, tagId)`, `timer_stop` (`null` without a timer),
  `timer_status`, `timers_recent`, `timer_picker_open`, `timer_picker_close`; `start_by_name` serves the CLI.
- `timers/picker.rs`: the `timer` window (`src/windows/timer/index.html`, 380×380, no decorations, transparent,
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
| `settings.json` | `Settings` (camelCase, `#[serde(default)]`; nested `tracker`, `capture`, `timeline`, `apps`, `reports`, `mcp` (`McpSettings { exposeActivity: false }`) each `#[serde(default)]`). Enum, free-text and nested fields use `deserialize_with = "lenient"`: an unknown value falls back to that field's default instead of quarantining the file. `textSize` (`TextSize`: small, medium, the default, or large) is the one text size; the pixel sizes per step are the frontend's `TEXT_SIZES`. Unknown keys are ignored on load and dropped on the next save; a `uiFontSize` in pixels is read into the private `legacy_ui_font_size` (`#[serde(rename = "uiFontSize", skip_serializing)]`) and `sanitized()` turns it into the nearest step (`TextSize::nearest_to_px`: up to 12 Small, 13–14 Medium, 15 and up Large). `sanitized()` clamps on load and on update: `tracker.idleMinutes` 1..=120, `tracker.awayPromptMinutes` 1..=240, `timeline.zoom` 0.5..=4.0 (non-finite: 1.0); snaps `timeline.summaryMinutes` to `SUMMARY_MINUTES` and `timeline.snapMinutes` to `SNAP_MINUTES` (nearest, smaller on a tie); resets an unknown accent (neither `system` (`SYSTEM_ACCENT`) nor one of `ACCENTS`: `is_app_accent`) to `system` (an unknown `lightScheme` or `darkScheme` falls back to `default` or `dark` through `lenient`) and moves a 0021 file's `scheme` into `lightScheme`; turns a blank `dataDir` into `null` (an unknown `calendarTitle` (`CalendarTitle`: short, the default, or full) or `dayTitle` (`DayTitle`: long, the default, or short) falls back through `lenient`); a blank `timerShortcut` becomes `Mod+Shift+T`. `SettingsStore::update` returns `(previous, saved)` from one lock. A save that changes `timeline`, `apps`, `dayTitle` or `calendarTitle` emits `data_changed { kind: settings, day: null }`, so every open day and the sidebar calendar reload. `apps` maps an app key to its `AppOverride { hidden, ownLane }`; an entry left at the defaults is removed. |
| `keybindings.json` | `{ "commandId": "Mod+Shift+X" \| null }` overrides; `null` unbinds. `keybindings::sanitize` drops (and logs) entries whose id is not `[A-Za-z0-9.]+` or whose binding is malformed. A trailing `++` is the `+` key |
| `ui_state.json` | `UiState`: `sidebarHidden`, `sidebarWidth` (px, default 200), `lastDay` (`get_ui_state` leaves it out when it is more than 30 days before today, `UiState::restorable`), `timelineZoom` (the Day view's px per minute, default 2.5), `paneSizes` (px by pane id), `onboardingDone` (default `false`: the welcome sheet shows); unknown keys, such as the dropped `dayMode`, are ignored |
| `data/timetracker.sqlite` (+ `-wal`, `-shm`) | clients, projects, tags, activity, away spans, recordings ([data.md](data.md)); `dataDir` in `settings.json` points elsewhere (written only by `data_move`) |
| `timetracker.sock`, `timetracker.lock` | single instance |

`src/api/types.ts` mirrors the constants the page needs (`ACCENTS`, `SETTINGS_LIMITS`, `TEXT_SIZES`); a Rust
test compares `ACCENTS` with it. Appearance is `lightScheme` and `darkScheme` (`Scheme`: default, light,
dark; defaults Default and Dark; [ADR 0022](adr/0022-a-scheme-per-system-mode.md)), the look for each macOS
mode. A 0021 file's `scheme` is read into the private `legacy_scheme` (`#[serde(rename = "scheme",
skip_serializing)]`) and `sanitized()` moves it to `lightScheme`; its `mode` is ignored. A part 1 file's
`theme` is ignored and its editor scheme names load as the defaults. The old keys are dropped on the next save.

**Adding a setting:**
1. Add the field to `Settings` (or its nested struct) in `store/settings/` (`mod.rs`; nested structs and enums in `types.rs`), with a default.
2. Clamp it in `sanitized`; give an enum field `#[serde(deserialize_with = "lenient")]`.
3. Regenerate `src/api/types.gen.ts` (`bun run types`), mirror a numeric clamp in `SETTINGS_LIMITS`, and
   add the default to `src/test/settings.ts`.
4. Render it on its page in `src/components/settings/pages.tsx`.

## Logging

`tauri-plugin-log` writes to stdout and to Tauri's `app_log_dir`,
`~/Library/Logs/com.philippstracker.timetracker/`, rotating 3 files of 5 MiB, local timestamps. The level
is Info, or `--log-level` for that run (`app::start` passes it to `init_logging`); there is no setting.
`open_log_folder` (Help → Open Log Folder) opens the folder.
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

`menu/` builds the menu bar: TimeTracker (About, Settings…, Launch at Login, Show Welcome Again, Services,
Hide, Hide Others, Show All, Quit TimeTracker), File, Edit (Undo and Redo are app items; Cut, Copy, Paste and
Select All are native), View (ends with the native Enter Full Screen), Go, Day, Window (native Minimize, Zoom,
Bring All to Front) and Help. [ux.md](ux.md#menu-bar) lists every item.

- **Items.** `menu/items.rs` lists every item that runs an app command once in `MENU_ITEMS`
  (`MenuItemDef`: id, label, default binding, `binding_of`, `check`). `build()` in `menu/mod.rs` places them
  in groups of ids (`groups`). The TypeScript command with the same id has `menu: true` and the same default.
- **Bindings.** An item shows `KeybindingsStore::menu_binding` of its command: the user override, else the
  default (`menu::default_binding`). `binding_of` lets an item show another command's key: Record Selection
  shows ↵ of `recording.open`, which records the selection (or edits the focused recording).
  `label_and_accelerator` turns a binding with ⌘ or ⌃ (`keybindings::is_chord`) into an accelerator; any other
  binding goes into the label (`Previous (P)`, `Delete Recording (⌫)`, `Move Recording to Previous Day (⌥←)`)
  and is never an accelerator, because a bare-key or ⌥ accelerator would take its key from text fields; the
  page's `handleKeydown` runs those keys. `keybindings::accelerator` converts to Tauri's form (`Mod` →
  `CmdOrCtrl`, `-` → `Minus`, `=` → `Equal`, `+` → `Shift+Equal`). `set_keybindings` rebuilds the menu
  (`menu::rebuild`). Two items may share an accelerator only when listed in `SHARED_ACCELERATORS` (New Project
  and New Rule, both ⌘N): their scopes exclude each other, so at most one is enabled and macOS gives it the key.
- **Enabled and checked.** The page reports `MenuState { enabled, checked }` (ids) through `menu_sync`
  whenever it changes; `menu/sync.rs` keeps it in the managed `MenuSync` and applies it to every listed item
  (`item_state`): items missing from `enabled` are greyed out, check items show `checked` (the active view,
  Launch at Login). Before the first report every item is enabled; `app.quit` always is.
  `rebuild` builds with the last report.
- **Clicks.** `menu::forward`: `app.quit` exits right there, so quitting works even when the page is broken;
  every other listed id is emitted as `command` (`is_menu_command` filters out the native items). A click on
  a check item re-applies the last report first, since macOS toggles the mark itself; the page's next report
  sets it.
- **README.** Help → README calls `open_readme` (`help.rs`): the `README.md` bundled as a resource
  (`bundle.resources` in `tauri.conf.json`), else in debug builds the repository's, opened in the default app.

To add a menu item: add the TypeScript command with `menu: true` (and `checked` for a check item), add the id,
label and default to `MENU_ITEMS`, place it in `build()`, and list it in [ux.md](ux.md#menu-bar).

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
| `objc2`, `block2`, `objc2-foundation`, `objc2-app-kit`, `objc2-core-foundation`, `objc2-core-graphics`, `objc2-application-services` (macOS only) | the capture source ([capture.md](capture.md#macos-apis)), app icons ([App icons](#app-icons)) |
| `sqlx` (`sqlite` with bundled libsqlite3, `runtime-tokio`, `migrate`, `macros` for `migrate!` and `FromRow`) | the data store |
| `serde`, `serde_json` (`preserve_order`) | IPC, stores |
| `clap` | CLI |
| `thiserror` | `AppError` |
| `chrono`, `uuid`, `url` | time stamps, ids, `is_web_url` |
| `base64` | app icons as data URLs |
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
- `img-src 'self' data:`: no view loads a remote image, even if a window title contains a URL; app icons
  arrive as `data:` URLs;
- `style-src 'self' 'unsafe-inline'`: React `style` attributes;
- `connect-src ipc: http://ipc.localhost`: Tauri IPC only.

`app.security.devCsp` applies under `tauri dev` and adds inline scripts and Vite's HMR websocket on
`localhost:1420`/`1421`.
