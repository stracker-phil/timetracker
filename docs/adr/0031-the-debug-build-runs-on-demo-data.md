# 0031. The debug build runs on demo data

## Status

Accepted (2026-11-04), implemented. Inherited from Taskboard ADR 0011.

## Context

Debug builds already had their own identifier, so their settings, data, socket, lock and logs were apart
from the release app's. Three things still leaked or collided. The debug tracker captured with
`capture/macos.rs` like the release one, so the debug database held the owner's real apps, window titles and
browser addresses, and screenshots could show them. Both builds claimed `timetracker://`, so a link could
open the debug app. Both registered the global timer shortcut ⌃⌥⌘T, and the second registration failed with
only a log line. The repo is public, so manual tests and committed screenshots must never show real data.

## Decision

- **Debug builds capture made-up windows.** `capture::new_source` returns `capture/demo.rs` `DemoSource`,
  which replays the showcase's windows (`test_support::showcase::demo_windows`) one every few minutes, with
  the permission granted and no idle time. The real source runs in a debug build only with
  `TIMETRACKER_REAL_CAPTURE=1` (`dev::real_capture`), off by default.
- **A new debug data folder starts on the showcase.** On launch, a debug build whose database holds no
  activity runs `seed_showcase` before the tracker starts (`dev::seed_first_run`). `timetracker dev seed`
  stays for re-seeding (`--yes`) and load data (`--years`).
- **Each build has its own URL scheme**: `timetracker` and `timetracker-debug` (`lib.rs` `LINK_SCHEME`,
  `tauri.debug.conf.json`). A link that starts the app is read with `get_current`.
- **The debug build registers no global shortcut by default** (`DEFAULT_TIMER_SHORTCUT`), so the release app
  keeps ⌃⌥⌘T. One can still be set in its Settings.
- `lib.rs` tests compare the name, identifier and scheme with both Tauri configs.
  `scripts/dev/screenshot.sh` captures only `TimeTracker (Debug)`.
- Rejected:
  - real capture in debug builds with an app or title filter, because whatever the filter misses lands in
    the database;
  - one shared data folder with a debug flag, because half-finished code would write the owner's data;
  - seeding only by hand, because a forgotten `dev seed` leaves an empty app that invites real use.

## Consequences

- Trying a change never shows or stores the owner's activity, and both apps run side by side.
- Capture code (`macos.rs`, `browsers.rs`) is exercised only in the release build or with the switch on, so
  a change there is checked with `TIMETRACKER_REAL_CAPTURE=1` or in the release app.
- The demo windows' bundle ids are mostly not installed, so their rows show initials instead of icons.
