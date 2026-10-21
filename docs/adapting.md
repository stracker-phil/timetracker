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
| Product name | `TimeTracker` | `tauri.conf.json` `productName` and window `title`; `menu.rs` (app menu, `Quit TimeTracker`); `tray.rs` (tooltip, item labels); `index.html`; the toolbar; user-facing errors in `lib.rs` and `remote.rs`; the `TimeTracker.app` path in `scripts/install-app.sh`, `scripts/install-cli.sh`, `scripts/dev/launch.sh` and the `app` script of `package.json`; the window owner in `scripts/dev/screenshot.sh`; `Info.plist` usage strings |
| Bundle identifier | `com.philippstracker.timetracker` | `tauri.conf.json` `identifier`; `lib.rs` `IDENTIFIER` (a test compares both); `scripts/dev/js.sh` (socket path); `scripts/dev/launch.sh` (signing identifier). It also names the config dir and the log dir; changing it starts with empty settings and data |
| Binary and CLI command | `timetracker` | Cargo package `name` in `src-tauri/Cargo.toml`; `cli/mod.rs` `name` and `EXAMPLES`; `cli/reply.rs` (`TimeTracker <version>` line); the README Scripting examples; `scripts/install-cli.sh`; `scripts/dev/launch.sh`; `.claude/settings.json` (`pkill -x timetracker`) |
| Rust lib | `timetracker_lib` | `Cargo.toml` `[lib]`; `main.rs`; `tests/` |
| URL scheme | `timetracker://` | `tauri.conf.json` `plugins.deep-link.desktop.schemes` (the bundler writes it into `Info.plist`); `deep_link.rs` (`link_message`, error text); the README Scripting examples |
| Socket, lock | `timetracker.sock`, `timetracker.lock` | `lib.rs` `SOCKET_NAME`, `LOCK_NAME`; `scripts/dev/js.sh` (the socket) |
| Config dir | `~/Library/Application Support/com.philippstracker.timetracker/` | `lib.rs` `config_dir()` |
| Logs dir | `~/Library/Logs/com.philippstracker.timetracker/` | Tauri's `app_log_dir`, from the identifier |
| Env vars | `TIMETRACKER_WINDOW_FRAME`, `TIMETRACKER_BACKGROUND`, `TIMETRACKER_DEV_TERMINAL`, `TIMETRACKER_SIGN_IDENTITY` | `dev.rs`, `window.rs`, `scripts/dev/launch.sh` |
| Store on `window` | `window.timetracker` | `src/main.tsx`, `src/vite-env.d.ts`, the dev scripts |
| Icons | clock glyph | `src-tauri/icons/` (regenerate with `bun tauri icon <1024px png or svg>`); tray template images in `src-tauri/icons/tray/` |
| Author, license | Philipp Stracker, MIT | `Cargo.toml`, `package.json`, `LICENSE` |

## Platform-specific code

