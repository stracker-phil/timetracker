//! The start that made the running timer, kept in `timer_origin.json` across restarts, so starting the same
//! timer again leaves its recording or suggestion as the owner edited it since.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::recordings::timer::TimerInput;
use crate::store::json_store::JsonStore;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerOrigin {
    /// The running recording's id, or the running suggestion's.
    pub timer_id: String,
    pub input: TimerInput,
}

pub struct TimerOriginStore {
    store: JsonStore<Option<TimerOrigin>>,
}

impl TimerOriginStore {
    pub fn open(dir: &Path) -> Self {
        Self {
            store: JsonStore::open(dir.join("timer_origin.json")),
        }
    }

    pub fn get(&self) -> Option<TimerOrigin> {
        self.store.read(Clone::clone)
    }

    pub async fn set(&self, origin: TimerOrigin) -> AppResult<()> {
        self.store
            .update_async(move |current| *current = Some(origin))
            .await
    }
}

/// Whether starting `input` repeats the start that made the running timer `running_id`.
pub fn repeats(origin: Option<&TimerOrigin>, running_id: Option<&str>, input: &TimerInput) -> bool {
    origin.is_some_and(|o| running_id == Some(o.timer_id.as_str()) && o.input == *input)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(project: &str, note: &str, tag: Option<&str>) -> TimerInput {
        TimerInput {
            project_id: project.into(),
            note: note.into(),
            tag_id: tag.map(Into::into),
        }
    }

    fn origin() -> TimerOrigin {
        TimerOrigin {
            timer_id: "r1".into(),
            input: input("p1", "review", Some("t1")),
        }
    }

    #[test]
    fn a_new_folder_has_no_origin() {
        let dir = tempfile::tempdir().unwrap();

        assert_eq!(TimerOriginStore::open(dir.path()).get(), None);
    }

    #[tokio::test]
    async fn a_set_origin_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        TimerOriginStore::open(dir.path())
            .set(origin())
            .await
            .unwrap();

        let reopened = TimerOriginStore::open(dir.path());

        assert_eq!(reopened.get(), Some(origin()));
        assert!(dir.path().join("timer_origin.json").is_file());
    }

    #[tokio::test]
    async fn setting_again_replaces_the_origin() {
        let dir = tempfile::tempdir().unwrap();
        let store = TimerOriginStore::open(dir.path());
        store.set(origin()).await.unwrap();
        let next = TimerOrigin {
            timer_id: "r2".into(),
            input: input("p2", "", None),
        };

        store.set(next.clone()).await.unwrap();

        assert_eq!(TimerOriginStore::open(dir.path()).get(), Some(next));
    }

    #[test]
    fn a_corrupt_file_has_no_origin() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("timer_origin.json"), "{not json").unwrap();

        assert_eq!(TimerOriginStore::open(dir.path()).get(), None);
    }

    #[test]
    fn the_same_input_on_the_same_running_recording_repeats_the_start() {
        assert!(repeats(
            Some(&origin()),
            Some("r1"),
            &input("p1", "review", Some("t1"))
        ));
    }

    #[test]
    fn anything_else_is_a_new_start() {
        let same = input("p1", "review", Some("t1"));
        let cases = [
            ("no origin", None, Some("r1"), same.clone()),
            ("no running recording", Some(origin()), None, same.clone()),
            (
                "another running recording",
                Some(origin()),
                Some("r2"),
                same.clone(),
            ),
            (
                "another note",
                Some(origin()),
                Some("r1"),
                input("p1", "other", Some("t1")),
            ),
            (
                "another project",
                Some(origin()),
                Some("r1"),
                input("p2", "review", Some("t1")),
            ),
            (
                "another tag",
                Some(origin()),
                Some("r1"),
                input("p1", "review", Some("t2")),
            ),
            (
                "no tag",
                Some(origin()),
                Some("r1"),
                input("p1", "review", None),
            ),
        ];

        for (name, origin, running, input) in cases {
            assert!(!repeats(origin.as_ref(), running, &input), "{name}");
        }
    }
}
