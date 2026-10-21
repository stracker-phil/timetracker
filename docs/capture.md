# Capture and tracker

The tracker records what the user does into `activities` and `away_spans` ([data.md](data.md)). It runs as a
tokio task inside the app ([ADR 0007](adr/0007-tracker-in-process-menu-bar-app.md)) and reads the system
through the `Source` trait ([ADR 0008](adr/0008-capture-behind-a-source-trait.md)). The code is in
`src-tauri/src/capture/` and `src-tauri/src/tracker/`.

## Source

`capture/mod.rs`:

| Member | Returns |
|---|---|
| `snapshot()` | `Option<Snapshot { at, app, bundle_id, pid, title, url, path, private }>`; `None` when nothing is frontmost. `url`, `path` and `private` stay empty until step 04 |
| `idle_seconds()` | seconds since the last keyboard, mouse or trackpad input |
| `events()` | a `broadcast::Receiver<SystemEvent>`: `Activated` (another app came to the front), `Sleep`, `Wake`, `Lock`, `Unlock`, `SessionInactive`, `SessionActive` |
| `permission()` | `PermissionStatus`: `granted`, `denied`, `unknown` |
| `request_permission()` | shows the system prompt, at most once per app run |

`capture::new_source(app)` returns the macOS source. `capture/fake.rs` (`cfg(test)` or feature `test-util`,
[ADR 0015](adr/0015-test-doubles-behind-test-util.md)) has `FakeSource`, which tests script.

### macOS APIs

`capture/macos.rs` is the only module that uses objc2. `MacSource::new(app)` registers its notification
observers on the main thread (`run_on_main_thread`), whose run loop Tauri runs. Each observer block only
sends a `SystemEvent`, and the observers stay registered for the app's lifetime.

| Reading | API | Permission |
|---|---|---|
| Frontmost app (name, bundle id, pid) | `NSWorkspace.frontmostApplication` | none |
| App switches | `NSWorkspaceDidActivateApplicationNotification` on the workspace notification center | none |
| Window title | `AXUIElementCreateApplication(pid)` → `AXFocusedWindow` → `AXTitle`, with a 0.5 s messaging timeout so a hung app cannot stall the poll | Accessibility |
| Idle time | `CGEventSourceSecondsSinceLastEventType(CombinedSessionState, kCGAnyInputEventType)` | none |
| Sleep, wake | `NSWorkspaceWillSleepNotification`, `NSWorkspaceDidWakeNotification` | none |
| Fast user switching | `NSWorkspaceSessionDidResignActiveNotification`, `…DidBecomeActive…` | none |
| Screen lock | `com.apple.screenIsLocked`, `com.apple.screenIsUnlocked` on `NSDistributedNotificationCenter` | none |
| Permission | `AXIsProcessTrusted`, and `AXIsProcessTrustedWithOptions({ kAXTrustedCheckOptionPrompt: true })` to prompt | – |

Attribute names are passed as `CFString`s (`"AXFocusedWindow"`, `"AXTitle"`). A failed title read gives an
empty title and is logged at Debug once per pid. `CGWindowListCopyWindowInfo` is never used, because its
titles need Screen Recording.

## Tracker

`tracker/mod.rs`. `Tracker` is managed as `Arc<Tracker>` and holds the `Db`, the source, the `tracker`
settings (`set_settings` replaces them; `update_settings` calls it), and its state behind one async mutex,
so ticks, events, pause and resume never interleave.

`TrackerStatus { state, current, permission, text }` is what the page and the tray see. `TrackerState` is
tagged by `kind`:

| State | Means |
|---|---|
| `running` | recording; `current` is the open activity row, or `null` when nothing is frontmost |
| `away { since, reason }` | an away span with `reason` `idle`, `sleep` or `lock` is open |
| `paused { until }` | a `paused` away span is open; `until: null` lasts until resumed |
| `needsPermission` | Accessibility is not granted; nothing is recorded |

### One step

`tick(now)` runs every `pollSeconds` (5 s while the permission is missing), and `on_event(event, now)` runs
for each `SystemEvent` after updating the asleep, locked and session flags. Both run the same step, whose
stages (`held_by_pause`, `held_by_permission`, `held_by_system`, `held_by_idle`) each either hold the tracker
or pass it on to recording:

1. **Paused**: nothing until `until` passes; then the paused span closes and the tracker runs again.
2. **Permission**: when not granted, the open activity and any away span close at `now`, the state is
   `needsPermission`, and the step ends. Once granted, the tracker runs again.
3. **Sleep, lock, session switch**: while one is active, the activity and any other away span close at
   `now` and a `sleep` or `lock` span opens there. Sleep wins over a lock. Fast user switching counts as
   `lock`. When the flags clear (wake, unlock), the span closes at `now`.
4. **Idle**: at `idle_seconds() >= idleMinutes × 60` the activity closes at `now − idle` (when input
   stopped) and an `idle` span opens there. It never starts before the tracker last started running
   (`running_since`), so it does not overlap an earlier pause or sleep. Input again closes it at `now`.
5. **Snapshot**: `None` closes the activity. A snapshot whose app, bundle id, title, URL or path differ
   from the open row closes it at `now` and inserts a `tracker` row starting at `now`. An equal snapshot
   writes nothing: `end` is written only on a change.
6. **The 1 s rule**: a change less than 1 s after the open row started replaces that row's details in place
   (`activities::replace_open`, keeping its start), so windows passed on the way through ⌘Tab leave no rows.

Nothing is split at midnight; readers clip (`time::clip`, `activities::list_clipped`).

### Pause

`pause(until, now)` takes `PauseUntil`: `{ minutes: n }`, `tomorrow` (the next local midnight) or `resumed`.
It closes the activity and any away span, and opens a `paused` span. Pausing again only moves `until`.
`resume(now)` closes the paused span and runs a step at once. Pauses come from the tray, the sidebar, the
palette (`tracker.pause`, `tracker.resume`) and, in step 05, the CLI.

### Startup and the loop

`tracker::start(app, tracker)` (called from `setup` when the data store is open) subscribes the tray and the
page to the statuses, then runs `recover(now)`.
Activity rows left open by a crash or forced quit get `end = start + pollSeconds`, because their real end
is unknown (it is written only on change). Open away spans end at `now`. Open rows of external sources stay
as they are.

Then it spawns `run`, which waits for the next tick or event in a `select!`, and supervises it: when `run`
panics, the error is logged and it restarts after 5 s, closing the open activity first. A failing step is
logged once per distinct error, not on every tick.

### Status

After every step the tracker builds a `TrackerStatus` and sends it on its broadcast channel when it differs
from the last one sent, or when the last one is 10 s old. `status()` returns the last one sent.
`tracker::start` passes each status to `tray::sync` and to the page as the `tracker_status` event; a lagging
receiver catches up with `status()`.

`status_text(state, current, now, tz)` (`tracker/status.rs`) is the one line shown in the tray and the
sidebar. The page never builds it.

| State | Text |
|---|---|
| running | `Tracking · Safari`, or `Tracking` with nothing in front |
| away | `Away since 10:42` |
| paused | `Paused until 11:15`, `Paused until tomorrow` (until the next local midnight), or `Paused` |
| needsPermission | `Needs permission` |

## Permission onboarding

While the state is `needsPermission`, the page shows `PermissionDialog` ([frontend.md](frontend.md#components))
and the tray shows the attention icon. "Check again" calls `request_permission`: the system prompt (once per
run), then an immediate step, so a granted permission starts tracking without a restart.

The grant belongs to the code signature, so dev builds need a stable signature to keep it
([testing.md](testing.md#accessibility-in-dev-builds)).
