//! Fixtures for the recordings tests: a database with two projects and two tags, a service snapping to
//! 5 minutes, and clock times on a fixed June day.

use chrono::{TimeZone, Utc};
use tokio::sync::Mutex;

use super::undo::{UndoStack, UndoStatus};
use super::{RecordingInput, Recordings};
use crate::db::Db;
use crate::db::projects::Project;
use crate::db::recordings::{self, Recording};
use crate::db::tags::{self, Tag};
use crate::db::time::{Day, Ms};
use crate::error::AppError;
use crate::test_support::project;

pub const MIN: Ms = 60_000;
pub const HOUR: Ms = 60 * MIN;

/// `hour:minute` UTC on 2026-06-15. Its multiples of 5 minutes are round wall-clock minutes in every
/// time zone in use, so the tests do not depend on the machine's zone.
pub fn at(hour: i64, minute: i64) -> Ms {
    let midnight = Utc
        .with_ymd_and_hms(2026, 6, 15, 0, 0, 0)
        .single()
        .expect("a valid date")
        .timestamp_millis();
    midnight + hour * HOUR + minute * MIN
}

/// The first local midnight after `at(12, 0)`, and the two days around it.
pub fn midnight() -> (Day, Ms, Day) {
    let before = Day::of(at(12, 0));
    (before, before.next().bounds().0, before.next())
}

pub struct World {
    pub db: Db,
    pub undo: Mutex<UndoStack>,
    /// "Acme"
    pub project: Project,
    /// "Beta"
    pub other: Project,
    /// "alpha", sorts before `beta`.
    pub alpha: Tag,
    pub beta: Tag,
}

impl World {
    pub async fn new() -> Self {
        let db = Db::open_memory().await.expect("open db");
        let project_a = project(&db, "Acme").await;
        let other = project(&db, "Beta").await;
        let alpha = tags::insert(&db, "alpha").await.expect("insert tag");
        let beta = tags::insert(&db, "beta").await.expect("insert tag");
        Self {
            db,
            undo: Mutex::new(UndoStack::default()),
            project: project_a,
            other,
            alpha,
            beta,
        }
    }

    /// The service as the app builds it, snapping to 5 minutes.
    pub fn service(&self) -> Recordings<'_> {
        self.snapping(5)
    }

    pub fn snapping(&self, snap_minutes: u32) -> Recordings<'_> {
        Recordings {
            db: &self.db,
            undo: &self.undo,
            snap_minutes,
        }
    }

    /// For `project`, without note or tags.
    pub fn input(&self, start: Ms, end: Ms) -> RecordingInput {
        RecordingInput {
            project_id: self.project.id.clone(),
            start,
            end,
            note: String::new(),
            tag_ids: Vec::new(),
        }
    }

    /// Saves a recording of `project` through the service.
    pub async fn create(&self, start: Ms, end: Ms) -> Recording {
        self.service()
            .create(self.input(start, end), super::Overlap::Cancel)
            .await
            .expect("create recording")
            .value
    }

    /// Every stored recording by start.
    pub async fn rows(&self) -> Vec<Recording> {
        recordings::list_for_range(&self.db, Ms::MIN, Ms::MAX)
            .await
            .expect("list recordings")
    }

    pub async fn status(&self) -> UndoStatus {
        self.undo.lock().await.status()
    }
}

pub fn spans(rows: &[Recording]) -> Vec<(Ms, Ms)> {
    rows.iter()
        .map(|r| (r.start, r.end.expect("a closed recording")))
        .collect()
}

pub fn no_two_overlap(rows: &[Recording]) -> bool {
    rows.iter().enumerate().all(|(i, a)| {
        rows[i + 1..].iter().all(|b| {
            let (a_end, b_end) = (a.end.unwrap_or(Ms::MAX), b.end.unwrap_or(Ms::MAX));
            a_end <= b.start || b_end <= a.start
        })
    })
}

/// The form field of an `InvalidField` error.
pub fn field_of(err: &AppError) -> Option<String> {
    err.field().map(|f| f.path.to_string())
}

/// A stored-looking recording of project `p` without tags, for tests that never touch a database.
pub fn row(id: &str, start: Ms, end: Ms) -> Recording {
    Recording {
        id: id.into(),
        project_id: "p".into(),
        start,
        end: Some(end),
        note: String::new(),
        tag_ids: Vec::new(),
        created_at: 1_000,
        updated_at: 2_000,
    }
}
