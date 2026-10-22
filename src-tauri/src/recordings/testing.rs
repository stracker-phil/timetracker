//! Fixtures for the recordings tests: a database with two projects and two tags, a service snapping to
//! 5 minutes, and clock times on a fixed June day.

use chrono::{TimeZone, Utc};
use tokio::sync::Mutex;

use super::undo::{UndoStack, UndoStatus};
use super::{RecordingInput, Recordings, Span};
use crate::db::Db;
use crate::db::projects::Project;
use crate::db::recordings::{self, Recording, RecordingPatch, RecordingSpan};
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

    /// For `project`, one span, without note or tag.
    pub fn input(&self, start: Ms, end: Ms) -> RecordingInput {
        RecordingInput {
            project_id: self.project.id.clone(),
            spans: vec![Span { start, end }],
            note: String::new(),
            tag_id: None,
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

    /// Saves a recording of `project` with several spans through the service.
    pub async fn create_spans(&self, spans: &[(Ms, Ms)]) -> Recording {
        let input = RecordingInput {
            spans: spans
                .iter()
                .map(|&(start, end)| Span { start, end })
                .collect(),
            ..self.input(0, 0)
        };
        self.service()
            .create(input, super::Overlap::Cancel)
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

/// The closed spans of one recording, by start.
pub fn times(recording: &Recording) -> Vec<(Ms, Ms)> {
    spans(std::slice::from_ref(recording))
}

/// The span ids of one recording, by start.
pub fn span_ids(recording: &Recording) -> Vec<String> {
    recording.spans.iter().map(|s| s.id.clone()).collect()
}

/// Every span of `rows`, closed, by start.
pub fn spans(rows: &[Recording]) -> Vec<(Ms, Ms)> {
    let mut spans: Vec<(Ms, Ms)> = rows
        .iter()
        .flat_map(|r| &r.spans)
        .map(|s| (s.start, s.end.expect("a closed span")))
        .collect();
    spans.sort_unstable();
    spans
}

/// No span of one recording shares time with a span of another.
pub fn no_two_overlap(rows: &[Recording]) -> bool {
    let spans_of = |r: &Recording| -> Vec<(Ms, Ms)> {
        r.spans
            .iter()
            .map(|s| (s.start, s.end.unwrap_or(Ms::MAX)))
            .collect()
    };
    rows.iter().enumerate().all(|(i, a)| {
        rows[i + 1..].iter().all(|b| {
            spans_of(a).iter().all(|&(a_start, a_end)| {
                spans_of(b)
                    .iter()
                    .all(|&(b_start, b_end)| a_end <= b_start || b_end <= a_start)
            })
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
        note: String::new(),
        tag_id: None,
        created_at: 1_000,
        updated_at: 2_000,
        spans: vec![RecordingSpan {
            id: format!("{id}-span"),
            start,
            end: Some(end),
        }],
    }
}

pub fn no_status() -> UndoStatus {
    UndoStatus {
        undo_label: None,
        redo_label: None,
    }
}

pub fn undo_label(label: &str) -> UndoStatus {
    UndoStatus {
        undo_label: Some(label.into()),
        redo_label: None,
    }
}

pub fn patch(f: impl FnOnce(&mut RecordingPatch)) -> RecordingPatch {
    let mut patch = RecordingPatch::default();
    f(&mut patch);
    patch
}

pub fn span(start: Ms, end: Ms) -> Span {
    Span { start, end }
}

pub fn respan(start: Ms, end: Ms) -> RecordingPatch {
    patch(|p| p.spans = Some(vec![span(start, end)]))
}

pub fn input_of(w: &World, spans: &[(Ms, Ms)]) -> RecordingInput {
    RecordingInput {
        spans: spans.iter().map(|&(s, e)| span(s, e)).collect(),
        ..w.input(0, 0)
    }
}

pub fn overlap_ids(err: &AppError) -> Vec<String> {
    let in_the_way: Vec<Recording> =
        serde_json::from_value(err.details().unwrap().clone()).unwrap();
    in_the_way.into_iter().map(|r| r.id).collect()
}
