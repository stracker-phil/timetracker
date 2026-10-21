# 0008. macOS capture sits behind a `Source` trait; objc2 bindings live in one module

## Status

Accepted (2026-10-21), implemented.

Amended 2026-10-21: `capture::macos` also imports `objc2-core-foundation` (for `CFString` attribute names).
The trait also has `permission()` and `request_permission()`, `snapshot` takes the `CaptureSettings`, and
`Snapshot` carries its time `at`. `capture::fake` compiles under `cfg(test)` or `test-util`.

## Context

Reading the frontmost app, the focused window's title, a browser's URL, a document's path, the idle time and
sleep/lock events needs Objective-C APIs (NSWorkspace, the Accessibility API, CGEventSource,
NSDistributedNotificationCenter, NSAppleScript). These are untestable in CI, main-thread sensitive and the
only non-portable part of the app. The tracker's state machine (when a span opens or closes, when the user is
away) must be tested without macOS.

## Decision

`capture::Source` is a trait: `snapshot() -> Option<Snapshot>` (app, bundle id, pid, title, url, path,
private), `idle_seconds() -> f64`, and a stream of `SystemEvent`s (sleep, wake, lock, unlock, session
changes). `capture::macos` is the only module that imports the `objc2` family (`objc2`, `objc2-foundation`,
`objc2-app-kit`, `objc2-application-services`, `objc2-core-graphics`, `block2`); `capture::fake` (behind
`test-util`) replays scripted snapshots and events. The tracker takes `Arc<dyn Source>`. Window titles come
from the Accessibility API (one permission); `CGWindowListCopyWindowInfo` is not used because titles there
need Screen Recording. Browser URLs come from Apple Events per browser (ADR text in step 04).

## Consequences

- The tracker's rules are unit tested with the fake source; the macOS module is tested by hand.
- A Linux or Windows port implements one trait.
- Every objc2 call that can hang (AX messaging to a frozen app) sets a timeout, so the poll never stalls.
