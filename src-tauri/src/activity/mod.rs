//! Activity writes that come from outside the tracker: the CLI, deep links and the page share these.

pub mod commands;
pub mod external;

use tauri::{AppHandle, Manager};

use crate::db::Db;
use crate::db::activities::ActivityRow;
use crate::db::time::{Ms, now_ms};
use crate::error::AppResult;
use crate::events::{self, DataKind};
use crate::state::AppState;
pub use external::ActivityInput;

fn db(app: &AppHandle) -> AppResult<Db> {
    app.state::<AppState>().data.db()
}

/// Tells the page about every day the rows touch.
fn notify(app: &AppHandle, rows: &[ActivityRow]) {
    for day in external::days_touched(rows, now_ms(), &chrono::Local) {
        events::data_changed(app, DataKind::Activity, Some(day));
    }
}

pub async fn sources(app: &AppHandle) -> AppResult<Vec<String>> {
    external::sources(&db(app)?).await
}

pub async fn add(app: &AppHandle, items: &[ActivityInput]) -> AppResult<Vec<i64>> {
    let rows = external::add_many(&db(app)?, items, now_ms()).await?;
    notify(app, &rows);
    Ok(rows.iter().map(|row| row.id).collect())
}

pub async fn start(app: &AppHandle, input: &ActivityInput) -> AppResult<ActivityRow> {
    let started = external::start(&db(app)?, input, now_ms()).await?;
    let mut written = vec![started.row.clone()];
    written.extend(started.closed);
    notify(app, &written);
    Ok(started.row)
}

pub async fn stop(
    app: &AppHandle,
    source: &str,
    end: Option<Ms>,
) -> AppResult<Option<ActivityRow>> {
    let stopped = external::stop(&db(app)?, source, end, now_ms()).await?;
    if let Some(row) = &stopped {
        notify(app, std::slice::from_ref(row));
    }
    Ok(stopped)
}
