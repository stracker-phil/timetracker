//! Fixtures for unit tests and integration tests.

use crate::db::activities::{self, ActivityRow, NewActivity};
use crate::db::clients::{self, Client, ClientInput};
use crate::db::projects::{self, Project, ProjectInput};
use crate::db::recordings::{self, NewRecording, Recording};
use crate::db::time::Ms;
use crate::db::{DB_FILE, Db};
use crate::store::json_file::create_private_dir;

/// A fresh private directory standing in for the app's config dir; removed on drop.
pub fn temp_config_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");
    create_private_dir(dir.path()).expect("make temp dir private");
    dir
}

/// A migrated database in a file of a fresh temp dir, so WAL and migrations run as in the app.
/// Keep the dir alive as long as the database.
pub async fn temp_db() -> (tempfile::TempDir, Db) {
    let dir = temp_config_dir();
    let db = Db::open(&dir.path().join("data").join(DB_FILE))
        .await
        .expect("open temp db");
    (dir, db)
}

/// Inserts a `tracker` activity; `end: None` leaves it open.
pub async fn activity(db: &Db, start: Ms, end: Option<Ms>, app: &str, title: &str) -> ActivityRow {
    let id = activities::insert(
        db,
        NewActivity {
            source: "tracker".into(),
            start,
            app: app.into(),
            bundle_id: None,
            title: title.into(),
            url: None,
            path: None,
            private: false,
        },
    )
    .await
    .expect("insert activity");
    sqlx::query(r#"UPDATE activities SET "end" = ?2 WHERE id = ?1"#)
        .bind(id)
        .bind(end)
        .execute(db.pool())
        .await
        .expect("set activity end");
    sqlx::query_as("SELECT * FROM activities WHERE id = ?1")
        .bind(id)
        .fetch_one(db.pool())
        .await
        .expect("read activity")
}

/// A client with the next unused color.
pub async fn client(db: &Db, name: &str) -> Client {
    client_with_color(db, name, "").await
}

/// A client with the given accent color.
pub async fn client_with_color(db: &Db, name: &str, color: &str) -> Client {
    clients::insert(
        db,
        ClientInput {
            name: name.into(),
            color: color.into(),
        },
    )
    .await
    .expect("insert client")
}

/// A blue project without a client.
pub async fn project(db: &Db, name: &str) -> Project {
    projects::insert(
        db,
        ProjectInput {
            client_id: None,
            name: name.into(),
            color: "blue".into(),
            shade: None,
        },
    )
    .await
    .expect("insert project")
}

/// A recording without note or tag; `end: None` is the running timer.
pub async fn recording(db: &Db, project_id: &str, start: Ms, end: Option<Ms>) -> Recording {
    recordings::insert(
        db,
        NewRecording {
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project_id.into(),
            spans: vec![(start, end)],
            note: String::new(),
            tag_id: None,
        },
    )
    .await
    .expect("insert recording")
}
