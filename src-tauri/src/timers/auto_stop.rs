//! When the running timer is checked for its automatic stop (ADR 0042): each minute tick, the first one at
//! startup, and when the Mac wakes. The rule is `recordings::auto_stop`.

use tauri::AppHandle;
use tokio::sync::broadcast::error::RecvError;

use super::publish;
use crate::db::away::AwayReason;
use crate::db::time::{Ms, now_ms};
use crate::recordings::auto_stop::SLEEP_STOP_MS;
use crate::recordings::commands::changing;
use crate::tracker::Tracker;

/// Stops the running timer when it is due; `last_tick` is the previous minute tick, so a longer gap since
/// counts as a sleep the tracker may have missed.
pub async fn stop_if_due(app: &AppHandle, now: Ms, last_tick: Option<Ms>) {
    let unseen = last_tick
        .filter(|&tick| now - tick > SLEEP_STOP_MS)
        .map(|tick| (tick, now));
    if let Err(e) = changing(app, async |s| s.auto_stop_timer(now, unseen).await).await {
        log::warn!("Couldn't stop the timer on its own: {e}");
    }
}

/// Checks the timer each time the Mac wakes, so a timer left running stops at once.
pub fn watch_wake(app: &AppHandle, tracker: &Tracker) {
    let mut ended = tracker.subscribe_away();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            match ended.recv().await {
                Ok(span) if span.reason == AwayReason::Sleep => {}
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return,
            }
            stop_if_due(&app, now_ms(), None).await;
            publish(&app).await;
        }
    });
}
