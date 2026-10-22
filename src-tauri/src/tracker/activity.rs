//! The open activity span, kept in step with the frontmost window.

use super::{Inner, TRACKER_SOURCE, Tracker, filter};
use crate::capture::Snapshot;
use crate::db::activities::{self, ActivityRow, NewActivity};
use crate::db::time::Ms;
use crate::error::AppResult;
use crate::lock::MutexExt;

/// A window in front for less than this was passed on the way to another (⌘Tab) and is not kept.
pub(super) const MIN_SPAN: Ms = 1_000;

impl Tracker {
    /// Keeps the open activity span in step with the frontmost window. Excluded windows leave a gap.
    pub(super) async fn record(&self, inner: &mut Inner, now: Ms) -> AppResult<()> {
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
                if activities::replace_open(&self.db(), id, new_activity(&snapshot, start)).await? {
                    inner.current = Some(row(id, start, snapshot));
                    return Ok(());
                }
            }
        }
        self.close_activity(inner, now).await?;
        let id = activities::insert(&self.db(), new_activity(&snapshot, now)).await?;
        inner.current = Some(row(id, now, snapshot));
        Ok(())
    }

    pub(super) async fn close_activity(&self, inner: &mut Inner, end: Ms) -> AppResult<()> {
        if inner.current.take().is_some() {
            activities::close_open(&self.db(), TRACKER_SOURCE, end).await?;
        }
        Ok(())
    }
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
