//! `timetracker record`: a recording's project, spans, note and tag as a `RecordRequest`.

use chrono::TimeZone;
use clap::Args;

use super::time;
use crate::db::time::Ms;
use crate::recordings::cli::RecordRequest;
use crate::recordings::{Overlap, Span};

#[derive(Args, Debug)]
pub struct RecordArgs {
    /// The project's id, name or a unique start of its name; a suggestion may leave it out for the owner to
    /// pick
    #[arg(long)]
    project: Option<String>,
    /// 2026-10-21T09:30 (local, or with Z or ±hh:mm), 09:30 (today) or now
    #[arg(long, requires = "to")]
    from: Option<String>,
    /// Like --from
    #[arg(long, requires = "from")]
    to: Option<String>,
    /// A span of the recording, like 14:25-15:00 (each end like --from); repeat for several
    #[arg(
        long = "span",
        value_name = "FROM-TO",
        required_unless_present = "from"
    )]
    spans: Vec<String>,
    /// One line
    #[arg(long, default_value = "")]
    note: String,
    /// A tag by name (at most one)
    #[arg(long)]
    tag: Option<String>,
    /// Trim or remove the recordings in the way instead of refusing (with --final)
    #[arg(long)]
    replace: bool,
    /// Write a recording instead of a suggestion to review
    #[arg(long = "final")]
    final_write: bool,
    /// Write a suggestion even with --final
    #[arg(long)]
    draft: bool,
    /// The app the suggestion comes from, shown in the Day view (default cli)
    #[arg(long)]
    source: Option<String>,
}

impl RecordArgs {
    pub fn request<Tz: TimeZone>(self, now: Ms, tz: &Tz) -> Result<RecordRequest, String> {
        let final_write = is_final(self.final_write, self.draft);
        if self.replace && !final_write {
            return Err("--replace changes recordings; add --final".into());
        }
        if final_write && self.project.is_none() {
            return Err("--final records time; add --project".into());
        }
        Ok(RecordRequest {
            project: self.project,
            spans: spans(self.from.zip(self.to), &self.spans, now, tz)?,
            note: self.note,
            tag: self.tag,
            overlap: overlap(self.replace),
            final_write,
            source: self.source,
        })
    }
}

/// `--final` writes recordings, unless `--draft` asks for a suggestion anyway (ADR 0028).
pub(super) fn is_final(final_flag: bool, draft: bool) -> bool {
    final_flag && !draft
}

pub(super) fn overlap(replace: bool) -> Overlap {
    if replace {
        Overlap::Replace
    } else {
        Overlap::Cancel
    }
}