| Piece | Where | Linux | Windows |
|---|---|---|---|
| **Config dir** hard-coded to `~/Library/Application Support/<id>` | `lib.rs` `config_dir()`, `scripts/dev/js.sh` | `dirs::config_dir()`; it must resolve before Tauri starts, because the CLI and socket run first | `dirs::config_dir()` (`%APPDATA%\<id>`) |
| **Unix socket** for the single-instance handoff | `ipc.rs` | Works as is; mind the ~108-byte path limit | A named pipe (`tokio::net::windows::named_pipe`) with the same line protocol |
| **File permissions** (0600 files, 0700 dirs) | `ipc.rs`, `store/json_file.rs` | Works | Gate with `#[cfg(unix)]` |
| **Window chrome**: overlay title bar, `trafficLightPosition`, `transparent`, `windowEffects: ["sidebar"]`, `macOSPrivateApi` | `tauri.conf.json`; Cargo feature `macos-private-api` | Ignored; give `html`/`.app` an opaque `--chrome` | `windowEffects: ["mica"]`, or an opaque background |
| **Traffic-light space** and drag regions | the toolbar's left inset (`--traffic-lights`); `data-tauri-drag-region` in the toolbar | Remove the inset | Same |
| **Menu**: app menu with Services, Hide, Show All | `menu.rs` | Move Settings and Quit into a File menu | Same |
| **Permissions**: Accessibility (window titles, document paths) and Automation per browser (addresses; the prompt shows `NSAppleEventsUsageDescription` from `src-tauri/Info.plist`) | `capture/macos.rs`, `Info.plist` | No prompts; reading other windows needs X11 or compositor support | No prompts; browser addresses need UI Automation |
| **Capture**: frontmost app, window titles (Accessibility), idle time, sleep and lock notifications, the permission prompt | `capture/macos.rs` behind the `Source` trait; `tracker/commands.rs` `PRIVACY_SETTINGS` | A new `Source`: X11/Wayland for the active window, `org.freedesktop.ScreenSaver` for idle and lock, logind for sleep | A new `Source`: `GetForegroundWindow` + `GetWindowText`, `GetLastInputInfo`, `WM_POWERBROADCAST` / `WTS_SESSION_LOCK` |
| **Tray**: template image, menu on left click | `tray.rs` | Works with an `AppIndicator`; template tinting does not apply | Works; use a colored icon |
| **Launch at login**: `MacosLauncher::LaunchAgent` | `lib.rs` (autostart plugin) | the plugin writes an XDG autostart entry | the plugin writes a registry key |
| **Shortcuts**: `Mod` is ⌘, labels use ⌘⌥⇧ glyphs | `src/commands/registry.ts`, `keybindings.rs` `key_label` | Map `Mod` to Ctrl | Same |
| **Fonts**: bundled Geist and JetBrains Mono with `-apple-system` and `Menlo` fallbacks | `fonts.css`, `theme.css` | Works | Works |
| **Bundle targets**: only `app` | `tauri.conf.json` `bundle.targets`; `package.json` scripts | `deb`, `appimage` | `nsis`, `msi` |
| **Dev and install scripts** (zsh, Swift, CoreGraphics, `screencapture`) | `scripts/` | Rewrite or use `bun tauri dev` | PowerShell |
| **Capture** (later steps): objc2, Accessibility, Apple Events for browser URLs | `capture/macos.rs` | A new `Source` (X11/Wayland) | A new `Source` (Win32) |
| **Tests**: the setup pretends to be a Mac | `src/test-setup.ts` | Make the platform a parameter | Same |

A port is done when `bun run test`, `bun run lint` and `bun run tauri build` pass on the target, and the app
can: hand a second `timetracker` launch to the running window; hide on close and come back from the tray;
start hidden at login; show its menus and shortcuts with the right modifier.

## Common changes

| Goal | Start here |
|---|---|
| Add a command or shortcut | [frontend.md: commands](frontend.md#commands) |
| Add a menu item | [backend.md: native menu](backend.md#native-menu) |
| Add a setting | [backend.md: persistence](backend.md#persistence) |
| Add a Tauri command | [backend.md: Tauri commands](backend.md#tauri-commands) |
| Emit a new event | [backend.md: events](backend.md#events-to-the-page) |
| Change colors or schemes | [frontend.md: styling](frontend.md#styling), [ux.md](ux.md#color-schemes-and-tokens) |

## Conventions to keep

- **Rust owns the logic.** The frontend renders and forwards intent.
- **IPC only in `api.ts` files.**
- **Typed errors** cross the boundary as `{kind, message, code}`.
- **No network.** No HTTP client, no update check, CSP allows IPC only.
- **Activity is raw.** It is never edited; recordings are a separate layer with undoable operations.
- **Tests with every change**, through public interfaces ([`.claude/rules/testing.md`](../.claude/rules/testing.md)).
- **One commit per step or unit**, each revertible, never pushed ([`.claude/rules/commits.md`](../.claude/rules/commits.md)).
