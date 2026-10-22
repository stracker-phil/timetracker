//! Clients, projects and tags as the page edits them. The rules live in the repositories
//! (`db::clients`, `db::projects`, `db::tags`); these commands add the `data_changed` events.

use tauri::{AppHandle, Manager};

use crate::calendar::{month_range, week_days};
use crate::db::Db;
use crate::db::clients::{self, Client, ClientInput};
use crate::db::projects::{self, Project, ProjectInput, ProjectListItem, UsageWindows};
use crate::db::tags::{self, Tag, TagListItem};
use crate::db::time::{Day, Ms, now_ms};
use crate::error::AppResult;
use crate::events::{self, DataKind};
use crate::state::AppState;

fn db(app: &AppHandle) -> AppResult<Db> {
    app.state::<AppState>().data.db()
}

/// Every day may show the changed names and colors.
fn changed<T>(app: &AppHandle, kind: DataKind, value: T) -> AppResult<T> {
    events::data_changed(app, kind, None);
    Ok(value)
}

#[tauri::command]
pub async fn clients_list(app: AppHandle, include_archived: bool) -> AppResult<Vec<Client>> {
    clients::list(&db(&app)?, include_archived).await
}

/// `id: None` creates the client.
#[tauri::command]
pub async fn clients_save(
    app: AppHandle,
    id: Option<String>,
    input: ClientInput,
) -> AppResult<Client> {
    let db = db(&app)?;
    let client = match id {
        Some(id) => clients::update(&db, &id, input).await?,
        None => clients::insert(&db, input).await?,
    };
    changed(&app, DataKind::Projects, client)
}

#[tauri::command]
pub async fn clients_archive(app: AppHandle, id: String, archived: bool) -> AppResult<Client> {
    let client = clients::set_archived(&db(&app)?, &id, archived).await?;
    changed(&app, DataKind::Projects, client)
}

#[tauri::command]
pub async fn projects_list(
    app: AppHandle,
    include_archived: bool,
) -> AppResult<Vec<ProjectListItem>> {
    list_with_usage(&db(&app)?, include_archived, now_ms()).await
}

/// The projects with their recorded time in the week and the month `now` falls in.
pub async fn list_with_usage(
    db: &Db,
    include_archived: bool,
    now: Ms,
) -> AppResult<Vec<ProjectListItem>> {
    projects::list_items(db, include_archived, now, usage_windows(Day::of(now))).await
}

/// This week and this month of `today`, from the first day's midnight to the midnight after the last.
fn usage_windows(today: Day) -> UsageWindows {
    let span = |(first, last): (Day, Day)| (first.bounds().0, last.bounds().1);
    UsageWindows {
        week: span(week_days(today)),
        month: span(month_range(today)),
    }
}

/// `id: None` creates the project.
#[tauri::command]
pub async fn projects_save(
    app: AppHandle,
    id: Option<String>,
    input: ProjectInput,
) -> AppResult<Project> {
    let db = db(&app)?;
    let project = match id {
        Some(id) => projects::update(&db, &id, input).await?,
        None => projects::insert(&db, input).await?,
    };
    changed(&app, DataKind::Projects, project)
}

#[tauri::command]
pub async fn projects_archive(app: AppHandle, id: String, archived: bool) -> AppResult<Project> {
    let project = projects::set_archived(&db(&app)?, &id, archived).await?;
    changed(&app, DataKind::Projects, project)
}

#[tauri::command]
pub async fn projects_recent(app: AppHandle, limit: u32) -> AppResult<Vec<Project>> {
    projects::recent(&db(&app)?, limit).await
}

#[tauri::command]
pub async fn tags_list(app: AppHandle) -> AppResult<Vec<TagListItem>> {
    tags::list_with_usage(&db(&app)?).await
}

/// `id: None` creates the tag, or returns the one with that name.
#[tauri::command]
pub async fn tags_save(app: AppHandle, id: Option<String>, name: String) -> AppResult<Tag> {
    let db = db(&app)?;
    let tag = match id {
        Some(id) => tags::rename(&db, &id, &name).await?,
        None => tags::insert(&db, &name).await?,
    };
    changed(&app, DataKind::Tags, tag)
}

/// Also removes the tag from every recording.
#[tauri::command]
pub async fn tags_delete(app: AppHandle, id: String) -> AppResult<()> {
    tags::delete(&db(&app)?, &id).await?;
    changed(&app, DataKind::Tags, ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{project, recording};

    const HOUR: Ms = 3_600_000;

    fn at(day: &str, hour: Ms) -> Ms {
        Day::parse(day).unwrap().bounds().0 + hour * HOUR
    }

    #[tokio::test]
    async fn recorded_time_counts_from_monday_for_the_week_and_from_the_first_for_the_month() {
        for (now, recorded_days, week_hours, month_hours) in [
            (
                at("2026-10-25", 20),
                [
                    "2026-09-30",
                    "2026-10-01",
                    "2026-10-18",
                    "2026-10-19",
                    "2026-10-25",
                ],
                2,
                4,
            ),
            (
                at("2027-01-03", 20),
                [
                    "2026-12-27",
                    "2026-12-28",
                    "2026-12-31",
                    "2027-01-01",
                    "2027-01-03",
                ],
                4,
                2,
            ),
        ] {
            let db = Db::open_memory().await.unwrap();
            let busy = project(&db, "Busy").await;
            for day in recorded_days {
                recording(&db, &busy.id, at(day, 9), Some(at(day, 10))).await;
            }

            let items = list_with_usage(&db, false, now).await.unwrap();

            assert_eq!(
                (items[0].week_ms, items[0].month_ms),
                (week_hours * HOUR, month_hours * HOUR),
                "{now}"
            );
        }
    }
}
