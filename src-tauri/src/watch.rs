//! `timetracker watch`: other processes subscribe to changes over the socket (ADR 0029). The app sends
//! every change to `Watchers`; each watching connection writes it as one `WatchEvent` JSON line until
//! its client disconnects.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::OwnedWriteHalf;
use tokio::sync::broadcast::{self, error::RecvError};

use crate::capture::PermissionStatus;
use crate::db::time::now_ms;
use crate::events::{DataChanged, DataKind};
use crate::lock::MutexExt;
use crate::scripting::timer::{self, TimerState};
use crate::state::AppState;
use crate::tracker::{Tracker, TrackerState, TrackerStatus};

/// Events a slow client may fall behind by before it is disconnected.
const BACKLOG: usize = 64;

/// One line of `watch`: `{"event":"timer","data":{...}}`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum WatchEvent {
    /// The timer after it started, stopped or changed; not the minute ticks.
    Timer(TimerState),
    Data(DataChanged),
    /// The tracker after its state or permission changed; not a new current activity.
    Tracker(TrackerStatus),
}

/// The last timer and tracker sent, so a repeat is left out.
#[derive(Default)]
struct Last {
    timer: Option<TimerState>,
    tracker: Option<(TrackerState, PermissionStatus)>,
}

impl Last {
    /// Records `event` and tells whether it says nothing new.
    fn repeats(&mut self, event: &WatchEvent) -> bool {
        match event {
            WatchEvent::Timer(state) => {
                let key = TimerState {
                    elapsed_ms: None,
                    ..state.clone()
                };
                self.timer.replace(key.clone()) == Some(key)
            }
            WatchEvent::Tracker(status) => {
                let key = (status.state.clone(), status.permission);
                self.tracker.replace(key.clone()) == Some(key)
            }
            WatchEvent::Data(_) => false,
        }
    }
}

/// The subscriptions of the watching connections.
#[derive(Clone)]
pub struct Watchers {
    events: broadcast::Sender<WatchEvent>,
    last: Arc<Mutex<Last>>,
}

impl Default for Watchers {
    fn default() -> Self {
        Self {
            events: broadcast::channel(BACKLOG).0,
            last: Arc::default(),
        }
    }
}

impl Watchers {
    /// Forgets what was sent: the new watcher gets the current state in its reply instead.
    pub fn subscribe(&self) -> broadcast::Receiver<WatchEvent> {
        *self.last.lock_unpoisoned() = Last::default();
        self.events.subscribe()
    }

    pub fn is_watched(&self) -> bool {
        self.events.receiver_count() > 0
    }

    /// Sends `event` to every watcher unless it repeats the last of its kind.
    pub fn send(&self, event: WatchEvent) {
        if !self.is_watched() || self.last.lock_unpoisoned().repeats(&event) {
            return;
        }
        let _ = self.events.send(event);
    }
}

fn watchers(app: &AppHandle) -> Option<Watchers> {
    app.try_state::<Watchers>()
        .map(|w| w.inner().clone())
        .filter(Watchers::is_watched)
}

/// Sends the running timer to the watchers; read only while someone watches.
pub async fn timer_changed(app: &AppHandle) {
    let Some(watchers) = watchers(app) else {
        return;
    };
    let Ok(db) = app.state::<AppState>().data.db() else {
        return;
    };
    match timer::state(&db, now_ms()).await {
        Ok(state) => watchers.send(WatchEvent::Timer(state)),
        Err(e) => log::warn!("Couldn't read the timer for the watchers: {e}"),
    }
}

/// A recordings change may start, stop or edit the timer too, so the timer follows it.
pub fn data_changed(app: &AppHandle, changed: &DataChanged) {
    let Some(watchers) = watchers(app) else {
        return;
    };
    watchers.send(WatchEvent::Data(changed.clone()));
    if changed.kind == DataKind::Recordings {
        let app = app.clone();
        tauri::async_runtime::spawn(async move { timer_changed(&app).await });
    }
}

pub fn tracker_changed(app: &AppHandle, status: &TrackerStatus) {
    if let Some(watchers) = watchers(app) {
        watchers.send(WatchEvent::Tracker(status.clone()));
    }
}

/// The reply to `watch`: the timer, and the tracker while it runs, as events.
pub async fn current(app: &AppHandle) -> Vec<WatchEvent> {
    let timer = match app.state::<AppState>().data.db() {
        Ok(db) => timer::state(&db, now_ms()).await.unwrap_or_default(),
        Err(_) => TimerState::default(),
    };
    let tracker = app
        .try_state::<Arc<Tracker>>()
        .map(|tracker| WatchEvent::Tracker(tracker.status()));
    std::iter::once(WatchEvent::Timer(timer))
        .chain(tracker)
        .collect()
}

