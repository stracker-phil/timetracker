//! `timetracker record`: a recording's project, spans, note and tag as a `RecordRequest`.

use chrono::TimeZone;
use clap::Args;

use super::time;
use crate::db::time::Ms;
use crate::recordings::cli::RecordRequest;
use crate::recordings::{Overlap, Span};

#[derive(Args, Debug)]
pub struct RecordArgs {
    /// The project's id, name or a unique start of its name
    #[arg(long)]
    project: String,
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
    /// Trim or remove the recordings in the way instead of refusing
    #[arg(long)]
    replace: bool,
}

impl RecordArgs {
    pub fn request<Tz: TimeZone>(self, now: Ms, tz: &Tz) -> Result<RecordRequest, String> {
        Ok(RecordRequest {
            project: self.project,
            spans: spans(self.from.zip(self.to), &self.spans, now, tz)?,
            note: self.note,
            tag: self.tag,
            overlap: if self.replace {
                Overlap::Replace
            } else {
                Overlap::Cancel
            },
        })
    }
}

/// The span of the flags, or with `--json` and no span flags the array on stdin.
/// `--from`/`--to` as one span, then every `--span`.
fn spans<Tz: TimeZone>(
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
                project: "ac".into(),
                spans: vec![Span {
                    start: ms("2026-10-21T09:00:00+02:00"),
                    end: ms(end),
                }],
                note: note.into(),
                tag: tag.map(str::to_string),
                overlap,
            },
        }
    }

    fn spans_request(spans: &[(&str, &str)]) -> IpcMessage {
        IpcMessage::Record {
            request: RecordRequest {
                project: "ac".into(),
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
    fn record_with_replace_overlaps_by_replacing() {
        let (message, ..) = remote(&[
            "record",
            "--project",
            "ac",
            "--from",
            "09:00",
            "--to",
            "10:30",
            "--replace",
        ]);

        assert_eq!(
            message,
            record_request("", None, "2026-10-21T10:30:00+02:00", Overlap::Replace)
        );
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
            (
                vec!["record", "--from", "09:00", "--to", "10:00"],
                "--project",
            ),
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
