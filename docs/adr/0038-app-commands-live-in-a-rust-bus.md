# 0038. App commands live in a Rust bus

## Status

Accepted (2026-11-04), implemented. Inherited from Taskboard ADR 0012 (itself from Daybook ADR 0017).

## Context

The same action was reachable from the menu bar, the tray, the page's keys, the palette, links and the CLI,
and each surface wired its own handler. The menu bar mirrored the page's commands by hand (`MENU_ITEMS`) and
emitted a `command` event the page ran; `menu::forward` special-cased Quit, Hide to Menu Bar, Settings and
Close Window so they worked without the page; the tray had its own ids and handlers for the same Settings,
Quit, timer and pause; links knew only `app:settings`, through an `IpcMessage` of their own. Tauri also hands
every menu event to every handler, so a tray click on Start Timer… ran twice. Preconditions drifted: Stop Timer
was greyed by the page's copy of the timer, the tray by Rust's.

## Decision

- App-level actions are commands of one Rust bus (`bus/`), each with a colon id (`app:settings`, `timer:stop`,
  `tracker:pause:60`), a title, an optional enabled and checked check and a handler, registered by its feature
  (`bus::register`, `timers::commands::register`, `tracker::commands::register`). A family is a prefix that
  takes an argument (`app:settings:<page>`, `tracker:pause:<length>`); an exact id wins, else the longest
  prefix. The bus refuses an unknown id and a disabled one before it runs.
- The menu bar, the tray (`menu::clicked`), the page (`run_command`), the palette, links and `timetracker run`
  run them by id; a failure from a click is an error toast.
- What depends on the view, the selection or the focus stays a page command with its dotted id (`nav.prev`),
  reached from Rust as `page:<id>`: the bus emits `page_command` to the window in front, else the main window,
  and the menu shows that window's report of what can run (`menu_sync`, kept per window).
- The page keeps a binding entry for each bus command with a key (`busCommands.ts`), so Settings → Shortcuts
  still rebinds it; overrides under the old ids move on read (`keybindings::RENAMED`).
- Links run only an allowlist (`deep_link::link_command`): showing, Settings, the timer, pausing and a few views.
- Rejected:
  - keeping every command in the page and special-casing the few that must work without it (the state before:
    each surface grew its own exceptions);
  - moving every command to Rust (the Day view's focus, selection and dialogs live in the page; Rust would need
    the page's state on every change);
  - one bus for app and page commands with the page reporting each command's state (the per-window report of
    the menu's page items already gives the menu what it needs).

## Consequences

- One place decides whether an app action can run, for every surface; Quit, Settings and the timer work while
  the page is broken or hidden.
- The tray and the menu bar share ids, so a click runs once.
- Ids change: stored key overrides migrate, but scripts and links must use the colon ids.
- A page command is only as available as its window's last report; a window that doesn't report hands the
  menu's page items to the main window.
