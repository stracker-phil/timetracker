//! Clients, projects and tags as the page edits them. The rules live in the repositories
//! (`db::clients`, `db::projects`, `db::tags`); these commands add the `data_changed` events.

use tauri::{AppHandle, Manager};

use crate::db::Db;
use crate::db::clients::{self, Client, ClientInput};
use crate::db::projects::{self, Project, ProjectInput, ProjectListItem};
use crate::db::tags::{self, Tag, TagListItem};
use crate::db::time::now_ms;
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
    projects::list_items(&db(&app)?, include_archived, now_ms()).await
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