/// `--from`/`--to` as one span, then every `--span`.
pub(super) fn spans<Tz: TimeZone>(
    from_to: Option<(String, String)>,
    spans: &[String],
    now: Ms,
    tz: &Tz,
) -> Result<Vec<Span>, String> {
    let mut all = Vec::new();
    if let Some((from, to)) = from_to {
        all.push(Span {
            start: time::instant(&from, now, tz)?,
            end: time::instant(&to, now, tz)?,
        });
    }
    for text in spans {
        all.push(time::span(text, now, tz)?);
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::testing::*;
    use crate::ipc::IpcMessage;

    fn record_request(note: &str, tag: Option<&str>, end: &str, overlap: Overlap) -> IpcMessage {
        IpcMessage::Record {
            request: RecordRequest {
                project: Some("ac".into()),
                spans: vec![Span {
                    start: ms("2026-10-21T09:00:00+02:00"),
                    end: ms(end),
                }],
                note: note.into(),
                tag: tag.map(str::to_string),
                overlap,
                final_write: false,
                source: None,
            },
        }
    }

    fn request_of(extra: &[&str]) -> RecordRequest {
        let base = [
            "record",
            "--project",
            "ac",
            "--from",
            "09:00",
            "--to",
            "10:30",
        ];
        let args: Vec<&str> = base.iter().chain(extra).copied().collect();
        match remote(&args).0 {
            IpcMessage::Record { request } => request,
            other => panic!("not a record: {other:?}"),
        }
    }

    fn spans_request(spans: &[(&str, &str)]) -> IpcMessage {
        IpcMessage::Record {
            request: RecordRequest {
                project: Some("ac".into()),
                spans: spans
                    .iter()
                    .map(|&(start, end)| Span {
                        start: ms(start),
                        end: ms(end),
                    })
                    .collect(),
                note: String::new(),
                tag: None,
                overlap: Overlap::Cancel,
                final_write: false,
                source: None,
            },
        }
    }

    #[test]
    fn record_takes_every_span_flag_in_order() {
        let (message, ..) = remote(&[
            "record",
            "--project",
            "ac",
            "--span",
            "14:25-15:00",
            "--span",
            "16:05-20:15",
        ]);

        assert_eq!(
            message,
            spans_request(&[
                ("2026-10-21T14:25:00+02:00", "2026-10-21T15:00:00+02:00"),
                ("2026-10-21T16:05:00+02:00", "2026-10-21T20:15:00+02:00"),
            ])
        );
    }

    #[test]
    fn record_puts_the_from_to_span_before_the_span_flags() {
        let (message, ..) = remote(&[
            "record",
            "--project",
            "ac",
            "--span",
            "16:05-20:15",
            "--from",
            "09:00",
            "--to",
            "10:30",
        ]);

        assert_eq!(
            message,
            spans_request(&[
                ("2026-10-21T09:00:00+02:00", "2026-10-21T10:30:00+02:00"),
                ("2026-10-21T16:05:00+02:00", "2026-10-21T20:15:00+02:00"),
            ])
        );
    }

    #[test]
    fn a_span_flag_reads_the_en_dash_dates_offsets_and_now() {
        let (message, ..) = remote(&[
            "record",
            "--project",
            "ac",
            "--span",
            "14:25–15:00",
            "--span",
            "2026-10-20T22:00-2026-10-21T01:00",
            "--span",
            "2026-10-21T09:30Z-2026-10-21T10:00Z",
            "--span",
            "11:00-now",
        ]);

        assert_eq!(
            message,
            spans_request(&[
                ("2026-10-21T14:25:00+02:00", "2026-10-21T15:00:00+02:00"),
                ("2026-10-20T22:00:00+02:00", "2026-10-21T01:00:00+02:00"),
                ("2026-10-21T09:30:00Z", "2026-10-21T10:00:00Z"),
                ("2026-10-21T11:00:00+02:00", "2026-10-21T12:00:00+02:00"),
            ])
        );
    }

    #[test]
    fn record_needs_from_and_to_together_or_a_span() {
        let cases: Vec<(Vec<&str>, &str)> = vec![
            (vec!["record", "--project", "ac"], "--span"),
            (vec!["record", "--project", "ac", "--from", "09:00"], "--to"),
            (
                vec![
                    "record",
                    "--project",
                    "ac",
                    "--span",
                    "14:25-15:00",
                    "--to",
                    "10:00",
                ],
                "--from",
            ),
            (
                vec!["record", "--project", "ac", "--span", "14:25"],
                "Not a span: `14:25`",
            ),
            (
                vec![
                    "record",
                    "--project",
                    "ac",
                    "--span",
                    "14:25-15:00",
                    "--span",
                    "later",
                ],
                "Not a span: `later`",
            ),
        ];
        for (args, expected) in cases {
            let err = run(&args).unwrap_err();
            assert!(!err.in_app, "{args:?}");
            assert!(err.message.contains(expected), "{args:?}: {}", err.message);
        }
    }

    #[test]
    fn record_builds_a_request_with_the_times_in_the_local_zone() {
        let (message, json, starts_app) = remote(&[
            "record",
            "--project",
            "ac",
            "--from",
            "09:00",
            "--to",
            "2026-10-21T10:30",
            "--note",
            "fix login",
            "--tag",
            "internal meeting",
        ]);

        assert_eq!(
            message,
            record_request(
                "fix login",
                Some("internal meeting"),
                "2026-10-21T10:30:00+02:00",
                Overlap::Cancel
            )
        );
        assert!(!json);
        assert!(!starts_app);
    }

    #[test]
    fn a_second_tag_on_record_or_start_is_a_parse_error() {
        for args in [
            vec![
                "record",
                "--project",
                "ac",
                "--from",
                "09:00",
                "--to",
                "10:30",
                "--tag",
                "a",
                "--tag",
                "b",
            ],
            vec!["start", "acme", "--tag", "a", "--tag", "b"],
        ] {
            let err = run(&args).unwrap_err();
            assert!(!err.in_app, "{args:?}");
            assert!(err.message.contains("--tag"), "{args:?}: {}", err.message);
        }
    }

    #[test]
    fn record_without_note_and_tag_sends_them_empty() {
        let (message, ..) = remote(&[
            "record",
            "--project",
            "ac",
            "--from",
            "09:00",
            "--to",
            "10:30",
        ]);

        assert_eq!(
            message,
            record_request("", None, "2026-10-21T10:30:00+02:00", Overlap::Cancel)
        );
    }

    #[test]
    fn record_final_with_replace_overlaps_by_replacing() {
        let request = request_of(&["--final", "--replace"]);

        assert_eq!(
            request,
            RecordRequest {
                project: Some("ac".into()),
                spans: vec![Span {
                    start: ms("2026-10-21T09:00:00+02:00"),
                    end: ms("2026-10-21T10:30:00+02:00"),
                }],
                note: String::new(),
                tag: None,
                overlap: Overlap::Replace,
                final_write: true,
                source: None,
            }
        );
    }

    #[test]
    fn record_is_final_only_with_final_and_without_draft() {
        for (extra, expected) in [
            (vec![], false),
            (vec!["--final"], true),
            (vec!["--draft"], false),
            (vec!["--final", "--draft"], false),
            (vec!["--draft", "--final"], false),
        ] {
            assert_eq!(request_of(&extra).final_write, expected, "{extra:?}");
        }
    }

    #[test]
    fn record_without_a_project_sends_none_for_a_suggestion() {
        for extra in [vec![], vec!["--draft"], vec!["--final", "--draft"]] {
            let args: Vec<&str> = ["record", "--span", "14:25-15:00"]
                .iter()
                .chain(&extra)
                .copied()
                .collect();

            let request = match remote(&args).0 {
                IpcMessage::Record { request } => request,
                other => panic!("not a record: {other:?}"),
            };

            assert_eq!(request.project, None, "{extra:?}");
            assert!(!request.final_write, "{extra:?}");
        }
    }

    #[test]
    fn record_final_without_a_project_is_refused_in_the_terminal() {
        let err = run(&["record", "--span", "14:25-15:00", "--final"]).unwrap_err();

        assert!(!err.in_app);
        assert_eq!(err.message, "--final records time; add --project");
    }

    #[test]
    fn record_passes_the_source_on_and_leaves_it_out_by_default() {
        assert_eq!(request_of(&[]).source, None);
        assert_eq!(
            request_of(&["--source", "planner"]).source,
            Some("planner".into())
        );
    }

    #[test]
    fn replace_without_an_effective_final_is_refused() {
        for extra in [
            vec!["--replace"],
            vec!["--replace", "--draft"],
            vec!["--replace", "--final", "--draft"],
        ] {
            let args: Vec<&str> = [
                "record",
                "--project",
                "ac",
                "--from",
                "09:00",
                "--to",
                "10:30",
            ]
            .iter()
            .chain(&extra)
            .copied()
            .collect();
            let err = run(&args).unwrap_err();

            assert!(!err.in_app, "{extra:?}");
            assert_eq!(
                err.message, "--replace changes recordings; add --final",
                "{extra:?}"
            );
        }
    }

    #[test]
    fn record_accepts_now_and_the_json_flag() {
        let (message, json, _) = remote(&[
            "record",
            "--project",
            "ac",
            "--from",
            "09:00",
            "--to",
            "now",
            "--json",
        ]);

        assert_eq!(
            message,
            record_request("", None, "2026-10-21T12:00:00+02:00", Overlap::Cancel)
        );
        assert!(json);
    }

    #[test]
    fn record_errors_name_the_problem_and_stay_in_the_terminal() {
        let cases: Vec<(Vec<&str>, &str)> = vec![
            (vec!["record", "--project", "ac", "--to", "10:00"], "--from"),
            (vec!["record", "--project", "ac", "--from", "09:00"], "--to"),
            (
                vec![
                    "record",
                    "--project",
                    "ac",
                    "--from",
                    "nonsense",
                    "--to",
                    "10:00",
                ],
                "Not a time: `nonsense`",
            ),
            (
                vec![
                    "record",
                    "--project",
                    "ac",
                    "--from",
                    "09:00",
                    "--to",
                    "later",
                ],
                "Not a time: `later`",
            ),
        ];
        for (args, expected) in cases {
            let err = run(&args).unwrap_err();
            assert!(!err.in_app, "{args:?}");
            assert!(err.message.contains(expected), "{args:?}: {}", err.message);
        }
    }
}
