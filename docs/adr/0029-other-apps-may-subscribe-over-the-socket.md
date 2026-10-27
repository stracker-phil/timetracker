# 0029. Other apps may subscribe to changes over the socket

## Status

Accepted (2026-10-27), implemented.

## Context

Other apps follow TimeTracker: a menu bar companion shows the running timer, a status line shows whether
tracking is paused. The socket answered one request per connection (ADR 0017), so such an app had to poll
`timer status` every few seconds, which is late and wakes both processes for nothing. The page already gets
every change as `timer_status`, `tracker_status` and `data_changed` events, but those reach only
TimeTracker's own windows.

## Decision

- **A `watch` request keeps its connection.** It is answered like any request, with the current timer and
  tracker, and then the app writes one JSON line per change on the same connection until the client closes
  it: `{"event":"timer"|"tracker"|"data","data":…}`. The payloads are the CLI's (`TimerState`,
  `TrackerStatus`, `DataChanged`), so a script reads a `timer` line as it reads `timer status --json`.
- **Changes only.** A timer line comes when the timer started, stopped or changed, not on the minute ticks; a
  tracker line when its state or permission changed, not on its heartbeat or a new current activity. A
  `data` line comes for every `data_changed`; a client reloads what it shows of that day.
- **The same emitters.** `events::data_changed`, `timers::publish` and the tracker's status task hand each
  change to `watch.rs` next to the page, so nothing new decides when something changed.
- **No replay.** A client that falls behind is disconnected, and when the app quits the connection ends
  (`timetracker watch` exits 2). A client reconnects and takes the reply as the current state; events in
  between are not kept.
- **Same trust as every request.** The socket stays 0600 with the peer-uid check; watching needs no setting.

## Consequences

- Other apps follow the timer within a second of a change, without polling.
- A watcher sees which day changed and the tracker's current app and window title, as `status --json`
  already shows any process of the user.
- The app keeps one task per watcher; reading the timer for a change costs a query only while someone
  watches.
- A client must handle the connection ending: it retries until TimeTracker runs again.
