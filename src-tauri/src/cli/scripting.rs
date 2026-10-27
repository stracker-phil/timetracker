//! `timetracker edit`, `delete`, `approve`, `recordings`, `suggestions`, `timer` and `tracker`: writes are
//! suggestions unless `--final`, and `--draft` wins over `--final` (ADR 0028); the timer is always direct.

use chrono::TimeZone;
use clap::{Args, Subcommand};

use super::record::{is_final, overlap, spans};
use super::report::range;
use crate::db::time::Ms;
use crate::ipc::IpcMessage;
use crate::scripting::writes::EditRequest;
use crate::timers;
use crate::timers::commands::{TimerStartRequest, TimerUpdateRequest};

/// The project, note and tag a change sets; absent ones stay.
#[derive(Args, Debug)]
pub struct ChangeArgs {
    /// The project's id, name or a unique start of its name
    #[arg(long)]
    project: Option<String>,
    /// One line
    #[arg(long)]
    note: Option<String>,
    /// A tag by name (at most one)
    #[arg(long, conflicts_with = "no_tag")]
    tag: Option<String>,
    /// Remove the tag
    #[arg(long)]
    no_tag: bool,
}

#[derive(Args, Debug)]
pub struct FinalArgs {
    /// Act on a recording instead of a suggestion (edit: or record the suggestion with the changes)
    #[arg(long = "final")]
    final_write: bool,
    /// Act on suggestions only, even with --final
    #[arg(long)]
    draft: bool,
}

impl FinalArgs {
    fn is_final(&self) -> bool {
        is_final(self.final_write, self.draft)
    }
}

#[derive(Args, Debug)]
pub struct EditArgs {
    /// A suggestion's id (from `timetracker suggestions`), or a recording's with --final
    id: String,
    #[command(flatten)]
    change: ChangeArgs,
    /// Replace every span: 2026-10-21T09:30, 09:30 or now
    #[arg(long, requires = "to")]
    from: Option<String>,
    /// Like --from
    #[arg(long, requires = "from")]
    to: Option<String>,
    /// Replace every span, like 14:25-15:00; repeat for several
    #[arg(long = "span", value_name = "FROM-TO")]
    spans: Vec<String>,
    #[command(flatten)]
    final_args: FinalArgs,
    /// Trim or remove the recordings in the way instead of refusing (with --final)
    #[arg(long)]
    replace: bool,
}

impl EditArgs {
    pub fn message<Tz: TimeZone>(self, now: Ms, tz: &Tz) -> Result<IpcMessage, String> {
        let final_write = self.final_args.is_final();
        if self.replace && !final_write {
            return Err("--replace changes recordings; add --final".into());
        }
        let spans = spans(self.from.zip(self.to), &self.spans, now, tz)?;
        let ChangeArgs {
            project,
            note,
            tag,
            no_tag,
        } = self.change;
        Ok(IpcMessage::Edit {
            request: EditRequest {
                id: self.id,
                project,
                note,
                tag,
                clear_tag: no_tag,
                spans: (!spans.is_empty()).then_some(spans),
                final_write,
                overlap: overlap(self.replace),
            },
        })
    }
}

#[derive(Args, Debug)]
pub struct DeleteArgs {
    /// A suggestion's id, or a recording's with --final
    id: String,
    #[command(flatten)]
    final_args: FinalArgs,
}

impl DeleteArgs {
    pub fn message(self) -> IpcMessage {
        IpcMessage::Delete {
            final_write: self.final_args.is_final(),
            id: self.id,
        }
    }
}

/// The days a listing covers.
#[derive(Args, Debug)]
pub struct ListRange {
    /// today (default), yesterday, week, last-week, month, last-month, a day, or two days FROM TO
    #[arg(num_args = 0..=2)]
    range: Vec<String>,
}

