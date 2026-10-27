# Adapting TimeTracker

TimeTracker is written so that a coding agent can take it over. Clone the repo, open it in Claude Code (or a
similar tool), and describe what you need. `CLAUDE.md` is loaded automatically; it points the agent to the
rules in `.claude/rules/` and to these docs. The `ship-step` skill runs after every step (cleanup, review,
tests, lint, build, launch, commit), and subagents in `.claude/agents/` write the tests, run the suites and
review changed files. This page lists what such changes touch.

## Identity

The app's name and ids live in a few places. Change them together.

| What | Value | Where |
|---|---|---|
| Product name | `TimeTracker` | `tauri.conf.json` `productName` and window `title`; `menu/` (app menu, `Quit TimeTracker`); `tray.rs` (tooltip, item labels); `index.html`; the toolbar; user-facing errors in `lib.rs`, `instance.rs` and `remote.rs`; the `TimeTracker.app` path in `scripts/install.sh`, `scripts/stop-app.sh`, `scripts/dev/launch.sh` and the `app` script of `package.json`; the window owner in `scripts/dev/screenshot.sh`; `Info.plist` usage strings |
| Bundle identifier | `com.philippstracker.timetracker` | `tauri.conf.json` `identifier`; `lib.rs` `IDENTIFIER` (a test compares both); `scripts/dev/js.sh` (the debug socket path); `scripts/dev/launch.sh` (the debug signing identifier); `tauri.debug.conf.json` (the debug bundle's `.debug` identifier, `lib.rs` `DEBUG_IDENTIFIER`, and `TimeTracker (Debug)` name, so macOS lists and grants it apart). It also names the config dir and the log dir (the debug build has its own of both, `BUILD_IDENTIFIER`); changing it starts with empty settings and data |
| Binary and CLI command | `timetracker` | Cargo package `name` in `src-tauri/Cargo.toml`; `cli/mod.rs` `name` and `EXAMPLES`; `cli/reply.rs` (`TimeTracker <version>` line); the README Scripting examples; `scripts/install.sh`; `scripts/stop-app.sh`; `scripts/dev/launch.sh`; `.claude/settings.json` (`pkill -x timetracker`) |
| Rust lib | `timetracker_lib` | `Cargo.toml` `[lib]`; `main.rs`; `tests/` |
| URL scheme | `timetracker://` | `tauri.conf.json` `plugins.deep-link.desktop.schemes` (the bundler writes it into `Info.plist`); `deep_link.rs` (`link_message`, error text); the README Scripting examples |
| Socket, lock | `timetracker.sock`, `timetracker.lock` in the config dir | `lib.rs` `SOCKET_NAME`, `instance.rs` `LOCK_NAME`; `scripts/dev/js.sh` (the socket) |
| Data file | `timetracker.sqlite` in `<config dir>/data/` | `db/mod.rs` `DB_FILE`, `default_data_dir`; `db/relocate.rs` (the copy) |
| LaunchAgent | `~/Library/LaunchAgents/TimeTracker.plist`, written by `tauri-plugin-autostart` and named after `productName` | `app.rs` (`MacosLauncher::LaunchAgent`, `--hidden`), `autostart.rs` |
| Config dir | `~/Library/Application Support/com.philippstracker.timetracker/` (debug builds: `….debug/`) | `lib.rs` `config_dir()`, `BUILD_IDENTIFIER` |
| Logs dir | `~/Library/Logs/com.philippstracker.timetracker/` | Tauri's `app_log_dir`, from the identifier |
| Env vars | `TIMETRACKER_WINDOW_FRAME`, `TIMETRACKER_BACKGROUND` (debug builds), `TIMETRACKER_DEV_TERMINAL`, `TIMETRACKER_SIGN_IDENTITY`, `TIMETRACKER_CLI` | `dev.rs`, `window.rs`, `scripts/dev/launch.sh`, `scripts/dev/seed-day.sh` |
| Store on `window` | `window.timetracker` | `src/main.tsx`, `src/vite-env.d.ts`, the dev scripts |
| Icons | clock glyph | `design/icon.svg`, rasterized to `design/icon-1024.png` (`rsvg-convert -w 1024 -h 1024`); `src-tauri/icons/` regenerated with `bun tauri icon design/icon-1024.png` (only the desktop files are kept); tray template images in `src-tauri/icons/tray/` |
| Author, license | Philipp Stracker, MIT | `Cargo.toml`, `package.json`, `LICENSE` |

## Platform-specific code

| Piece | Where | Linux | Windows |
|---|---|---|---|
| **Capture**: frontmost app, window titles, document paths, idle time, sleep, lock and session notifications, the permission check and prompt | `capture/macos.rs`, which with `app_icons.rs` and `calendar/locale.rs` is the only code that imports `objc2`, `block2`, `objc2-foundation`, `objc2-app-kit`, `objc2-core-foundation`, `objc2-core-graphics`, `objc2-application-services` (macOS-only dependencies in `Cargo.toml`); `capture::new_source` | A new `Source`: X11/Wayland for the active window, `org.freedesktop.ScreenSaver` for idle and lock, logind for sleep | A new `Source`: `GetForegroundWindow` + `GetWindowText`, `GetLastInputInfo`, `WM_POWERBROADCAST` / `WTS_SESSION_LOCK` |
| **App icons**: `NSWorkspace` app URL by bundle id, `iconForFile:`, drawn into a 64×64 PNG | `app_icons.rs` (`InstalledApps`, `installed()`; other targets get no icons and show initials) | A new `IconSource`: the `.desktop` file's `Icon=` resolved through the icon theme | A new `IconSource`: `SHGetFileInfo` / `IShellItemImageFactory` on the app's executable |
| **Date order**: the region's day-before-month order, from `NSDateFormatter.dateFormatFromTemplate("yMMMd")` for `NSLocale.currentLocale` | `calendar/locale.rs` (`DateOrder::system`; other targets read month first) | `LC_TIME` through `nl_langinfo(D_FMT)` | `GetLocaleInfoEx(LOCALE_SSHORTDATE)` |
| **Browser addresses**: AppleScript per browser through `NSAppleScript`; private-window detection | `capture/browsers.rs`, `capture/private.rs` (title rules, portable) | Not readable from outside; keep titles only, or a browser extension that sends `activity` over the socket | UI Automation on the address bar |
| **Permissions**: Accessibility (window titles, document paths) and Automation per browser (the prompt shows `NSAppleEventsUsageDescription`) | `capture/macos.rs`; `src-tauri/Info.plist`; `tracker/commands.rs` `PRIVACY_SETTINGS` (the System Settings link); `PermissionDialog`, the welcome sheet | No prompts; `permission()` returns `granted` | Same |
| **Tray**: template images (black with alpha, tinted by macOS), menu on left click, the timer picker placed under it | `tray.rs`, `src-tauri/icons/tray/`; `timers/picker.rs` (`Position::TrayCenter`, `tauri-plugin-positioner`); from the keyboard on the focused screen (`timers/screen.rs`, AppKit and `CGWindowListCopyWindowInfo`) | Works with an `AppIndicator`; template tinting does not apply, ship colored icons | Works; colored icons |
| **Launch at login**: `MacosLauncher::LaunchAgent` with `--hidden` | `app.rs` (autostart plugin), `autostart.rs` | The plugin writes an XDG autostart entry | The plugin writes a registry key |
| **Global shortcut** for the timer picker (`Settings.timerShortcut`, default `Mod+Ctrl+Alt+T`, `null` for none) | `timers/shortcut.rs` (`tauri-plugin-global-shortcut`), `store/keybindings.rs` `accelerator`, `is_global` | X11 works; Wayland needs the GlobalShortcuts portal | Works |
| **Shortcuts**: `Mod` is ⌘, labels use ⌘⌥⇧ glyphs | `src/commands/registry.ts`, `store/keybindings.rs` `key_label`, `menu/mod.rs` | Map `Mod` to Ctrl | Same |
| **Config dir** hard-coded to `~/Library/Application Support/<id>` | `lib.rs` `config_dir()`, `scripts/dev/js.sh` | `dirs::config_dir()`; it must resolve before Tauri starts, because the CLI and socket run first | `dirs::config_dir()` (`%APPDATA%\<id>`) |
| **Unix socket** for the single-instance handoff, CLI, `watch` and MCP `status`/`open_day` | `ipc/`, `watch.rs`, `mcp/ui.rs` | Works as is; mind the ~108-byte path limit | A named pipe (`tokio::net::windows::named_pipe`) with the same line protocol |
| **File permissions** (0600 files, 0700 dirs) | `ipc/server.rs`, `store/json_file.rs`, `store/json_store.rs`, `db/mod.rs`, `db/relocate.rs` | Works | Gate with `#[cfg(unix)]` |
| **Window chrome**: overlay title bar, `trafficLightPosition`, `transparent`, `windowEffects: ["sidebar"]`, `macOSPrivateApi` | `tauri.conf.json`; Cargo feature `macos-private-api`; `window.rs` | Ignored; the page is opaque already | `windowEffects: ["mica"]`, or an opaque background |
| **Traffic-light space** and drag regions | `--traffic-lights` and `--toolbar-height` in `theme.css`; `data-tauri-drag-region` in `Toolbar.tsx` and `Sidebar.tsx` (`.sidebar-titlebar`) | Remove the inset and the strip | Same |
| **System accent**: WebKit's `AccentColor` follows the macOS accent color; the sidebar turns transparent for the window's vibrancy where its tone matches the mode | `theme.css` (`[data-accent="system"]`), `sidebar.css` | `AccentColor` is the platform's accent or a default blue; drop the transparent sidebar rules | Same |
| **Menu**: app menu with Services, Hide, Show All | `menu/mod.rs` | Move Settings and Quit into a File menu | Same |
| **Fonts**: the system font (`-apple-system`, SF Pro) and `ui-monospace` (SF Mono), with `system-ui` and `Menlo` fallbacks | `theme.css` (`--font-ui`, `--font-mono`) | Falls back to the platform's UI font; bundle a font if it must match | Same |
| **Bundle targets**: `app` and `dmg`, signed ad hoc (`bundle.macOS.signingIdentity: "-"`), `icon.icns` | `tauri.conf.json` `bundle`; `package.json` `release`, `build:app`, `app` | `deb`, `appimage` | `nsis`, `msi` (`icon.ico` is already generated) |
| **Install and dev scripts** (zsh, `ln` into `~/Applications` and `~/.local/bin`, `codesign`, Swift, CoreGraphics, `screencapture`) | `scripts/install.sh`, `scripts/stop-app.sh`, `scripts/sign-app.sh`, `scripts/create-signing-cert.sh`, `scripts/dev/` | Link into `~/.local/bin` and a `.desktop` file; rewrite the dev scripts or use `bun tauri dev` | PowerShell |
| **Tests**: the setup pretends to be a Mac | `src/test-setup.ts` | Make the platform a parameter | Same |

A port is done when `bun run test`, `bun run lint`, `bun run test:db` and `bun run release` pass on the
target, and the app can: track the frontmost app and its window title, and mark idle, sleep and lock as away;
hand a second `timetracker` launch and every CLI command to the running instance; hide on close and come back
from the tray; start hidden at login; open the timer picker with the global shortcut; show its menus and
shortcuts with the right modifier; move the data folder.

## Common changes

| Goal | Start here |
|---|---|
| Add a command or shortcut | [frontend.md: commands](frontend.md#commands) |
| Add a menu item | [backend.md: native menu](backend.md#native-menu) |
| Add a setting | [backend.md: persistence](backend.md#persistence) |
| Add a Tauri command | [backend.md: Tauri commands](backend.md#tauri-commands) |
| Add a CLI subcommand | [backend.md: command tree](backend.md#command-tree) |
| Emit a new event | [backend.md: events](backend.md#events-to-the-page) |
| Change colors or schemes | [frontend.md: styling](frontend.md#styling), [ux.md](ux.md#appearance-and-tokens) |
| Add a capture source | [Another capture source](#another-capture-source) |
| Feed activity from another app | [README: Scripting](../README.md#scripting); no code change |
| Remove the MCP server | [Dropping the MCP server](#dropping-the-mcp-server) |
| Remove the tray | [Dropping the tray](#dropping-the-tray) |

### Another capture source

A new platform implements `capture::Source` ([capture.md](capture.md#source)): `snapshot(capture)`,
`idle_seconds`, `events` (a `broadcast` of `SystemEvent`s), `permission`, `request_permission`. Put it in
`capture/<platform>.rs` behind `#[cfg(target_os = "…")]`, return it from a `capture::new_source` for that
target, and keep `macos.rs` and the objc2 crates under `cfg(target_os = "macos")`. The tracker, exclusions,
private-window redaction and every view are untouched. Test the tracker rules with `FakeSource`, the source
by hand. Another *app* (a planner, a meeting tool) is not a source: it writes external activity through
`timetracker activity` or the socket ([ADR 0009](adr/0009-external-activity-with-a-source.md)).

### Dropping the MCP server

`git rm -r src-tauri/src/mcp`; delete `pub mod mcp;` and the `CliOutcome::Mcp` arm in `lib.rs`; the `Mcp`
variant of `Command` and of `CliOutcome` and its tests in `cli/mod.rs` (and the `claude mcp add` line in
`EXAMPLES`); `rmcp` in `Cargo.toml`; `McpSettings` and `Settings.mcp` in `store/settings/types.rs` (old files keep
the key harmlessly); `binary_path` in `db/commands.rs` `DataInfo`; on the page, the MCP rows of the Data page
in `src/components/settings/pages.tsx` (`mcpAddCommand`, "Expose activity to the MCP server") and their tests
in `DataPage.test.tsx`, plus `mcp`/`binaryPath` in `src/test/settings.ts` and `src/test/data.ts`. Run
`bun run types`, then `bun run lint && bun run test`, and drop the MCP sections of the README and
[backend.md](backend.md#mcp-server).

### Dropping the tray

The tray is the only way back to a hidden window besides a second launch, so decide first: either quit on
close (remove `prevent_close` in `app.rs`'s `WindowEvent::CloseRequested` handler and the `ExitRequested`
`prevent_exit`; tracking then stops with the window, against [ADR 0007](adr/0007-tracker-in-process-menu-bar-app.md)),
or keep running and show the window on a Dock click (`RunEvent::Reopen` → `window::show_main`). Then delete
`tray.rs`, `pub mod tray;` in `lib.rs`, `tray::install` in `app.rs`, `tray::sync` in `tracker/task.rs`, `tray::set_timer`
in `timers/mod.rs`, `src-tauri/icons/tray/`, and the `tray-icon` features of `tauri` and
`tauri-plugin-positioner` in `Cargo.toml`. The timer picker then opens at `Position::Center`
(`timers/picker.rs`); pause, resume and timers stay in the window, the palette and the CLI. Update the Tray
section of [backend.md](backend.md#tray).

## Conventions to keep

- **Rust owns the logic.** The frontend renders and forwards intent.
- **IPC only in `api.ts` files.**
- **Typed errors** cross the boundary as `{kind, message, code}`.
- **No network.** No HTTP client, no update check, CSP allows IPC only.
- **Activity is raw.** It is never edited; recordings are a separate layer with undoable operations.
- **Tests with every change**, through public interfaces ([`.claude/rules/testing.md`](../.claude/rules/testing.md)).
- **One commit per step or unit**, each revertible, never pushed ([`.claude/rules/commits.md`](../.claude/rules/commits.md)).
