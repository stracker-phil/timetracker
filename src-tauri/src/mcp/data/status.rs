use serde::Serialize;

use super::minutes;
use crate::db::activities::ActivityRow;
use crate::db::time::to_local_iso;
use crate::remote::StatusReport;
use crate::timers::open_start;
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
    pub note: String,
    pub start_iso: String,
    pub elapsed_min: f64,
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
        timer: report.timer.map(|timer| TimerOut {
            start_iso: to_local_iso(open_start(&timer.recording)),
            project: timer.project_name,
            note: timer.recording.note,
            elapsed_min: minutes(timer.elapsed_ms),
        }),
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
    use crate::db::recordings::{Recording, RecordingSpan};
    use crate::db::time::Ms;
    use crate::mcp::data::fixtures::{at, iso};
    use crate::timers::RunningTimer;

    #[test]
    fn the_status_timer_starts_at_the_start_of_its_open_span() {
        let span = |id: &str, start: Ms, end: Option<Ms>| RecordingSpan {
            id: id.into(),
            start,
            end,
        };
        let recording = Recording {
            id: "r1".into(),
            project_id: "p1".into(),
            note: "tooling".into(),
            tag_id: None,
            created_at: 0,
            updated_at: 0,
            spans: vec![
                span("s1", at(9, 0), Some(at(10, 0))),
                span("s2", at(14, 0), None),
            ],
        };
        let report = StatusReport {
            version: "1.0.0".into(),
            tracker: None,
            timer: Some(RunningTimer {
                recording,
                project_name: "Portal".into(),
                color: "blue".into(),
                shade: 4,
                elapsed_ms: 30 * 60_000,
            }),
            billable: None,
            window_visible: false,
        };

        let out = serde_json::to_value(status_out(report, false)).unwrap();

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
}
