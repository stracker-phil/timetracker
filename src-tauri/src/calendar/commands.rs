use tauri::State;

use super::period::{Period, PeriodUnit, PickerMonth, period, picker_month};
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

/// The week of `day` with tracked time.
pub async fn load_week(db: &Db, day: Day) -> AppResult<WeekView> {
    Ok(week(day, &input(db, week_days(day), true).await?))
}

/// The month grid of `day`, recordings only.
pub async fn load_month(db: &Db, day: Day) -> AppResult<MonthView> {
    Ok(month(day, &input(db, month_days(day), false).await?))
}

#[tauri::command]
pub async fn week_view(state: State<'_, AppState>, day: Day) -> AppResult<WeekView> {
    load_week(&state.data.db()?, day).await
}

#[tauri::command]
pub async fn month_view(state: State<'_, AppState>, day: Day) -> AppResult<MonthView> {
    load_month(&state.data.db()?, day).await
}

#[tauri::command]
pub fn calendar_period(unit: PeriodUnit, day: Day) -> Period {
    period(unit, day, Day::today())
}

#[tauri::command]
pub fn calendar_picker(month: Day, selected: Option<(Day, Day)>) -> PickerMonth {
    picker_month(month, selected, Day::today())
}
