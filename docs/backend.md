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
| `activity_list` / `away_list` (`db/commands.rs`) | `day: Day` | `ActivityRow[]` / `AwayRow[]` of the day, cut to its bounds; open rows end now | `db`, `invalid_input` |
| `apps_seen` (`db/commands.rs`) | `days: u16` | `AppSeen[] { bundleId, app, lastSeen, totalMs }`: tracker apps of the last `days` days, one per bundle id, most time first | `db` |
| `tracker_status` (`tracker/commands.rs`) | – | `TrackerStatus` (the last one sent) | `db` (tracker off) |
| `tracker_pause` | `until: PauseUntil` | `TrackerStatus` | `db` |
| `tracker_resume` | – | `TrackerStatus` | `db` |
| `request_permission` | – | `PermissionStatus` (after the system prompt, shown once per run) | `db` |
| `open_privacy_settings` | – | – (System Settings → Privacy & Security → Accessibility) | `io` |

To add a command: write it in its module (or `commands.rs` for app-wide ones), register it in
`generate_handler!`, export its types with ts-rs, add the wrapper to `src/api/index.ts` and a default to
`src/test/apiMock.ts`.

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
or `null` when every day may have changed. `events::data_changed(app, kind, day)` emits it. The page bumps
its `dataVersion` on every `data_changed`, so views keyed on it reload.

## CLI and single-instance handoff

`cli.rs` (clap derive): `--hidden` (start without the window; a running instance stays as it is),
`--log-level <error|warn|info|debug|trace>` (overrides the setting for this run), `--help`, `--version`.
`parse(args) -> Result<CliOutcome, String>`. Subcommands come with the CLI step.

`ipc.rs`:

- **Socket:** a Unix domain socket at `<config dir>/timetracker.sock`, mode 0600 in a 0700 dir. `serve` drops
  peers with another uid (`same_user`, `libc::getuid`).
- **Protocol:** one line of JSON per connection, at most 64 KiB, within 5 s:
  - one-way (a bare `IpcMessage`): answered with `ok\n` before it is handled (`try_forward`);
  - request (`IpcRequest { id, message }`, sent by `ipc::request`): answered once handled with one
    `IpcReply { id, ok, result, error }` line; `IpcReply::into_result` turns it into a `HandlerResult`.

  Malformed, oversized or incomplete lines get no answer.
- **`Forward` outcomes** of `try_forward`: `Delivered` (exit 0, 2 for a CLI error), `NoInstance` (socket
  missing or refused: start the app), `Rejected` (no `ok`) or an error such as a timeout (print and exit 2,
  so a hung instance never leads to a second one).
- **Messages** (`IpcMessage`, tagged by `type`): `activate { hidden }`, `error { message }`, and `eval { js }`
  (debug builds only, sent by `scripts/dev/js.sh`).
- **Handling** (`launch::on_ipc_message`): `eval` runs in the window. Everything else is logged by type
  (`IpcMessage::kind`, never the contents) and pushed to `LaunchQueue`: `error` becomes
  `LaunchEvent::Error`, `activate { hidden: false }` becomes `LaunchEvent::Activate`; a queued event emits
  `launch`. Then the window is shown (`window::show_main`) unless the message was `activate { hidden: true }`.
- **Stale sockets:** `bind` removes a leftover socket only when connecting to it is refused; it refuses to
  bind while a listener is alive.

### Requests the page answers

`remote.rs`: `Bridge::ask` emits `remote` with a fresh id (`pending::PendingReplies`) and waits up to 4 s
(below the IPC client's 5 s) for `remote_reply`. `RemoteRequest` has no variants yet; the CLI step adds them.

### Deep links

`deep_link.rs`: `listen` registers `on_open_url` for `timetracker://` (scheme in `tauri.conf.json`
`plugins.deep-link`) and logs each URL. Routing comes with the CLI step.

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
| `ui_state.json` | `UiState`: `sidebarHidden`, `sidebarWidth` (px, default 200), `lastDay`, `zoom` (default 1), `paneSizes` (px by pane id) |
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
