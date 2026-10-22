//! `timetracker report`: the range words and flags as a `ReportRequest`, and what to print.

use chrono::TimeZone;
use clap::Args;

use super::time;
use crate::db::time::Ms;
use crate::ipc::IpcMessage;
use crate::reports::Range;
use crate::reports::cli::{ReportOutput, ReportRequest};
use crate::store::settings::{Group, Rounding};

#[derive(Args, Debug)]
pub struct ReportArgs {
    /// today (default), yesterday, week, last-week, month, last-month, a day (YYYY-MM-DD) or two days FROM TO
    #[arg(num_args = 0..=2)]
    range: Vec<String>,
    /// How to group the hours
    #[arg(long, value_enum, default_value = "project")]
    by: Group,
    /// A client's id, name or unique start of its name; repeat for several
    #[arg(long = "client")]
    clients: Vec<String>,
    /// A project's id, name or unique start of its name; repeat for several
    #[arg(long = "project")]
    projects: Vec<String>,
    /// A tag's id, name or unique start of its name; repeat for several
    #[arg(long = "tag")]
    tags: Vec<String>,
    /// Round each recording to this many minutes
    #[arg(long, value_parser = ["5", "6", "10", "15", "30"])]
    round: Option<String>,
    /// Round up instead of to the nearest step
    #[arg(long, requires = "round")]
    up: bool,
    /// Print only the notes, one per line in time order (with --project: that project's)
    #[arg(long)]
    notes: bool,
}

impl ReportArgs {
    pub fn message<Tz: TimeZone>(self, now: Ms, tz: &Tz) -> Result<IpcMessage, String> {
        let output = if self.notes {
            ReportOutput::Notes
        } else {
            ReportOutput::Text
        };
        Ok(IpcMessage::Report {
            request: self.request(now, tz)?,
            output,
        })
    }

    fn request<Tz: TimeZone>(self, now: Ms, tz: &Tz) -> Result<ReportRequest, String> {
        Ok(ReportRequest {
            range: range(&self.range, now, tz)?,
            group_by: self.by,
            rounding: rounding(self.round.as_deref(), self.up),
            clients: self.clients,
            projects: self.projects,
            tags: self.tags,
        })
    }
}

/// No words, one range word or day, or two days `FROM TO`; at most two words.
pub fn range<Tz: TimeZone>(words: &[String], now: Ms, tz: &Tz) -> Result<Range, String> {
    Ok(match words {
        [] => Range::Today,
        [word] => match word.as_str() {
            "today" => Range::Today,
            "yesterday" => Range::Yesterday,
            "week" => Range::ThisWeek,
            "last-week" => Range::LastWeek,
            "month" => Range::ThisMonth,
            "last-month" => Range::LastMonth,
            day => {
                let day = time::day(day, now, tz)
                    .map_err(|_| format!("Not a range: `{day}` (see timetracker report --help)"))?;
                Range::Custom { from: day, to: day }
            }
        },
        [from, to] => {
            let (from, to) = (time::day(from, now, tz)?, time::day(to, now, tz)?);
            if from > to {
                return Err(format!("The range starts after it ends: {from} {to}"));
            }
            Range::Custom { from, to }
        }
        _ => unreachable!("clap allows at most two range words"),
    })
}