impl ListRange {
    pub fn recordings<Tz: TimeZone>(self, now: Ms, tz: &Tz) -> Result<IpcMessage, String> {
        Ok(IpcMessage::Recordings {
            range: range(&self.range, now, tz)?,
        })
    }

    pub fn suggestions<Tz: TimeZone>(self, now: Ms, tz: &Tz) -> Result<IpcMessage, String> {
        Ok(IpcMessage::Suggestions {
            range: range(&self.range, now, tz)?,
        })
    }
}

#[derive(Subcommand, Debug)]
pub enum TimerCommand {
    /// Start a timer for a project now; a running timer stops (the same project, note and tag continue)
    Start {
        /// The project's id, name or a unique start of its name
        project: String,
        /// One line
        #[arg(long, default_value = "")]
        note: String,
        /// A tag by name (at most one)
        #[arg(long)]
        tag: Option<String>,
    },
    /// Stop the running timer
    Stop,
    /// Print the running timer (--json: flat fields with project, client, tag, note and start)
    Status,
    /// Open the timer picker on the focused screen, as the timer shortcut does; the window stays hidden
    Pick,
    /// List the recent timers, as the tray offers them: project, note and tag, newest first
    Recent {
        /// At most this many
        #[arg(
            long,
            default_value_t = timers::RECENT as u32,
            value_parser = clap::value_parser!(u32).range(1..=timers::MAX_RECENT as i64)
        )]
        limit: u32,
    },
    /// Change the running timer in place: its project, note or tag
    Update(ChangeArgs),
}

impl TimerCommand {
    pub fn message(self) -> IpcMessage {
        match self {
            TimerCommand::Start { project, note, tag } => IpcMessage::TimerStart {
                request: TimerStartRequest { project, note, tag },
            },
            TimerCommand::Stop => IpcMessage::TimerStop,
            TimerCommand::Status => IpcMessage::TimerStatus,
            TimerCommand::Pick => IpcMessage::TimerPicker,
            TimerCommand::Recent { limit } => IpcMessage::TimerRecent { limit },
            TimerCommand::Update(ChangeArgs {
                project,
                note,
                tag,
                no_tag,
            }) => IpcMessage::TimerUpdate {
                request: TimerUpdateRequest {
                    project,
                    note,
                    tag,
                    clear_tag: no_tag,
                },
            },
        }
    }
}

/// Activity tracking: the background capture of apps and windows, apart from the timer.
#[derive(Subcommand, Debug)]
pub enum TrackerCommand {
    /// Pause tracking
    Pause {
        /// 15m, 1h (up to 24h), tomorrow (until midnight) or off (until resumed)
        until: String,
    },
    /// Resume paused tracking
    Resume,
    /// Print the tracker state and the current activity
    Status,
}