/// Writes every event as a line until the client disconnects, falls behind or the app quits. What the
/// client sends meanwhile is ignored.
pub async fn stream(
    mut events: broadcast::Receiver<WatchEvent>,
    mut read: impl AsyncRead + Unpin,
    mut write: OwnedWriteHalf,
) {
    let mut ignored = [0u8; 256];
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Ok(event) => {
                    let Ok(mut line) = serde_json::to_string(&event) else {
                        continue;
                    };
                    line.push('\n');
                    if write.write_all(line.as_bytes()).await.is_err() {
                        return;
                    }
                }
                Err(RecvError::Lagged(missed)) => {
                    log::warn!("A watcher fell {missed} events behind; disconnecting it");
                    return;
                }
                Err(RecvError::Closed) => return,
            },
            read = read.read(&mut ignored) => {
                if matches!(read, Ok(0) | Err(_)) {
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::activities::ActivityRow;
    use crate::db::time::Day;
    use tokio::sync::broadcast::error::TryRecvError;

    fn timer(elapsed_ms: Option<i64>) -> TimerState {
        TimerState {
            running: true,
            recording_id: Some("r1".into()),
            project_id: Some("p1".into()),
            project: Some("acme".into()),
            client: Some("ACME Inc".into()),
            tag_id: None,
            tag: None,
            note: Some("fix login".into()),
            started_at: Some(1_000),
            started_at_iso: Some("2026-10-21T09:00:00+02:00".into()),
            elapsed_ms,
        }
    }

    fn activity(title: &str) -> ActivityRow {
        ActivityRow {
            id: 1,
            source: "tracker".into(),
            start: 1_000,
            end: None,
            app: "Safari".into(),
            bundle_id: None,
            title: title.into(),
            url: None,
            path: None,
            private: false,
        }
    }

    fn tracker(state: TrackerState, current: Option<ActivityRow>, text: &str) -> TrackerStatus {
        TrackerStatus {
            state,
            current,
            permission: PermissionStatus::Granted,
            text: text.into(),
        }
    }

    fn data(kind: DataKind, day: Option<&str>) -> WatchEvent {
        WatchEvent::Data(DataChanged {
            kind,
            day: day.map(|d| Day::parse(d).unwrap()),
        })
    }

    fn received(events: &mut broadcast::Receiver<WatchEvent>) -> Vec<WatchEvent> {
        std::iter::from_fn(|| events.try_recv().ok()).collect()
    }

    #[test]
    fn watchers_are_watched_only_while_a_subscriber_lives() {
        let watchers = Watchers::default();
        assert!(!watchers.is_watched());

        let events = watchers.subscribe();
        assert!(watchers.is_watched());

        drop(events);
        assert!(!watchers.is_watched());
    }

    #[test]
    fn sending_without_a_subscriber_reaches_nobody_and_does_not_fail() {
        let watchers = Watchers::default();
        watchers.send(WatchEvent::Timer(timer(Some(1))));

        let mut events = watchers.subscribe();
        assert_eq!(received(&mut events), vec![]);
    }

    #[test]
    fn every_subscriber_gets_each_event() {
        let watchers = Watchers::default();
        let mut first = watchers.subscribe();
        let mut second = watchers.subscribe();

        watchers.send(data(DataKind::Projects, None));

        assert_eq!(received(&mut first), vec![data(DataKind::Projects, None)]);
        assert_eq!(received(&mut second), vec![data(DataKind::Projects, None)]);
    }

    #[test]
    fn a_timer_that_differs_only_in_elapsed_time_is_left_out() {
        let watchers = Watchers::default();
        let mut events = watchers.subscribe();

        watchers.send(WatchEvent::Timer(timer(Some(1_000))));
        watchers.send(WatchEvent::Timer(timer(Some(61_000))));
        watchers.send(WatchEvent::Timer(TimerState::default()));

        assert_eq!(
            received(&mut events),
            vec![
                WatchEvent::Timer(timer(Some(1_000))),
                WatchEvent::Timer(TimerState::default()),
            ]
        );
    }

    #[test]
    fn a_changed_timer_note_is_sent() {
        let watchers = Watchers::default();
        let mut events = watchers.subscribe();
        let renamed = TimerState {
            note: Some("review".into()),
            ..timer(Some(2_000))
        };

        watchers.send(WatchEvent::Timer(timer(Some(1_000))));
        watchers.send(WatchEvent::Timer(renamed.clone()));

        assert_eq!(
            received(&mut events),
            vec![
                WatchEvent::Timer(timer(Some(1_000))),
                WatchEvent::Timer(renamed),
            ]
        );
    }

    #[test]
    fn a_tracker_with_the_same_state_and_permission_is_left_out() {
        let watchers = Watchers::default();
        let mut events = watchers.subscribe();
        let first = tracker(TrackerState::Running, Some(activity("Inbox")), "Safari");
        let new_activity = tracker(
            TrackerState::Running,
            Some(activity("Docs")),
            "Safari: Docs",
        );

        watchers.send(WatchEvent::Tracker(first.clone()));
        watchers.send(WatchEvent::Tracker(new_activity));

        assert_eq!(received(&mut events), vec![WatchEvent::Tracker(first)]);
    }

    #[test]
    fn a_tracker_whose_state_or_permission_changes_is_sent() {
        let watchers = Watchers::default();
        let mut events = watchers.subscribe();
        let running = tracker(TrackerState::Running, None, "Tracking");
        let paused = tracker(TrackerState::Paused { until: None }, None, "Paused");
        let denied = TrackerStatus {
            permission: PermissionStatus::Denied,
            ..paused.clone()
        };

        for status in [&running, &paused, &denied] {
            watchers.send(WatchEvent::Tracker(status.clone()));
        }

        assert_eq!(
            received(&mut events),
            vec![
                WatchEvent::Tracker(running),
                WatchEvent::Tracker(paused),
                WatchEvent::Tracker(denied),
            ]
        );
    }

    #[test]
    fn data_events_are_never_left_out() {
        let watchers = Watchers::default();
        let mut events = watchers.subscribe();
        let changed = data(DataKind::Recordings, Some("2026-10-21"));

        watchers.send(changed.clone());
        watchers.send(changed.clone());

        assert_eq!(received(&mut events), vec![changed.clone(), changed]);
    }

    #[test]
    fn a_new_subscriber_makes_the_next_identical_event_go_out_again() {
        let watchers = Watchers::default();
        let mut first = watchers.subscribe();
        let status = tracker(TrackerState::Running, None, "Tracking");
        watchers.send(WatchEvent::Timer(timer(Some(1))));
        watchers.send(WatchEvent::Tracker(status.clone()));
        assert_eq!(received(&mut first).len(), 2);

        let mut second = watchers.subscribe();
        watchers.send(WatchEvent::Timer(timer(Some(2))));
        watchers.send(WatchEvent::Tracker(status.clone()));

        let expected = vec![
            WatchEvent::Timer(timer(Some(2))),
            WatchEvent::Tracker(status),
        ];
        assert_eq!(received(&mut first), expected);
        assert_eq!(received(&mut second), expected);
    }

    #[test]
    fn a_subscriber_that_falls_more_than_the_backlog_behind_lags() {
        let watchers = Watchers::default();
        let mut events = watchers.subscribe();

        for _ in 0..=BACKLOG {
            watchers.send(data(DataKind::Activity, None));
        }

        assert_eq!(events.try_recv(), Err(TryRecvError::Lagged(1)));
    }

    #[test]
    fn events_have_their_wire_shape_and_round_trip() {
        let cases = [
            (
                WatchEvent::Timer(timer(Some(5_000))),
                serde_json::json!({
                    "event": "timer",
                    "data": {
                        "running": true, "recordingId": "r1", "projectId": "p1",
                        "project": "acme", "client": "ACME Inc", "tagId": null, "tag": null,
                        "note": "fix login", "startedAt": 1000,
                        "startedAtIso": "2026-10-21T09:00:00+02:00", "elapsedMs": 5000
                    }
                }),
            ),
            (
                data(DataKind::Recordings, Some("2026-10-21")),
                serde_json::json!({
                    "event": "data",
                    "data": { "kind": "recordings", "day": "2026-10-21" }
                }),
            ),
            (
                data(DataKind::Settings, None),
                serde_json::json!({ "event": "data", "data": { "kind": "settings", "day": null } }),
            ),
            (
                WatchEvent::Tracker(tracker(
                    TrackerState::Paused { until: Some(9_000) },
                    None,
                    "Paused",
                )),
                serde_json::json!({
                    "event": "tracker",
                    "data": {
                        "state": { "kind": "paused", "until": 9000 },
                        "current": null, "permission": "granted", "text": "Paused"
                    }
                }),
            ),
        ];
        for (event, wire) in cases {
            assert_eq!(serde_json::to_value(&event).unwrap(), wire, "{event:?}");
            assert_eq!(
                serde_json::from_value::<WatchEvent>(wire.clone()).unwrap(),
                event,
                "{wire}"
            );
        }
    }
}
