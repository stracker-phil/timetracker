//! One step of the state machine: each stage holds the tracker (paused, no permission, away) or passes it
//! on to recording.

use super::{Inner, Tracker, TrackerState};
use crate::capture::PermissionStatus;
use crate::db::away::{self, AwayReason, AwayRow, NewAway};
use crate::db::time::Ms;
use crate::error::AppResult;

impl Tracker {
    pub(super) async fn step(&self, inner: &mut Inner, now: Ms) -> AppResult<()> {
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
                away::close_open(&self.db(), now).await?;
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

    /// Spans never overlap, so everything open ends where the away span starts.
    pub(super) async fn enter_away(
        &self,
        inner: &mut Inner,
        start: Ms,
        reason: AwayReason,
    ) -> AppResult<()> {
        self.close_activity(inner, start).await?;
        away::close_open(&self.db(), start).await?;
        away::insert(&self.db(), NewAway { start, reason }).await?;
        inner.state = TrackerState::Away {
            since: start,
            reason,
        };
        Ok(())
    }

    pub(super) async fn leave_away(&self, inner: &mut Inner, now: Ms) -> AppResult<()> {
        let open = away::open(&self.db()).await?;
        away::close_open(&self.db(), now).await?;
        if let Some(span) = open {
            let end = now.max(span.start);
            let _ = self.away_ended.send(AwayRow {
                end: Some(end),
                ..span
            });
        }
        inner.state = TrackerState::Running;
        inner.running_since = now;
        Ok(())
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
