//! A scripted source for tests: the test sets what the "system" shows and emits its events.

use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

use super::{PermissionStatus, Snapshot, Source, SystemEvent};
use crate::lock::MutexExt;

pub struct FakeSource {
    snapshot: Mutex<Option<Snapshot>>,
    idle_seconds: Mutex<f64>,
    permission: Mutex<PermissionStatus>,
    events: broadcast::Sender<SystemEvent>,
}

impl FakeSource {
    /// Permission granted, nothing frontmost, no idle time.
    pub fn new() -> Arc<FakeSource> {
        Arc::new(FakeSource {
            snapshot: Mutex::new(None),
            idle_seconds: Mutex::new(0.0),
            permission: Mutex::new(PermissionStatus::Granted),
            events: broadcast::channel(32).0,
        })
    }

    pub fn set_snapshot(&self, snapshot: Option<Snapshot>) {
        *self.snapshot.lock_unpoisoned() = snapshot;
    }

    /// Shows `app` with a window `title` in front.
    pub fn show(&self, app: &str, title: &str) {
        self.set_snapshot(Some(snapshot(app, title)));
    }

    pub fn set_idle(&self, seconds: f64) {
        *self.idle_seconds.lock_unpoisoned() = seconds;
    }

    pub fn set_permission(&self, permission: PermissionStatus) {
        *self.permission.lock_unpoisoned() = permission;
    }

    pub fn emit(&self, event: SystemEvent) {
        let _ = self.events.send(event);
    }
}

impl Source for FakeSource {
    fn snapshot(&self) -> Option<Snapshot> {
        self.snapshot.lock_unpoisoned().clone()
    }

    fn idle_seconds(&self) -> f64 {
        *self.idle_seconds.lock_unpoisoned()
    }

    fn events(&self) -> broadcast::Receiver<SystemEvent> {
        self.events.subscribe()
    }

    fn permission(&self) -> PermissionStatus {
        *self.permission.lock_unpoisoned()
    }

    fn request_permission(&self) {}
}

/// A snapshot of `app` (bundle id `test.<app>`) with a window `title`.
pub fn snapshot(app: &str, title: &str) -> Snapshot {
    Snapshot {
        at: 0,
        app: app.into(),
        bundle_id: Some(format!("test.{}", app.to_lowercase())),
        pid: 1,
        title: title.into(),
        url: None,
        path: None,
        private: false,
    }
}