fn rounding(minutes: Option<&str>, up: bool) -> Rounding {
    match (minutes, up) {
        (Some("5"), false) => Rounding::Nearest5,
        (Some("6"), false) => Rounding::Nearest6,
        (Some("10"), false) => Rounding::Nearest10,
        (Some("15"), false) => Rounding::Nearest15,
        (Some("30"), false) => Rounding::Nearest30,
        (Some("5"), true) => Rounding::Up5,
        (Some("6"), true) => Rounding::Up6,
        (Some("10"), true) => Rounding::Up10,
        (Some("15"), true) => Rounding::Up15,
        (Some("30"), true) => Rounding::Up30,
        (None, _) => Rounding::None,
        (Some(other), _) => unreachable!("clap allows only the listed steps, not {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::testing::*;
    use crate::reports::cli::ReportOutput;
    use crate::store::settings::{Group, Rounding};

    fn report_request(
        range: crate::reports::Range,
        group_by: Group,
        rounding: Rounding,
    ) -> crate::reports::cli::ReportRequest {
        crate::reports::cli::ReportRequest {
            range,
            group_by,
            rounding,
            clients: vec![],
            projects: vec![],
            tags: vec![],
        }
    }

    #[test]
    fn report_without_arguments_reports_today_by_project_unrounded() {
        use crate::reports::Range;
        assert_eq!(
            remote(&["report"]),
            (
                IpcMessage::Report {
                    output: ReportOutput::Text,
                    request: report_request(Range::Today, Group::Project, Rounding::None)
                },
                false,
                false
            )
        );
    }

    #[test]
    fn report_range_words_and_days_become_ranges() {
        use crate::reports::Range;
        let cases = [
            (vec!["report", "today"], Range::Today),
            (vec!["report", "yesterday"], Range::Yesterday),
            (vec!["report", "week"], Range::ThisWeek),
            (vec!["report", "last-week"], Range::LastWeek),
            (vec!["report", "month"], Range::ThisMonth),
            (vec!["report", "last-month"], Range::LastMonth),
            (
                vec!["report", "2026-10-01"],
                Range::Custom {
                    from: day("2026-10-01"),
                    to: day("2026-10-01"),
                },
            ),
            (
                vec!["report", "2026-10-01", "2026-10-07"],
                Range::Custom {
                    from: day("2026-10-01"),
                    to: day("2026-10-07"),
                },
            ),
            (
                vec!["report", "2026-10-07", "2026-10-07"],
                Range::Custom {
                    from: day("2026-10-07"),
                    to: day("2026-10-07"),
                },
            ),
        ];
        for (args, range) in cases {
            assert_eq!(
                remote(&args).0,
                IpcMessage::Report {
                    output: ReportOutput::Text,
                    request: report_request(range, Group::Project, Rounding::None)
                },
                "{args:?}"
            );
        }
    }

    #[test]
    fn report_flags_set_grouping_rounding_and_json() {
        use crate::reports::Range;
        let (message, json, starts_app) = remote(&[
            "report",
            "last-week",
            "--by",
            "tag",
            "--round",
            "15",
            "--up",
            "--json",
        ]);
        assert_eq!(
            message,
            IpcMessage::Report {
                output: ReportOutput::Text,
                request: report_request(Range::LastWeek, Group::Tag, Rounding::Up15)
            }
        );
        assert!(json);
        assert!(!starts_app);
    }

    #[test]
    fn report_notes_asks_for_the_notes_instead_of_the_text() {
        use crate::reports::Range;
        assert_eq!(
            remote(&["report", "week", "--notes"]).0,
            IpcMessage::Report {
                output: ReportOutput::Notes,
                request: report_request(Range::ThisWeek, Group::Project, Rounding::None)
            }
        );
    }

    #[test]
    fn report_round_without_up_rounds_to_the_nearest_step() {
        for (minutes, rounding) in [
            ("5", Rounding::Nearest5),
            ("6", Rounding::Nearest6),
            ("10", Rounding::Nearest10),
            ("15", Rounding::Nearest15),
            ("30", Rounding::Nearest30),
        ] {
            let IpcMessage::Report { request, .. } = remote(&["report", "--round", minutes]).0
            else {
                panic!("not a report");
            };
            assert_eq!(request.rounding, rounding, "{minutes}");
        }
    }

    #[test]
    fn report_by_takes_every_grouping() {
        for (word, group) in [
            ("project", Group::Project),
            ("tag", Group::Tag),
            ("day", Group::Day),
            ("week", Group::Week),
            ("month", Group::Month),
        ] {
            let IpcMessage::Report { request, .. } = remote(&["report", "--by", word]).0 else {
                panic!("not a report");
            };
            assert_eq!(request.group_by, group, "{word}");
        }
    }

    #[test]
    fn report_filters_can_be_repeated() {
        let IpcMessage::Report { request, .. } = remote(&[
            "report",
            "--client",
            "acme",
            "--client",
            "globex",
            "--project",
            "portal",
            "--tag",
            "admin",
            "--tag",
            "support",
        ])
        .0
        else {
            panic!("not a report");
        };
        assert_eq!(request.clients, ["acme", "globex"]);
        assert_eq!(request.projects, ["portal"]);
        assert_eq!(request.tags, ["admin", "support"]);
    }

    #[test]
    fn report_errors_name_the_problem_and_stay_in_the_terminal() {
        let cases: Vec<(Vec<&str>, &str)> = vec![
            (vec!["report", "someday"], "Not a range: `someday`"),
            (
                vec!["report", "2026-10-01", "someday"],
                "Not a day: `someday`",
            ),
            (
                vec!["report", "2026-10-07", "2026-10-01"],
                "The range starts after it ends",
            ),
            (vec!["report", "--up"], "--round"),
            (vec!["report", "--round", "7"], "7"),
            (vec!["report", "--by", "year"], "year"),
            (vec!["report", "today", "yesterday", "week"], "week"),
        ];
        for (args, expected) in cases {
            let err = run(&args).unwrap_err();
            assert!(!err.in_app, "{args:?}");
            assert!(err.message.contains(expected), "{args:?}: {}", err.message);
        }
    }
}
