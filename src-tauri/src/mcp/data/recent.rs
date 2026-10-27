//! The recent timers, as the tray offers them to start again.

use serde::Serialize;

use super::Core;
use crate::db::time::to_local_iso;
use crate::error::AppResult;
use crate::scripting::timer::{self, RecentTimerEntry};
use crate::timers::RECENT;

#[derive(Serialize)]
pub struct RecentTimerOut {
    pub project_id: String,
    pub project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    pub note: String,
    pub last_used_iso: String,
}

fn recent_out(entry: RecentTimerEntry) -> RecentTimerOut {
    RecentTimerOut {
        project_id: entry.project_id,
        project: entry.project,
        client: entry.client,
        tag: entry.tag,
        note: entry.note,
        last_used_iso: to_local_iso(entry.last_used_at),
    }
}

impl Core {
    /// `limit` defaults to the tray's.
    pub async fn recent_timers(&self, limit: Option<u32>) -> AppResult<Vec<RecentTimerOut>> {
        let limit = limit.unwrap_or(RECENT as u32);
        Ok(timer::recent(self.db().await?, limit)
            .await?
            .into_iter()
            .map(recent_out)
            .collect())
    }
}
