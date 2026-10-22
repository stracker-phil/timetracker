//! The tokio task that runs the tracker for the app's lifetime.

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use super::Tracker;
use crate::capture::{PermissionStatus, SystemEvent};
use crate::db::time::{Ms, now_ms};
use crate::error::AppResult;
use crate::events::TRACKER_STATUS_EVENT;
use crate::tray;

const PERMISSION_POLL: Duration = Duration::from_secs(5);
const RESTART_DELAY: Duration = Duration::from_secs(5);

impl Tracker {
    /// Closes the open activity, for a restart after the loop stopped unexpectedly.
    async fn interrupt(&self, now: Ms) -> AppResult<()> {
        let mut inner = self.inner.lock().await;
        self.close_activity(&mut inner, now).await
    }

    fn poll_interval(&self, permission: PermissionStatus) -> Duration {
        if permission == PermissionStatus::Granted {
            Duration::from_secs(u64::from(self.settings().poll_seconds))
        } else {
            PERMISSION_POLL
        }
    }
}

/// Runs the tracker for the app's lifetime and passes its statuses to the tray and the page.
pub fn start(app: &AppHandle, tracker: Arc<Tracker>) {
    forward_statuses(app, &tracker);
    supervise(tracker);
}

fn forward_statuses(app: &AppHandle, tracker: &Arc<Tracker>) {
    let mut statuses = tracker.subscribe();
    let (app, tracker) = (app.clone(), Arc::clone(tracker));
    tauri::async_runtime::spawn(async move {
        loop {
            let status = match statuses.recv().await {
                Ok(status) => status,
                // Missed statuses are made good by the latest one.
                Err(RecvError::Lagged(_)) => tracker.status(),
                Err(RecvError::Closed) => return,
            };
            tray::sync(&app, &status);
            if let Err(e) = app.emit(TRACKER_STATUS_EVENT, &status) {
                log::warn!("Couldn't send the tracker status to the page: {e}");
            }
        }
    });
}

/// Closes stale spans once, then keeps the loop going and restarts it after a panic.
fn supervise(tracker: Arc<Tracker>) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = tracker.recover(now_ms()).await {
            log::error!("Couldn't close the spans left open by the last run: {e}");
        }
        loop {
            match tauri::async_runtime::spawn(run(tracker.clone())).await {
                Ok(()) => return,
                Err(e) => {
                    log::error!("The tracker stopped ({e}); restarting it in 5 s");
                    tokio::time::sleep(RESTART_DELAY).await;
                }
            }
        }
    });
}

async fn run(tracker: Arc<Tracker>) {
    let mut events = Some(tracker.source.events());
    let mut last_error: Option<String> = None;
    if let Err(e) = tracker.interrupt(now_ms()).await {
        log::warn!("Couldn't close the open activity: {e}");
    }
    loop {
        let wait = tracker.poll_interval(tracker.status().permission);
        let result = tokio::select! {
            () = tokio::time::sleep(wait) => tracker.tick(now_ms()).await,
            event = next_event(&mut events) => tracker.on_event(event, now_ms()).await,
        };
        report(result, &mut last_error);
    }
}

/// A failing database fails every tick, so each distinct error is logged once.
fn report(result: AppResult<()>, last_error: &mut Option<String>) {
    match result {
        Ok(()) => *last_error = None,
        Err(e) => {
            let message = e.to_string();
            if last_error.as_ref() != Some(&message) {
                log::warn!("The tracker couldn't record: {message}");
                *last_error = Some(message);
            }
        }
    }
}

/// Missed events (a lagging receiver) poll at once. The source owns the sender, so the channel
/// never closes while the tracker runs; if it did, only the timer would be left.
async fn next_event(events: &mut Option<broadcast::Receiver<SystemEvent>>) -> SystemEvent {
    loop {
        let Some(receiver) = events else {
            return std::future::pending().await;
        };
        match receiver.recv().await {
            Ok(event) => return event,
            Err(RecvError::Lagged(_)) => return SystemEvent::Activated,
            Err(RecvError::Closed) => *events = None,
        }
    }
}
