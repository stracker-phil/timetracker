use serde::Serialize;

use super::minutes;
use crate::db::activities::ActivityRow;
use crate::db::time::to_local_iso;
use crate::remote::StatusReport;
use crate::scripting::timer::TimerState;
use crate::tracker::TrackerState;

#[derive(Serialize)]
pub struct StatusOut {
    pub version: String,
    pub tracker: Option<TrackerOut>,
    pub timer: Option<TimerOut>,
    pub window_visible: bool,
}

#[derive(Serialize)]
pub struct TrackerOut {
    pub state: TrackerState,
    pub text: String,
    pub current: Option<CurrentOut>,
}

/// The title and URL only when activity is exposed.
#[derive(Serialize)]
pub struct CurrentOut {
    pub app: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub since_iso: String,
}

#[derive(Serialize)]
pub struct TimerOut {
    pub project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    pub note: String,
    pub start_iso: String,
    pub elapsed_min: f64,
}

/// `None` while no timer runs.
fn timer_out(timer: TimerState) -> Option<TimerOut> {
    Some(TimerOut {
        project: timer.project?,
        client: timer.client,
        tag: timer.tag,
        note: timer.note.unwrap_or_default(),
        start_iso: timer.started_at_iso?,
        elapsed_min: minutes(timer.elapsed_ms.unwrap_or(0)),
    })
}

/// The app's status report in minutes and local times; window titles only when activity is exposed.
pub fn status_out(report: StatusReport, expose_activity: bool) -> StatusOut {
    StatusOut {
        version: report.version,
        tracker: report.tracker.map(|tracker| TrackerOut {
            state: tracker.state,
            text: tracker.text,
            current: tracker.current.map(|row| current_out(row, expose_activity)),
        }),
        timer: timer_out(report.timer),
        window_visible: report.window_visible,
    }
}

fn current_out(row: ActivityRow, expose_activity: bool) -> CurrentOut {
    let shown = expose_activity && !row.private;
    CurrentOut {
        app: row.app,
        title: shown.then_some(row.title),
        url: row.url.filter(|_| shown),
        since_iso: to_local_iso(row.start),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::mcp::data::fixtures::{at, iso};

    fn report_with(timer: TimerState) -> StatusReport {
        StatusReport {
            version: "1.0.0".into(),
            tracker: None,
            timer,
            billable: None,
            window_visible: false,
        }
    }

    fn running_timer() -> TimerState {
        TimerState {
            running: true,
            recording_id: Some("r1".into()),
            project_id: Some("p1".into()),
            project: Some("Portal".into()),
            note: Some("tooling".into()),
            started_at: Some(at(14, 0)),
            started_at_iso: Some(iso(14, 0)),
            elapsed_ms: Some(30 * 60_000),
            ..TimerState::default()
        }
    }

    #[test]
    fn the_status_timer_starts_at_the_start_of_its_open_span() {
        let out = serde_json::to_value(status_out(report_with(running_timer()), false)).unwrap();

        assert_eq!(
            out["timer"],
            json!({
                "project": "Portal",
                "note": "tooling",
                "start_iso": iso(14, 0),
                "elapsed_min": 30.0,
            })
        );
    }

    #[test]
    fn the_status_timer_names_its_client_and_tag_when_it_has_them() {
        let timer = TimerState {
            client: Some("Acme".into()),
            tag: Some("development".into()),
            ..running_timer()
        };

        let out = serde_json::to_value(status_out(report_with(timer), false)).unwrap();

        assert_eq!(
            out["timer"],
            json!({
                "project": "Portal",
                "client": "Acme",
                "tag": "development",
                "note": "tooling",
                "start_iso": iso(14, 0),
                "elapsed_min": 30.0,
            })
        );
    }

    #[test]
    fn the_status_has_no_timer_while_none_runs() {
        let out =
            serde_json::to_value(status_out(report_with(TimerState::default()), false)).unwrap();

        assert_eq!(out["timer"], json!(null));
    }
}
