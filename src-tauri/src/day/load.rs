//! Reading a day's rows from the database and building its view; the commands and other layers call these.

use std::collections::HashMap;

use super::{DayInput, DayTotals, DayView, Names, ProjectRef, TagRef, ViewSettings, totals, view};
use crate::db::time::{Day, Ms};
use crate::db::{
    Db, activities, away, clients, dismissed, projects, recordings, stored_suggestions, tags,
};
use crate::error::AppResult;
use crate::rules::{self, matching::compile};
use crate::store::settings::Settings;

/// The day's view with its suggestions: the stored ones and the saved rules'.
pub async fn load(db: &Db, day: Day, settings: &Settings, now: Ms) -> AppResult<DayView> {
    let names = names(db).await?;
    let mut view = build(db, day, settings, now, names.clone()).await?;
    let rules = rules::store::active(db).await?;
    let dismissed = dismissed::list_for_day(db, day).await?;
    let stored = stored_suggestions::list_for_day(db, day).await?;
    view.suggestions = rules::day_suggestions(
        &view,
        &compile(&rules),
        &names,
        settings,
        &dismissed,
        &stored,
    );
    Ok(view)
}

/// The day's view without suggestions.
pub async fn view_of(db: &Db, day: Day, settings: &Settings, now: Ms) -> AppResult<DayView> {
    build(db, day, settings, now, names(db).await?).await
}

/// The day's totals, without building its view.
pub async fn totals_of(db: &Db, day: Day, now: Ms) -> AppResult<DayTotals> {
    Ok(totals(&input(db, day, now, Names::default()).await?))
}

/// Reads the rows that share time with the day (spans from the day before included) and builds its view.
async fn build(
    db: &Db,
    day: Day,
    settings: &Settings,
    now: Ms,
    names: Names,
) -> AppResult<DayView> {
    let input = input(db, day, now, names).await?;
    Ok(view(&input, &ViewSettings::from(settings)))
}

/// The rows that share time with the day.
async fn input(db: &Db, day: Day, now: Ms, names: Names) -> AppResult<DayInput> {
    let bounds = day.bounds();
    Ok(DayInput {
        day,
        bounds,
        now,
        activities: activities::list_for_range(db, bounds.0, bounds.1).await?,
        away: away::list_for_range(db, bounds.0, bounds.1).await?,
        recordings: recordings::list_for_range(db, bounds.0, bounds.1).await?,
        names,
    })
}

/// Archived projects and clients included: old recordings still show their names.
pub async fn names(db: &Db) -> AppResult<Names> {
    Ok(Names {
        projects: project_refs(db).await?,
        tags: tags::list(db)
            .await?
            .into_iter()
            .map(|t| {
                let tag = TagRef {
                    name: t.name,
                    emoji: t.emoji,
                };
                (t.id, tag)
            })
            .collect(),
    })
}

async fn project_refs(db: &Db) -> AppResult<HashMap<String, ProjectRef>> {
    let clients = clients::list(db, true).await?;
    Ok(projects::list(db, true)
        .await?
        .into_iter()
        .map(|p| {
            let client_name = p
                .client_id
                .as_ref()
                .and_then(|id| clients.iter().find(|c| &c.id == id))
                .map(|c| c.name.clone());
            let project = ProjectRef {
                name: p.name,
                client_name,
                color: p.color,
                shade: p.shade,
            };
            (p.id, project)
        })
        .collect())
}
