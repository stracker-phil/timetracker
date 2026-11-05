//! The start that made the running timer, kept for this run, so starting the same timer again leaves its
//! recording or suggestion as the owner edited it since.

use crate::recordings::timer::TimerInput;

#[derive(Debug, Clone, PartialEq)]
pub struct TimerOrigin {
    /// The running recording's id, or the running suggestion's.
    pub timer_id: String,
    pub input: TimerInput,
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