impl TrackerCommand {
    pub fn message(self) -> Result<IpcMessage, String> {
        Ok(match self {
            TrackerCommand::Pause { until } => IpcMessage::Pause {
                until: super::time::pause(&until)?,
            },
            TrackerCommand::Resume => IpcMessage::Resume,
            TrackerCommand::Status => IpcMessage::TrackerStatus,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::testing::*;
    use crate::recordings::{Overlap, Span};
    use crate::reports::Range;
    use crate::tracker::PauseUntil;

    fn edit_of(args: &[&str]) -> EditRequest {
        let args: Vec<&str> = ["edit", "abc"].iter().chain(args).copied().collect();
        match remote(&args).0 {
            IpcMessage::Edit { request } => request,
            other => panic!("not an edit: {other:?}"),
        }
    }

    fn unchanged(id: &str) -> EditRequest {
        EditRequest {
            id: id.into(),
            project: None,
            note: None,
            tag: None,
            clear_tag: false,
            spans: None,
            final_write: false,
            overlap: Overlap::Cancel,
        }
    }

    fn span(start: &str, end: &str) -> Span {
        Span {
            start: ms(start),
            end: ms(end),
        }
    }

    fn error_of(args: &[&str]) -> String {
        let err = run(args).unwrap_err();
        assert!(!err.in_app, "{args:?}");
        err.message
    }

    #[test]
    fn edit_with_only_an_id_changes_nothing_and_stays_a_suggestion() {
        assert_eq!(edit_of(&[]), unchanged("abc"));
    }

    #[test]
    fn edit_carries_the_project_note_and_tag() {
        assert_eq!(
            edit_of(&[
                "--project",
                "acme",
                "--note",
                "standup",
                "--tag",
                "internal"
            ]),
            EditRequest {
                project: Some("acme".into()),
                note: Some("standup".into()),
                tag: Some("internal".into()),
                ..unchanged("abc")
            }
        );
    }

    #[test]
    fn edit_can_empty_the_note_and_remove_the_tag() {
        assert_eq!(
            edit_of(&["--note", "", "--no-tag"]),
            EditRequest {
                note: Some(String::new()),
                clear_tag: true,
                ..unchanged("abc")
            }
        );
    }

    #[test]
    fn edit_tag_and_no_tag_exclude_each_other() {
        let message = error_of(&["edit", "abc", "--tag", "a", "--no-tag"]);

        assert!(message.contains("--tag"), "{message}");
        assert!(message.contains("--no-tag"), "{message}");
    }

    #[test]
    fn edit_replaces_the_spans_with_from_to_then_every_span_flag() {
        assert_eq!(
            edit_of(&["--from", "09:00", "--to", "09:30"]).spans,
            Some(vec![span(
                "2026-10-21T09:00:00+02:00",
                "2026-10-21T09:30:00+02:00"
            )])
        );
        assert_eq!(
            edit_of(&["--span", "14:25-15:00", "--span", "16:05-20:15"]).spans,
            Some(vec![
                span("2026-10-21T14:25:00+02:00", "2026-10-21T15:00:00+02:00"),
                span("2026-10-21T16:05:00+02:00", "2026-10-21T20:15:00+02:00"),
            ])
        );
        assert_eq!(
            edit_of(&["--span", "14:25-15:00", "--from", "09:00", "--to", "now"]).spans,
            Some(vec![
                span("2026-10-21T09:00:00+02:00", "2026-10-21T12:00:00+02:00"),
                span("2026-10-21T14:25:00+02:00", "2026-10-21T15:00:00+02:00"),
            ])
        );
    }

    #[test]
    fn edit_errors_name_the_problem_and_stay_in_the_terminal() {
        let cases: Vec<(Vec<&str>, &str)> = vec![
            (vec!["edit"], "<ID>"),
            (vec!["edit", "abc", "--from", "09:00"], "--to"),
            (vec!["edit", "abc", "--to", "09:00"], "--from"),
            (
                vec!["edit", "abc", "--span", "later"],
                "Not a span: `later`",
            ),
            (
                vec!["edit", "abc", "--from", "nonsense", "--to", "10:00"],
                "Not a time: `nonsense`",
            ),
        ];
        for (args, expected) in cases {
            let message = error_of(&args);
            assert!(message.contains(expected), "{args:?}: {message}");
        }
    }

    #[test]
    fn edit_is_final_only_with_final_and_without_draft() {
        for (extra, expected) in [
            (vec![], false),
            (vec!["--final"], true),
            (vec!["--draft"], false),
            (vec!["--final", "--draft"], false),
        ] {
            assert_eq!(edit_of(&extra).final_write, expected, "{extra:?}");
        }
    }

    #[test]
    fn edit_final_with_replace_overlaps_by_replacing() {
        let request = edit_of(&["--final", "--replace"]);

        assert_eq!(
            request,
            EditRequest {
                final_write: true,
                overlap: Overlap::Replace,
                ..unchanged("abc")
            }
        );
    }

    #[test]
    fn edit_replace_without_an_effective_final_is_refused() {
        for extra in [
            vec!["--replace"],
            vec!["--replace", "--draft"],
            vec!["--replace", "--final", "--draft"],
        ] {
            let args: Vec<&str> = ["edit", "abc"].iter().chain(&extra).copied().collect();

            assert_eq!(
                error_of(&args),
                "--replace changes recordings; add --final",
                "{extra:?}"
            );
        }
    }

    #[test]
    fn delete_names_the_id_and_needs_final_for_a_recording() {
        for (extra, expected) in [
            (vec![], false),
            (vec!["--final"], true),
            (vec!["--draft"], false),
            (vec!["--final", "--draft"], false),
        ] {
            let args: Vec<&str> = ["delete", "abc"].iter().chain(&extra).copied().collect();

            assert_eq!(
                remote(&args),
                (
                    IpcMessage::Delete {
                        id: "abc".into(),
                        final_write: expected
                    },
                    false,
                    false
                ),
                "{extra:?}"
            );
        }
    }

    #[test]
    fn delete_without_an_id_is_an_error() {
        assert!(error_of(&["delete"]).contains("<ID>"));
    }

    #[test]
    fn approve_takes_every_id_in_order() {
        assert_eq!(
            remote(&["approve", "b", "a", "c"]).0,
            IpcMessage::Approve {
                ids: vec!["b".into(), "a".into(), "c".into()]
            }
        );
    }

    #[test]
    fn approve_needs_at_least_one_id() {
        assert!(error_of(&["approve"]).contains("<IDS>"));
    }

    #[test]
    fn listings_default_to_today_and_read_range_words_and_days() {
        let cases = [
            (vec![], Range::Today),
            (vec!["yesterday"], Range::Yesterday),
            (vec!["last-week"], Range::LastWeek),
            (
                vec!["2026-10-01"],
                Range::Custom {
                    from: day("2026-10-01"),
                    to: day("2026-10-01"),
                },
            ),
            (
                vec!["2026-10-01", "2026-10-07"],
                Range::Custom {
                    from: day("2026-10-01"),
                    to: day("2026-10-07"),
                },
            ),
        ];
        for (words, range) in cases {
            let with = |command: &'static str| -> Vec<&'static str> {
                std::iter::once(command)
                    .chain(words.iter().copied())
                    .collect()
            };
            assert_eq!(
                remote(&with("recordings")).0,
                IpcMessage::Recordings {
                    range: range.clone()
                },
                "recordings {words:?}"
            );
            assert_eq!(
                remote(&with("suggestions")).0,
                IpcMessage::Suggestions { range },
                "suggestions {words:?}"
            );
        }
    }

    #[test]
    fn listing_ranges_that_make_no_sense_are_errors() {
        for command in ["recordings", "suggestions"] {
            assert!(
                error_of(&[command, "someday"]).contains("Not a range: `someday`"),
                "{command}"
            );
            assert!(
                error_of(&[command, "2026-10-07", "2026-10-01"])
                    .contains("The range starts after it ends"),
                "{command}"
            );
        }
    }

    #[test]
    fn projects_and_clients_list_archived_ones_only_on_request() {
        assert_eq!(
            remote(&["projects"]).0,
            IpcMessage::Projects { archived: false }
        );
        assert_eq!(
            remote(&["projects", "--archived"]).0,
            IpcMessage::Projects { archived: true }
        );
        assert_eq!(
            remote(&["clients"]).0,
            IpcMessage::Clients { archived: false }
        );
        assert_eq!(
            remote(&["clients", "--archived"]).0,
            IpcMessage::Clients { archived: true }
        );
    }

    #[test]
    fn catalog_and_undo_commands_become_their_messages() {
        for (args, message) in [
            (vec!["tags"], IpcMessage::Tags),
            (vec!["undo"], IpcMessage::Undo),
            (vec!["redo"], IpcMessage::Redo),
        ] {
            assert_eq!(remote(&args), (message, false, false), "{args:?}");
        }
    }

    #[test]
    fn scripting_commands_accept_the_json_flag() {
        for args in [
            vec!["recordings", "--json"],
            vec!["suggestions", "week", "--json"],
            vec!["projects", "--json"],
            vec!["tags", "--json"],
            vec!["timer", "status", "--json"],
            vec!["timer", "recent", "--json"],
        ] {
            assert!(remote(&args).1, "{args:?}");
        }
    }

    #[test]
    fn timer_subcommands_become_their_messages() {
        let start = |note: &str, tag: Option<&str>| IpcMessage::TimerStart {
            request: TimerStartRequest {
                project: "acme".into(),
                note: note.into(),
                tag: tag.map(String::from),
            },
        };
        let cases = [
            (vec!["timer", "start", "acme"], start("", None)),
            (
                vec!["timer", "start", "acme", "--note", "call", "--tag", "ext"],
                start("call", Some("ext")),
            ),
            (vec!["timer", "stop"], IpcMessage::TimerStop),
            (vec!["timer", "status"], IpcMessage::TimerStatus),
            (vec!["timer", "pick"], IpcMessage::TimerPicker),
            (
                vec!["timer", "recent"],
                IpcMessage::TimerRecent { limit: 8 },
            ),
            (
                vec!["timer", "recent", "--limit", "3"],
                IpcMessage::TimerRecent { limit: 3 },
            ),
        ];
        for (args, message) in cases {
            assert_eq!(remote(&args), (message, false, false), "{args:?}");
        }
    }

    #[test]
    fn timer_recent_limit_outside_one_to_a_hundred_is_an_error() {
        for limit in ["0", "101"] {
            let message = error_of(&["timer", "recent", "--limit", limit]);
            assert!(message.contains("--limit"), "{limit}: {message}");
        }
    }

    #[test]
    fn timer_update_carries_what_changes() {
        let update = |args: &[&str]| -> TimerUpdateRequest {
            let args: Vec<&str> = ["timer", "update"].iter().chain(args).copied().collect();
            match remote(&args).0 {
                IpcMessage::TimerUpdate { request } => request,
                other => panic!("not a timer update: {other:?}"),
            }
        };

        assert_eq!(
            update(&["--project", "acme", "--note", "ID-123", "--tag", "dev"]),
            TimerUpdateRequest {
                project: Some("acme".into()),
                note: Some("ID-123".into()),
                tag: Some("dev".into()),
                clear_tag: false,
            }
        );
        assert_eq!(
            update(&["--no-tag"]),
            TimerUpdateRequest {
                clear_tag: true,
                ..TimerUpdateRequest::default()
            }
        );
    }

    #[test]
    fn timer_update_tag_and_no_tag_exclude_each_other() {
        let message = error_of(&["timer", "update", "--tag", "a", "--no-tag"]);

        assert!(message.contains("--no-tag"), "{message}");
    }

    #[test]
    fn tracker_subcommands_become_their_messages() {
        let cases = [
            (
                vec!["tracker", "pause", "1h"],
                IpcMessage::Pause {
                    until: PauseUntil::Minutes(60),
                },
            ),
            (
                vec!["tracker", "pause", "tomorrow"],
                IpcMessage::Pause {
                    until: PauseUntil::Tomorrow,
                },
            ),
            (
                vec!["tracker", "pause", "off"],
                IpcMessage::Pause {
                    until: PauseUntil::Resumed,
                },
            ),
            (vec!["tracker", "resume"], IpcMessage::Resume),
            (vec!["tracker", "status"], IpcMessage::TrackerStatus),
        ];
        for (args, message) in cases {
            assert_eq!(remote(&args), (message, false, false), "{args:?}");
        }
    }

    #[test]
    fn tracker_pause_needs_a_known_duration() {
        assert!(error_of(&["tracker", "pause"]).contains("<UNTIL>"));
        assert_eq!(
            error_of(&["tracker", "pause", "soon"]),
            "Pause for 15m, 1h, tomorrow or off, not `soon`"
        );
    }
}
