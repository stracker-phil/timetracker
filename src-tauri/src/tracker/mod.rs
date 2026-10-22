//! Turns what the capture source reads into activity and away spans. `tick` (on a timer) and
//! `on_event` (sleep, lock, app switch) run the state machine; `pause` and `resume` come from
//! the user.

mod activity;
pub mod commands;
pub mod filter;
mod stages;
mod status;
mod task;

use std::sync::{Arc, Mutex, RwLock};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use ts_rs::TS;

pub use status::status_text;
pub use task::start;

use crate::capture::{PermissionStatus, Source, SystemEvent};
use crate::db::Db;
use crate::db::activities::{self, ActivityRow};
use crate::db::away::{self, AwayReason, AwayRow};
use crate::db::time::{Day, Ms};
use crate::error::AppResult;
use crate::lock::{MutexExt, RwLockExt};
use crate::store::settings::{CaptureSettings, TrackerSettings};

pub const TRACKER_SOURCE: &str = "tracker";
/// The status goes out at least this often, so the shown activity stays fresh.
const HEARTBEAT: Ms = 10_000;
/// How often the tracker looks at the front window; app switches are seen at once either way.
pub(crate) const POLL_MS: Ms = 2_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
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
    /// Replaced when the data folder moves.
    db: RwLock<Db>,
    source: Arc<dyn Source>,
    settings: Mutex<TrackerSettings>,
    capture: Mutex<CaptureSettings>,
    inner: tokio::sync::Mutex<Inner>,
    latest: Mutex<TrackerStatus>,
    statuses: broadcast::Sender<TrackerStatus>,
    away_ended: broadcast::Sender<AwayRow>,
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
            db: RwLock::new(db),
            source,
            settings: Mutex::new(settings),
            capture: Mutex::new(CaptureSettings::default()),
            inner: tokio::sync::Mutex::new(inner),
            statuses: broadcast::channel(16).0,
            away_ended: broadcast::channel(4).0,
        })
    }

    /// The last status sent.
    pub fn status(&self) -> TrackerStatus {
        self.latest.lock_unpoisoned().clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TrackerStatus> {
        self.statuses.subscribe()
    }

    /// Every away span the tracker closes because the user is back (not those closed at startup).
    pub fn subscribe_away(&self) -> broadcast::Receiver<AwayRow> {
        self.away_ended.subscribe()
    }

    pub fn set_settings(&self, settings: TrackerSettings) {
        *self.settings.lock_unpoisoned() = settings;
    }

    pub fn set_capture(&self, capture: CaptureSettings) {
        *self.capture.lock_unpoisoned() = capture;
    }

    fn db(&self) -> Db {
        self.db.read_unpoisoned().clone()
    }

    /// Stops the tracker where it is, without an away span, until `Suspended::resume`: a tick, an
    /// event, pause or resume waits meanwhile. The open activity span stays open.
    pub async fn suspend(&self) -> Suspended<'_> {
        Suspended {
            tracker: self,
            _inner: self.inner.lock().await,
        }
    }

    fn settings(&self) -> TrackerSettings {
        self.settings.lock_unpoisoned().clone()
    }

    /// Closes spans a crash or forced quit left open. The real end of an activity span is unknown
    /// (it is written on change only), so it ends one poll after its start; an away span ends now.
    pub async fn recover(&self, now: Ms) -> AppResult<()> {
        activities::close_stale(&self.db(), TRACKER_SOURCE, POLL_MS).await?;
        away::close_open(&self.db(), now).await?;
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

    /// Forgets the open span when its row is among `deleted`, so the next tick opens a new one.
    pub async fn reset_span(&self, deleted: &[i64], now: Ms) {
        let mut inner = self.inner.lock().await;
        if inner
            .current
            .as_ref()
            .is_some_and(|current| deleted.contains(&current.id))
        {
            inner.current = None;
            self.publish(&mut inner, now);
        }
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
}

pub struct Suspended<'a> {
    tracker: &'a Tracker,
    _inner: tokio::sync::MutexGuard<'a, Inner>,
}

impl Suspended<'_> {
    /// Goes on recording into `db`, which holds the same rows as the one it suspended with.
    pub fn resume(self, db: Db) {
        *self.tracker.db.write_unpoisoned() = db;
    }
}

#[cfg(test)]
mod tests;
