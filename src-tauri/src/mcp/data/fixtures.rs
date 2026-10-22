//! 2026-10-20 seeded with recordings, activity and away time, shared by the data tools' tests.

use std::path::Path;

use serde_json::Value;
use tempfile::TempDir;

use super::Core;
use crate::db::Db;
use crate::db::activities::{self, NewActivity};
use crate::db::away::{self, AwayReason, NewAway};
use crate::db::projects::{self, Project, ProjectInput};
use crate::db::recordings::{self, NewRecording, Recording};
use crate::db::tags::{self, Tag};
use crate::db::time::{Day, Ms, to_local_iso};
use crate::test_support::{client, project, temp_db};

pub const DAY: &str = "2026-10-20";

fn day() -> Day {
    Day::parse(DAY).unwrap()
}

pub fn at(hour: i64, minute: i64) -> Ms {
    day().bounds().0 + (hour * 60 + minute) * 60_000
}

pub fn iso(hour: i64, minute: i64) -> String {
    to_local_iso(at(hour, minute))
}

pub fn write_settings(dir: &Path, value: Value) {
    std::fs::write(dir.join("settings.json"), value.to_string()).unwrap();
}

pub async fn add_recording(
    db: &Db,
    project: &Project,
    (start, end): (Ms, Ms),
    note: &str,
    tag: Option<&Tag>,
) -> Recording {
    add_spanned(db, project, &[(start, end)], note, tag).await
}

pub async fn add_spanned(
    db: &Db,
    project: &Project,
    spans: &[(Ms, Ms)],
    note: &str,
    tag: Option<&Tag>,
) -> Recording {
    recordings::insert(
        db,
        NewRecording {
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project.id.clone(),
            spans: spans
                .iter()
                .map(|&(start, end)| (start, Some(end)))
                .collect(),
            note: note.into(),
            tag_id: tag.map(|t| t.id.clone()),
        },
    )
    .await
    .unwrap()
}

fn new_activity(source: &str, start: Ms, app: &str, title: &str) -> NewActivity {
    NewActivity {
        source: source.into(),
        start,
        app: app.into(),
        bundle_id: None,
        title: title.into(),
        url: None,
        path: None,
        private: false,
    }
}

async fn add_span(db: &Db, activity: NewActivity, end: Ms) {
    let id = activities::insert(db, activity).await.unwrap();
    sqlx::query(r#"UPDATE activities SET "end" = ?2 WHERE id = ?1"#)
        .bind(id)
        .bind(end)
        .execute(db.pool())
        .await
        .unwrap();
}

/// 2026-10-20: Portal (client Acme) 09:00-10:30 "fix login" and 13:00-14:00 "review", both tagged
/// development; Podcast 08:00-08:25 "outline" and 11:00-11:30 "edit episode", untagged. Tracker
/// activity 09:00-10:30, an external span 11:00-11:30, away 12:00-12:20.
pub struct Seed {
    pub dir: TempDir,
    pub db: Db,
    pub core: Core,
    pub portal: Project,
    pub podcast: Project,
    pub development: Tag,
    pub login: Recording,
}

pub async fn seeded() -> Seed {
    let (dir, db) = temp_db().await;
    let acme = client(&db, "Acme").await;
    let portal = projects::insert(
        &db,
        ProjectInput {
            client_id: Some(acme.id),
            name: "Portal".into(),
            color: "blue".into(),
            shade: None,
            billable: true,
        },
    )
    .await
    .unwrap();
    let podcast = project(&db, "Podcast").await;
    let development = tags::list(&db)
        .await
        .unwrap()
        .into_iter()
        .find(|t| t.name == "development")
        .unwrap();
    add_recording(&db, &podcast, (at(11, 0), at(11, 30)), "edit episode", None).await;
    let login = add_recording(
        &db,
        &portal,
        (at(9, 0), at(10, 30)),
        "fix login",
        Some(&development),
    )
    .await;
    add_recording(
        &db,
        &portal,
        (at(13, 0), at(14, 0)),
        "review",
        Some(&development),
    )
    .await;
    add_recording(&db, &podcast, (at(8, 0), at(8, 25)), "outline", None).await;

    add_span(
        &db,
        NewActivity {
            path: Some("/src/main.rs".into()),
            ..new_activity("tracker", at(9, 0), "Xcode", "main.rs")
        },
        at(10, 0),
    )
    .await;
    add_span(
        &db,
        NewActivity {
            url: Some("https://bank.example/accounts".into()),
            private: true,
            ..new_activity("tracker", at(10, 0), "Safari", "Accounts")
        },
        at(10, 30),
    )
    .await;
    add_span(
        &db,
        NewActivity {
            url: Some("https://plan.example/standup".into()),
            ..new_activity("external:planner", at(11, 0), "Planner", "Standup")
        },
        at(11, 30),
    )
    .await;
    away::insert(
        &db,
        NewAway {
            start: at(12, 0),
            reason: AwayReason::Idle,
        },
    )
    .await
    .unwrap();
    away::close_open(&db, at(12, 20)).await.unwrap();

    let core = Core::new(dir.path().to_path_buf());
    Seed {
        dir,
        db,
        core,
        portal,
        podcast,
        development,
        login,
    }
}

pub fn named<'a>(groups: &'a Value, name_key: &str, name: &str) -> &'a Value {
    groups
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g[name_key] == name)
        .unwrap_or_else(|| panic!("no group {name} in {groups}"))
}
