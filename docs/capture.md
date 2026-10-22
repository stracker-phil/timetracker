# Capture and tracker

The tracker records what the user does into `activities` and `away_spans` ([data.md](data.md)). It runs as a
tokio task inside the app ([ADR 0007](adr/0007-tracker-in-process-menu-bar-app.md)) and reads the system
through the `Source` trait ([ADR 0008](adr/0008-capture-behind-a-source-trait.md)). The code is in
`src-tauri/src/capture/` and `src-tauri/src/tracker/`.

## Source

`capture/mod.rs`:

| Member | Returns |
|---|---|
| `snapshot(capture)` | `Option<Snapshot { at, app, bundle_id, pid, title, url, path, private }>`; `None` when nothing is frontmost. `capture` (`CaptureSettings`) lets the source skip reading addresses and paths that are turned off |
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
| Document path | `AXDocument` of the focused window (a `file://` URL), not for browsers | Accessibility |
| Browser address | AppleScript through `NSAppleScript`, see [Browsers](#browsers) | Automation, per browser |
| Idle time | `CGEventSourceSecondsSinceLastEventType(CombinedSessionState, kCGAnyInputEventType)` | none |
| Sleep, wake | `NSWorkspaceWillSleepNotification`, `NSWorkspaceDidWakeNotification` | none |
| Fast user switching | `NSWorkspaceSessionDidResignActiveNotification`, `…DidBecomeActive…` | none |
| Screen lock | `com.apple.screenIsLocked`, `com.apple.screenIsUnlocked` on `NSDistributedNotificationCenter` | none |
| Permission | `AXIsProcessTrusted`, and `AXIsProcessTrustedWithOptions({ kAXTrustedCheckOptionPrompt: true })` to prompt | – |

Attribute names are passed as `CFString`s (`"AXFocusedWindow"`, `"AXTitle"`, `"AXDocument"`). A failed title read gives an
empty title and is logged at Debug once per pid. `CGWindowListCopyWindowInfo` is never used, because its
titles need Screen Recording.

### Browsers

`capture/browsers.rs` knows the browsers and their scripts; `MacSource::browser_page` runs them.

| Bundle ids | Script (inside `with timeout of 1 second`) |
|---|---|
| `com.apple.Safari`, `com.apple.SafariTechnologyPreview` | `URL of current tab of front window` |
| `com.google.Chrome`, `com.google.Chrome.canary`, `com.brave.Browser`, `com.microsoft.edgemac`, `org.chromium.Chromium` | `(mode of front window) & linefeed & (URL of active tab of front window)` |
| `company.thebrowser.Browser` (Arc), `com.vivaldi.Vivaldi` | `URL of active tab of front window` |
| `org.mozilla.firefox` (and Developer Edition, Nightly) | none: Firefox is not scriptable, only titles are recorded |

The script runs on the main thread (`run_on_main_thread`), as `NSAppleScript` requires, and the poll waits
at most 1.5 s for it; a browser that answers later has no address this poll. The first script per browser
makes macOS ask for Automation (the text is `NSAppleEventsUsageDescription` in `Info.plist`). Outcomes:

| AppleScript result | Address | Logged |
|---|---|---|
| an answer | `url::normalize` of it | – |
| `-1743` (Automation refused) | none; the browser is not asked again this run | Info, once per browser |
| `-1728` (no window) | none | – |
| anything else | none | Debug, at most once an hour per browser |

`capture/url.rs`: `normalize` trims, drops the fragment, lowercases scheme and host (the `url` crate), and
drops empty and new-tab pages (`about:blank`, `chrome://newtab`, …); text that does not parse is kept only
when it starts with `http` or `file`. `domain` is the host without `www.`, used by exclusions and later by
rules and grouping. `file_path` turns `AXDocument`'s `file://` URL into a decoded path.

### Private windows

A private window is stored with `private = 1`, title `Private`, no address and no path; the app name stays.
Consecutive private snapshots of one app are one span, because they look the same after redaction.

| Browser | Recognized by |
|---|---|
| Chrome, Brave, Edge, Chromium | the script's window `mode` is `incognito`, or the title ends with ` (Incognito)`, ` (Private)` or ` - InPrivate` |
| Arc, Vivaldi | the title rule above |
| Firefox | the title ends with ` — Private Browsing` or ` (Private Browsing)` |
| Safari | a failing script other than `-1743` and `-1728` is taken as a private window. Not yet confirmed by hand on the current macOS: check whether a private window answers the script or fails, and update `browsers::failed` |

`capture/private.rs` holds the title rules; `browsers::parse` and `browsers::failed` the script ones.

### What is never captured

Keystrokes, screenshots, screen contents, audio, the clipboard and page content. Nothing leaves the Mac.
Private windows are recorded as "Private" without title or address. The only readings are the ones in the
table above.

## Tracker

`tracker/mod.rs` (the state machine's stages are in `stages.rs`, the open activity span in `activity.rs`, the
tokio task in `task.rs`). `Tracker` is managed as `Arc<Tracker>` and holds the `Db`, the source, the `tracker`
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
5. **Snapshot**: `None` closes the activity. An excluded snapshot (`filter::excluded`) closes it too and
   opens nothing, so the time stays a gap. Otherwise `filter::redact` applies the private-window rule and
   drops the address or path when `captureUrls` / `captureDocuments` are off. A snapshot whose app, bundle id, title, URL or path differ
   from the open row closes it at `now` and inserts a `tracker` row starting at `now`. An equal snapshot
   writes nothing: `end` is written only on a change.
6. **The 1 s rule**: a change less than 1 s after the open row started replaces that row's details in place
   (`activities::replace_open`, keeping its start), so windows passed on the way through ⌘Tab leave no rows.

Nothing is split at midnight; readers clip (`time::clip`, `activities::list_clipped`).

When the user deletes activity that holds the open row, `reset_span(deleted_ids, now)` drops it from the
tracker without writing, so the next tick opens a new row ([backend.md](backend.md#activity-deletion)).

### Pause

`pause(until, now)` takes `PauseUntil`: `{ minutes: n }`, `tomorrow` (the next local midnight) or `resumed`.
It closes the activity and any away span, and opens a `paused` span. Pausing again only moves `until`.
`resume(now)` closes the paused span and runs a step at once. Pauses come from the tray, the sidebar, the
palette (`tracker.pause`, `tracker.resume`) and the CLI (`timetracker pause`, `timetracker resume`).

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

### Exclusions

`Settings.capture.excludedApps` (bundle ids; an app without one by name) and `excludedDomains` (a domain
excludes itself and every subdomain). `Settings::sanitized` trims, lowercases and dedups both lists, and
reduces a pasted address to its host without `www.`. The tracker gets them through `Tracker::set_capture`
(at startup and on `update_settings`) and checks them before any write.

## Permission onboarding

While the state is `needsPermission`, the page shows `PermissionDialog` ([frontend.md](frontend.md#components))
and the tray shows the attention icon. "Check again" calls `request_permission`: the system prompt (once per
run), then an immediate step, so a granted permission starts tracking without a restart.

The grant belongs to the code signature, so dev builds need a stable signature to keep it
([testing.md](testing.md#accessibility-in-dev-builds)).

## Away prompt trigger

The tracker broadcasts an away span when it ends because the user came back (`Tracker::subscribe_away`):
input after idle, wake, unlock, session active, or resume after a pause. `away_prompt::should_prompt` then
asks only for spans at least `tracker.awayPromptMinutes` long while `tracker.awayPromptEnabled` is on, and
never for a pause, which the user started on purpose. Spans closed at startup recovery never prompt: the app
was not running when they ended. See [backend.md](backend.md#away-prompt).
