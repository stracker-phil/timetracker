//! Turns what the capture source reads into activity and away spans. `tick` (on a timer) and
//! `on_event` (sleep, lock, app switch) run the state machine; `pause` and `resume` come from
//! the user.

pub mod commands;
pub mod filter;
mod status;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use ts_rs::TS;

pub use status::status_text;

use crate::capture::{PermissionStatus, Snapshot, Source, SystemEvent};
use crate::db::Db;
use crate::db::activities::{self, ActivityRow, NewActivity};
use crate::db::away::{self, AwayReason, NewAway};
use crate::db::time::{Day, Ms, now_ms};
use crate::error::AppResult;
use crate::events::TRACKER_STATUS_EVENT;
use crate::lock::MutexExt;
use crate::store::settings::{CaptureSettings, TrackerSettings};
use crate::tray;

pub const TRACKER_SOURCE: &str = "tracker";
/// A window in front for less than this was passed on the way to another (⌘Tab) and is not kept.
const MIN_SPAN: Ms = 1_000;
/// The status goes out at least this often, so the shown activity stays fresh.
const HEARTBEAT: Ms = 10_000;
const PERMISSION_POLL: Duration = Duration::from_secs(5);
const RESTART_DELAY: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum TrackerState {
    Running,
    /// `until: None` lasts until resumed.
    Paused {
        until: Option<Ms>,
    },
    NeedsPermission,
    Away {
        since: Ms,
        reason: AwayReason,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct TrackerStatus {
    pub state: TrackerState,
    /// The open activity span, while running.
    pub current: Option<ActivityRow>,
    pub permission: PermissionStatus,
    /// One line for the tray and the sidebar.
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum PauseUntil {
    Minutes(u32),
    /// The next local midnight.
    Tomorrow,
    Resumed,
}

impl PauseUntil {
    /// `None` lasts until resumed.
    fn ends_at(self, now: Ms) -> Option<Ms> {
        match self {
            PauseUntil::Minutes(minutes) => Some(now + Ms::from(minutes) * 60_000),
            PauseUntil::Tomorrow => Some(Day::of(now).next().bounds().0),
            PauseUntil::Resumed => None,
        }
    }
}

struct Inner {
    state: TrackerState,
    current: Option<ActivityRow>,
    permission: PermissionStatus,
    /// When the tracker last started running; an idle span never starts before it.
    running_since: Ms,
    asleep: bool,
    locked: bool,
    session_inactive: bool,
    /// When `Tracker::latest` was last sent.
    published_at: Option<Ms>,
}

impl Inner {
    fn status(&self, now: Ms) -> TrackerStatus {
        TrackerStatus {
            text: status_text(&self.state, self.current.as_ref(), now, &chrono::Local),
            state: self.state.clone(),
            current: self.current.clone(),
            permission: self.permission,
        }
    }
}

pub struct Tracker {
    db: Db,
    source: Arc<dyn Source>,
    settings: Mutex<TrackerSettings>,
    capture: Mutex<CaptureSettings>,
    inner: tokio::sync::Mutex<Inner>,
    latest: Mutex<TrackerStatus>,
    statuses: broadcast::Sender<TrackerStatus>,
}

impl Tracker {
    pub fn new(
        db: Db,
        source: Arc<dyn Source>,
        settings: TrackerSettings,
        now: Ms,
    ) -> Arc<Tracker> {
        let inner = Inner {
            state: TrackerState::Running,
            current: None,
            permission: source.permission(),
            running_since: now,
            asleep: false,
            locked: false,
            session_inactive: false,
            published_at: None,
        };
        Arc::new(Tracker {
            latest: Mutex::new(inner.status(now)),
            db,
            source,
            settings: Mutex::new(settings),
            capture: Mutex::new(CaptureSettings::default()),
            inner: tokio::sync::Mutex::new(inner),
            statuses: broadcast::channel(16).0,
        })
    }

    /// The last status sent.
    pub fn status(&self) -> TrackerStatus {
        self.latest.lock_unpoisoned().clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TrackerStatus> {
        self.statuses.subscribe()
    }

    pub fn set_settings(&self, settings: TrackerSettings) {
        *self.settings.lock_unpoisoned() = settings;
    }

    pub fn set_capture(&self, capture: CaptureSettings) {
        *self.capture.lock_unpoisoned() = capture;
    }

    fn settings(&self) -> TrackerSettings {
        self.settings.lock_unpoisoned().clone()
    }

    /// Closes spans a crash or forced quit left open. The real end of an activity span is unknown
    /// (it is written on change only), so it ends one poll after its start; an away span ends now.
    pub async fn recover(&self, now: Ms) -> AppResult<()> {
        let poll = Ms::from(self.settings().poll_seconds) * 1_000;
        activities::close_stale(&self.db, TRACKER_SOURCE, poll).await?;
        away::close_open(&self.db, now).await?;
        Ok(())
    }

    pub async fn tick(&self, now: Ms) -> AppResult<()> {
        let mut inner = self.inner.lock().await;
        let result = self.step(&mut inner, now).await;
        self.publish(&mut inner, now);
        result
    }

    pub async fn on_event(&self, event: SystemEvent, now: Ms) -> AppResult<()> {
        let mut inner = self.inner.lock().await;
        match event {
            SystemEvent::Activated => {}
            SystemEvent::Sleep => inner.asleep = true,
            SystemEvent::Wake => inner.asleep = false,
            SystemEvent::Lock => inner.locked = true,
            SystemEvent::Unlock => inner.locked = false,
            SystemEvent::SessionInactive => inner.session_inactive = true,
            SystemEvent::SessionActive => inner.session_inactive = false,
        }
        let result = self.step(&mut inner, now).await;
        self.publish(&mut inner, now);
        result
    }

    /// Pausing again only moves the end; the `paused` away span stays the same.
    pub async fn pause(&self, until: PauseUntil, now: Ms) -> AppResult<TrackerStatus> {
        let until = until.ends_at(now);
        let mut inner = self.inner.lock().await;
        let result = match inner.state {
            TrackerState::Paused { .. } => Ok(()),
            _ => self.enter_away(&mut inner, now, AwayReason::Paused).await,
        };
        if result.is_ok() {
            inner.state = TrackerState::Paused { until };
        }
        self.publish(&mut inner, now);
        result.map(|()| self.status())
    }

    pub async fn resume(&self, now: Ms) -> AppResult<TrackerStatus> {
        let mut inner = self.inner.lock().await;
        let mut result = Ok(());
        if matches!(inner.state, TrackerState::Paused { .. }) {
            result = self.leave_away(&mut inner, now).await;
        }
        if result.is_ok() {
            result = self.step(&mut inner, now).await;
        }
        self.publish(&mut inner, now);
        result.map(|()| self.status())
    }

    /// Shows the system prompt (once per run), then checks again, so a granted permission starts
    /// tracking at once.
    pub async fn request_permission(&self, now: Ms) -> AppResult<PermissionStatus> {
        self.source.request_permission();
        self.tick(now).await?;
        Ok(self.status().permission)
    }

    /// Closes the open activity, for a restart after the loop stopped unexpectedly.
    async fn interrupt(&self, now: Ms) -> AppResult<()> {
        let mut inner = self.inner.lock().await;
        self.close_activity(&mut inner, now).await
    }

    /// Each stage either holds the tracker (paused, no permission, away) or passes it on to recording.
    async fn step(&self, inner: &mut Inner, now: Ms) -> AppResult<()> {
        if self.held_by_pause(inner, now).await?
            || self.held_by_permission(inner, now).await?
            || self.held_by_system(inner, now).await?
            || self.held_by_idle(inner, now).await?
        {
            return Ok(());
        }
        self.record(inner, now).await
    }

    async fn held_by_pause(&self, inner: &mut Inner, now: Ms) -> AppResult<bool> {
        let TrackerState::Paused { until } = inner.state else {
            return Ok(false);
        };
        if until.is_none_or(|until| until > now) {
            return Ok(true);
        }
        self.leave_away(inner, now).await?;
        Ok(false)
    }

    async fn held_by_permission(&self, inner: &mut Inner, now: Ms) -> AppResult<bool> {
        inner.permission = self.source.permission();
        if inner.permission != PermissionStatus::Granted {
            if inner.state != TrackerState::NeedsPermission {
                self.close_activity(inner, now).await?;
                away::close_open(&self.db, now).await?;
                inner.state = TrackerState::NeedsPermission;
            }
            return Ok(true);
        }
        if inner.state == TrackerState::NeedsPermission {
            inner.state = TrackerState::Running;
            inner.running_since = now;
        }
        Ok(false)
    }

    /// Sleep, lock and session switches.
    async fn held_by_system(&self, inner: &mut Inner, now: Ms) -> AppResult<bool> {
        if let Some(reason) = system_away(inner) {
            if !is_away_for(&inner.state, reason) {
                self.enter_away(inner, now, reason).await?;
            }
            return Ok(true);
        }
        if is_away_for(&inner.state, AwayReason::Sleep)
            || is_away_for(&inner.state, AwayReason::Lock)
        {
            self.leave_away(inner, now).await?;
        }
        Ok(false)
    }

    async fn held_by_idle(&self, inner: &mut Inner, now: Ms) -> AppResult<bool> {
        let idle = self.source.idle_seconds();
        if idle >= f64::from(self.settings().idle_minutes) * 60.0 {
            if !is_away_for(&inner.state, AwayReason::Idle) {
                // The time without input was already away.
                let since = (now - (idle * 1_000.0) as Ms).max(inner.running_since);
                self.enter_away(inner, since, AwayReason::Idle).await?;
            }
            return Ok(true);
        }
        if is_away_for(&inner.state, AwayReason::Idle) {
            self.leave_away(inner, now).await?;
        }
        Ok(false)
    }

    /// Keeps the open activity span in step with the frontmost window. Excluded windows leave a gap.
    async fn record(&self, inner: &mut Inner, now: Ms) -> AppResult<()> {
        let capture = self.capture.lock_unpoisoned().clone();
        let Some(snapshot) = self.source.snapshot(&capture) else {
            return self.close_activity(inner, now).await;
        };
        if filter::excluded(&snapshot, &capture) {
            return self.close_activity(inner, now).await;
        }
        let snapshot = filter::redact(snapshot, &capture);
        if let Some(current) = &inner.current {
            if same_window(current, &snapshot) {
                return Ok(());
            }
            if now - current.start < MIN_SPAN {
                let (id, start) = (current.id, current.start);
                if activities::replace_open(&self.db, id, new_activity(&snapshot, start)).await? {
                    inner.current = Some(row(id, start, snapshot));
                    return Ok(());
                }
            }
        }
        self.close_activity(inner, now).await?;
        let id = activities::insert(&self.db, new_activity(&snapshot, now)).await?;
        inner.current = Some(row(id, now, snapshot));
        Ok(())
    }

    async fn close_activity(&self, inner: &mut Inner, end: Ms) -> AppResult<()> {
        if inner.current.take().is_some() {
            activities::close_open(&self.db, TRACKER_SOURCE, end).await?;
        }
        Ok(())
    }

    /// Spans never overlap, so everything open ends where the away span starts.
    async fn enter_away(&self, inner: &mut Inner, start: Ms, reason: AwayReason) -> AppResult<()> {
        self.close_activity(inner, start).await?;
        away::close_open(&self.db, start).await?;
        away::insert(&self.db, NewAway { start, reason }).await?;
        inner.state = TrackerState::Away {
            since: start,
            reason,
        };
        Ok(())
    }

    async fn leave_away(&self, inner: &mut Inner, now: Ms) -> AppResult<()> {
        away::close_open(&self.db, now).await?;
        inner.state = TrackerState::Running;
        inner.running_since = now;
        Ok(())
    }

    fn publish(&self, inner: &mut Inner, now: Ms) {
        let status = inner.status(now);
        let mut latest = self.latest.lock_unpoisoned();
        let due = *latest != status || inner.published_at.is_none_or(|at| now - at >= HEARTBEAT);
        if !due {
            return;
        }
        *latest = status.clone();
        let _ = self.statuses.send(status);
        inner.published_at = Some(now);
    }

    fn poll_interval(&self, permission: PermissionStatus) -> Duration {
        if permission == PermissionStatus::Granted {
            Duration::from_secs(u64::from(self.settings().poll_seconds))
        } else {
            PERMISSION_POLL
        }
    }
}

/// Sleep wins over a lock: the Mac sleeps with the screen locked.
fn system_away(inner: &Inner) -> Option<AwayReason> {
    if inner.asleep {
        Some(AwayReason::Sleep)
    } else if inner.locked || inner.session_inactive {
        Some(AwayReason::Lock)
    } else {
        None
    }
}

fn is_away_for(state: &TrackerState, reason: AwayReason) -> bool {
    matches!(state, TrackerState::Away { reason: r, .. } if *r == reason)
}

fn same_window(current: &ActivityRow, snapshot: &Snapshot) -> bool {
    current.app == snapshot.app
        && current.bundle_id == snapshot.bundle_id
        && current.title == snapshot.title
        && current.url == snapshot.url
        && current.path == snapshot.path
}

fn new_activity(snapshot: &Snapshot, start: Ms) -> NewActivity {
    NewActivity {
        source: TRACKER_SOURCE.into(),
        start,
        app: snapshot.app.clone(),
        bundle_id: snapshot.bundle_id.clone(),
        title: snapshot.title.clone(),
        url: snapshot.url.clone(),
        path: snapshot.path.clone(),
        private: snapshot.private,
    }
}

fn row(id: i64, start: Ms, snapshot: Snapshot) -> ActivityRow {
    ActivityRow {
        id,
        source: TRACKER_SOURCE.into(),
        start,
        end: None,
        app: snapshot.app,
        bundle_id: snapshot.bundle_id,
        title: snapshot.title,
        url: snapshot.url,
        path: snapshot.path,
        private: snapshot.private,
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

#[cfg(test)]
mod tests;
