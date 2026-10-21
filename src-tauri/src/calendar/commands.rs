use tauri::State;

use super::{CalendarInput, MonthView, WeekView, month, month_days, week, week_days};
use crate::day::commands::names;
use crate::db::time::{Day, now_ms};
use crate::db::{Db, activities, recordings};
use crate::error::AppResult;
use crate::state::AppState;

/// The rows of the days `from..=to`, read once for the whole range; activity only when the view shows
/// tracked time.
async fn input(db: &Db, (from, to): (Day, Day), with_activity: bool) -> AppResult<CalendarInput> {
    let (start, end) = (from.bounds().0, to.bounds().1);
    Ok(CalendarInput {
        today: Day::today(),
        now: now_ms(),
        recordings: recordings::list_for_range(db, start, end).await?,
        activities: if with_activity {
            activities::list_for_range(db, start, end).await?
        } else {
            Vec::new()
        },
        names: names(db).await?,
    })
}

#[tauri::command]
pub async fn week_view(state: State<'_, AppState>, day: Day) -> AppResult<WeekView> {
    Ok(week(
        day,
        &input(&state.data.db()?, week_days(day), true).await?,
    ))
}

#[tauri::command]
pub async fn month_view(state: State<'_, AppState>, day: Day) -> AppResult<MonthView> {
    Ok(month(
        day,
        &input(&state.data.db()?, month_days(day), false).await?,
    ))
}
