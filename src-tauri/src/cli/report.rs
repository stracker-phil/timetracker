//! `timetracker report`: the range words and flags as a `ReportRequest`.

use chrono::TimeZone;
use clap::Args;

use super::time;
use crate::db::time::Ms;
use crate::reports::Range;
use crate::reports::cli::ReportRequest;
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
}

impl ReportArgs {
    pub fn request<Tz: TimeZone>(self, now: Ms, tz: &Tz) -> Result<ReportRequest, String> {
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

fn range<Tz: TimeZone>(words: &[String], now: Ms, tz: &Tz) -> Result<Range, String> {
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
