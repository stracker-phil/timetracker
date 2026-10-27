//! Single-instance handoff: a second `timetracker` process sends its request over a Unix socket
//! (mode 0600, same user only) to the running app and exits. The messages are here, the sending side in
//! `client.rs`, the app's side in `server.rs`.
//!
//! One JSON line per connection. A bare `IpcMessage` (the one-way form) is acknowledged with
//! the line `ok` before it is handled; an `IpcRequest` is answered with an `IpcReply` line
//! once it is handled. A `watch` request stays open after its reply and streams changes (`watch`).

mod client;
mod server;

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::activity::ActivityInput;
use crate::db::time::{Day, Ms};
use crate::error::{AppError, ErrorKind};
use crate::recordings::cli::RecordRequest;
use crate::reports::Range;
use crate::reports::cli::{ReportOutput, ReportRequest};
use crate::scripting::writes::EditRequest;
use crate::timers::commands::{TimerStartRequest, TimerUpdateRequest};
use crate::tracker::PauseUntil;
pub use client::{Forward, open_request, request, try_forward};
pub use server::{bind, serve};

// The tests' own imports, as they had them when this was one file.
#[cfg(test)]
use crate::watch::Watchers;
#[cfg(test)]
use std::io::{self, BufRead, BufReader};
#[cfg(test)]
use std::os::unix::fs::PermissionsExt;
#[cfg(test)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(test)]
use std::path::Path;
#[cfg(test)]
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Every request another process can make. Each CLI subcommand is one message; `remote::handle`
/// dispatches them.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum IpcMessage {
    /// Another launch: bring the window to the front unless `hidden`.
    Activate {
        #[serde(default)]
        hidden: bool,
    },
    Error {
        message: String,
    },
    /// Shows the window at `day`'s Day view (today when `None`); `focus` also brings it to the front.
    Show {
        day: Option<Day>,
        focus: bool,
    },
    Hide,
    Quit,
    Status,
    Pause {
        until: PauseUntil,
    },
    Resume,
    ActivityAdd {
        items: Vec<ActivityInput>,
    },
    ActivityStart {
        input: ActivityInput,
    },
    /// `source` is the bare name, as in `ActivityInput`.
    ActivityStop {
        source: String,
        end: Option<Ms>,
    },
    /// Deletes activity and away spans that share time with `[from, to)`.
    ActivityDelete {
        from: Ms,
        to: Ms,
    },
    ActivityDeleteUndo,
    Record {
        request: RecordRequest,
    },
    Report {
        request: ReportRequest,
        #[serde(default, skip_serializing_if = "ReportOutput::is_text")]
        output: ReportOutput,
    },
    TimerStart {
        request: TimerStartRequest,
    },
    TimerStop,
    TimerStatus,
    /// The timer picker on the focused screen, as the timer shortcut opens it.
    TimerPicker,
    /// The latest distinct project, note and tag combinations, as the tray offers them.
    TimerRecent {
        limit: u32,
    },
    TrackerStatus,
    TimerUpdate {
        request: TimerUpdateRequest,
    },
    Edit {
        request: EditRequest,
    },
    /// Deleting a recording needs `final`.
    Delete {
        id: String,
        #[serde(default, rename = "final")]
        final_write: bool,
    },
    Approve {
        ids: Vec<String>,
    },
    Recordings {
        range: Range,
    },
    Suggestions {
        range: Range,
    },
    Projects {
        archived: bool,
    },
    Clients {
        archived: bool,
    },
    Tags,
    Undo,
    Redo,
    /// As a request: replies with the current timer and tracker, then streams every change.
    Watch,
    /// Runs JavaScript in the window (used by scripts/dev to drive the UI).
    #[cfg(debug_assertions)]
    Eval {
        js: String,
    },
    /// Seeds generated workdays (`test_support::seed`); refused when activity before today exists,
    /// unless `yes`.
    #[cfg(debug_assertions)]
    DevSeed {
        years: u32,
        yes: bool,
        #[serde(default)]
        showcase: bool,
    },
}

impl IpcMessage {
    /// The message type for logs; never its contents (titles, scripts).
    pub fn kind(&self) -> &'static str {
        match self {
            IpcMessage::Activate { .. } => "Activate",
            IpcMessage::Error { .. } => "Error",
            IpcMessage::Show { .. } => "Show",
            IpcMessage::Hide => "Hide",
            IpcMessage::Quit => "Quit",
            IpcMessage::Status => "Status",
            IpcMessage::Pause { .. } => "Pause",
            IpcMessage::Resume => "Resume",
            IpcMessage::ActivityAdd { .. } => "ActivityAdd",
            IpcMessage::ActivityStart { .. } => "ActivityStart",
            IpcMessage::ActivityStop { .. } => "ActivityStop",
            IpcMessage::ActivityDelete { .. } => "ActivityDelete",
            IpcMessage::ActivityDeleteUndo => "ActivityDeleteUndo",
            IpcMessage::Record { .. } => "Record",
            IpcMessage::Report { .. } => "Report",
            IpcMessage::TimerStart { .. } => "TimerStart",
            IpcMessage::TimerStop => "TimerStop",
            IpcMessage::TimerStatus => "TimerStatus",
            IpcMessage::TimerPicker => "TimerPicker",
            IpcMessage::TimerRecent { .. } => "TimerRecent",
            IpcMessage::TrackerStatus => "TrackerStatus",
            IpcMessage::TimerUpdate { .. } => "TimerUpdate",
            IpcMessage::Edit { .. } => "Edit",
            IpcMessage::Delete { .. } => "Delete",
            IpcMessage::Approve { .. } => "Approve",
            IpcMessage::Recordings { .. } => "Recordings",
            IpcMessage::Suggestions { .. } => "Suggestions",
            IpcMessage::Projects { .. } => "Projects",
            IpcMessage::Clients { .. } => "Clients",
            IpcMessage::Tags => "Tags",
            IpcMessage::Undo => "Undo",
            IpcMessage::Redo => "Redo",
            IpcMessage::Watch => "Watch",
            #[cfg(debug_assertions)]
            IpcMessage::Eval { .. } => "Eval",
            #[cfg(debug_assertions)]
            IpcMessage::DevSeed { .. } => "DevSeed",
        }
    }

    /// How long the sender waits for the answer.
    fn reply_timeout(&self) -> Duration {
        match self {
            #[cfg(debug_assertions)]
            IpcMessage::DevSeed { .. } => SEED_TIMEOUT,
            _ => CLIENT_TIMEOUT,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IpcRequest {
    pub id: String,
    pub message: IpcMessage,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct IpcReply {
    pub id: String,
    pub ok: bool,
    pub result: serde_json::Value,
    pub error: Option<IpcError>,
}

impl IpcReply {
    fn new(id: String, outcome: HandlerResult) -> Self {
        match outcome {
            Ok(result) => Self {
                id,
                ok: true,
                result,
                error: None,
            },
            Err(error) => Self {
                id,
                ok: false,
                result: serde_json::Value::Null,
                error: Some(error),
            },
        }
    }

    pub fn into_result(self) -> HandlerResult {
        if self.ok {
            Ok(self.result)
        } else {
            Err(self
                .error
                .unwrap_or_else(|| IpcError::new(ErrorKind::Io, "The request failed")))
        }
    }
}

/// Why a request failed, as its sender sees it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct IpcError {
    pub kind: ErrorKind,
    pub message: String,
}

impl IpcError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl From<AppError> for IpcError {
    fn from(e: AppError) -> Self {
        Self::new(e.kind(), e.to_string())
    }
}

/// What handling a message produced: a JSON result, or an error for the sender.
pub type HandlerResult = Result<serde_json::Value, IpcError>;

const ACK: &str = "ok";
/// Longer lines get no answer; the CLI splits larger activity batches (`cli::split`).
pub const MAX_MESSAGE_BYTES: u64 = 64 * 1024;
const CLIENT_TIMEOUT: Duration = Duration::from_secs(5);
/// Seeding years of data takes seconds.
#[cfg(debug_assertions)]
const SEED_TIMEOUT: Duration = Duration::from_secs(120);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recordings::{Overlap, Span};
    use crate::reports::Range;
    use crate::store::settings::{Group, Rounding};
    use crate::test_support::temp_config_dir;
    use tokio::sync::mpsc;

    fn serve_into_channel(socket: &Path) -> mpsc::UnboundedReceiver<IpcMessage> {
        let (tx, rx) = mpsc::unbounded_channel();
        let listener = bind(socket).unwrap();
        tokio::spawn(serve(listener, Watchers::default(), move |m| {
            let _ = tx.send(m);
            async { Ok(serde_json::Value::Null) }
        }));
        rx
    }

    fn serve_answering(socket: &Path) -> Watchers {
        let listener = bind(socket).unwrap();
        let watchers = Watchers::default();
        tokio::spawn(serve(listener, watchers.clone(), |m| async move {
            match m {
                IpcMessage::Error { message } => Err(IpcError::new(
                    ErrorKind::Overlap,
                    format!("failed: {message}"),
                )),
                other => Ok(serde_json::json!({ "handled": other.kind() })),
            }
        }));
        watchers
    }

    /// Like `serve_answering`, but `watch` itself fails.
    fn serve_refusing_watch(socket: &Path) -> Watchers {
        let listener = bind(socket).unwrap();
        let watchers = Watchers::default();
        tokio::spawn(serve(listener, watchers.clone(), |m| async move {
            match m {
                IpcMessage::Watch => Err(IpcError::new(ErrorKind::Io, "no watching")),
                other => Ok(serde_json::json!({ "handled": other.kind() })),
            }
        }));
        watchers
    }

    type Lines = BufReader<UnixStream>;

    async fn open_watch(socket: &Path) -> (IpcReply, Lines) {
        let socket = socket.to_path_buf();
        tokio::task::spawn_blocking(move || open_request(&socket, IpcMessage::Watch))
            .await
            .unwrap()
            .unwrap()
            .unwrap()
    }

    /// The next line after the reply; empty once the server closed the connection.
    async fn next_line(mut lines: Lines) -> (String, Lines) {
        tokio::task::spawn_blocking(move || {
            let mut line = String::new();
            lines.read_line(&mut line).unwrap();
            (line, lines)
        })
        .await
        .unwrap()
    }

    async fn wait_until_unwatched(watchers: &Watchers) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while watchers.is_watched() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the watching connection should end");
    }

    fn projects_changed() -> crate::watch::WatchEvent {
        crate::watch::WatchEvent::Data(crate::events::DataChanged {
            kind: crate::events::DataKind::Projects,
            day: None,
        })
    }

    fn request_line(message: IpcMessage) -> String {
        let request = IpcRequest {
            id: "r1".into(),
            message,
        };
        format!("{}\n", serde_json::to_string(&request).unwrap())
    }

    async fn ask(socket: &Path, message: IpcMessage) -> io::Result<Option<IpcReply>> {
        let socket = socket.to_path_buf();
        tokio::task::spawn_blocking(move || request(&socket, message))
            .await
            .unwrap()
    }

    async fn forward(socket: &Path, message: IpcMessage) -> Forward {
        let socket = socket.to_path_buf();
        tokio::task::spawn_blocking(move || try_forward(&socket, &message))
            .await
            .unwrap()
            .unwrap()
    }

    /// Sends raw bytes and returns whatever the server answers before closing.
    async fn raw_exchange(socket: &Path, bytes: &[u8]) -> String {
        let mut stream = tokio::net::UnixStream::connect(socket).await.unwrap();
        let _ = stream.write_all(bytes).await;
        let mut reply = String::new();
        let _ = stream.read_to_string(&mut reply).await;
        reply
    }

    fn activate() -> IpcMessage {
        IpcMessage::Activate { hidden: false }
    }

    #[tokio::test]
    async fn successful_request_is_answered_with_the_handler_result_under_its_id() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        serve_answering(&socket);

        let reply = ask(&socket, activate()).await.unwrap().unwrap();

        assert!(reply.ok);
        assert_eq!(reply.result, serde_json::json!({ "handled": "Activate" }));
        assert_eq!(reply.error, None);
        assert!(!reply.id.is_empty());
    }

    #[tokio::test]
    async fn failing_request_is_answered_with_the_handler_error() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        serve_answering(&socket);

        let reply = ask(
            &socket,
            IpcMessage::Error {
                message: "x".into(),
            },
        )
        .await
        .unwrap()
        .unwrap();

        assert!(!reply.ok);
        assert_eq!(reply.result, serde_json::Value::Null);
        let expected = IpcError::new(ErrorKind::Overlap, "failed: x");
        assert_eq!(reply.error, Some(expected.clone()));
        assert_eq!(reply.into_result(), Err(expected));
    }

    fn day(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    fn planner_input(end: Option<Ms>) -> ActivityInput {
        ActivityInput {
            source: "planner".into(),
            title: "Plan".into(),
            start: 1_000,
            end,
            app: None,
            url: None,
            path: None,
        }
    }

    fn cli_messages() -> Vec<IpcMessage> {
        vec![
            IpcMessage::Show {
                day: Some(day("2026-10-21")),
                focus: true,
            },
            IpcMessage::Show {
                day: None,
                focus: false,
            },
            IpcMessage::Hide,
            IpcMessage::Quit,
            IpcMessage::Status,
            IpcMessage::Pause {
                until: PauseUntil::Minutes(15),
            },
            IpcMessage::Pause {
                until: PauseUntil::Tomorrow,
            },
            IpcMessage::Pause {
                until: PauseUntil::Resumed,
            },
            IpcMessage::Resume,
            IpcMessage::ActivityAdd {
                items: vec![planner_input(Some(2_000)), planner_input(Some(3_000))],
            },
            IpcMessage::ActivityStart {
                input: planner_input(None),
            },
            IpcMessage::ActivityStop {
                source: "planner".into(),
                end: Some(5_000),
            },
            record_message(),
            report_message(),
            timer_start_message(),
            IpcMessage::TimerStop,
            IpcMessage::TimerStatus,
            IpcMessage::TimerPicker,
            IpcMessage::TimerRecent { limit: 5 },
            IpcMessage::TrackerStatus,
            timer_update_message(),
            suggestion_message(),
            edit_message(),
            IpcMessage::Delete {
                id: "s1".into(),
                final_write: true,
            },
            IpcMessage::Approve {
                ids: vec!["s1".into(), "rule:1000:2000".into()],
            },
            IpcMessage::Recordings {
                range: Range::Yesterday,
            },
            IpcMessage::Suggestions {
                range: Range::Custom {
                    from: day("2026-10-01"),
                    to: day("2026-10-07"),
                },
            },
            IpcMessage::Projects { archived: true },
            IpcMessage::Clients { archived: false },
            IpcMessage::Tags,
            IpcMessage::Undo,
            IpcMessage::Redo,
            IpcMessage::Watch,
        ]
    }

    fn timer_update_message() -> IpcMessage {
        IpcMessage::TimerUpdate {
            request: TimerUpdateRequest {
                project: Some("acme".into()),
                note: Some("ID-123: Payment bug".into()),
                tag: None,
                clear_tag: true,
            },
        }
    }

    fn edit_message() -> IpcMessage {
        IpcMessage::Edit {
            request: EditRequest {
                id: "s1".into(),
                project: None,
                note: Some("standup".into()),
                tag: Some("internal meeting".into()),
                clear_tag: false,
                spans: Some(vec![Span {
                    start: 1_000,
                    end: 2_000,
                }]),
                final_write: true,
                overlap: Overlap::Cancel,
            },
        }
    }

    fn suggestion_message() -> IpcMessage {
        IpcMessage::Record {
            request: RecordRequest {
                project: Some("acme".into()),
                spans: vec![Span {
                    start: 1_000,
                    end: 2_000,
                }],
                note: String::new(),
                tag: None,
                overlap: Overlap::Cancel,
                final_write: false,
                source: Some("planner".into()),
            },
        }
    }

    fn timer_start_message() -> IpcMessage {
        IpcMessage::TimerStart {
            request: TimerStartRequest {
                project: "acme".into(),
                note: "weekly call".into(),
                tag: Some("external meeting".into()),
            },
        }
    }

    #[test]
    fn timer_messages_are_of_their_own_kinds() {
        assert_eq!(timer_start_message().kind(), "TimerStart");
        assert_eq!(IpcMessage::TimerStop.kind(), "TimerStop");
        assert_eq!(IpcMessage::TimerPicker.kind(), "TimerPicker");
        assert_eq!(IpcMessage::TimerRecent { limit: 5 }.kind(), "TimerRecent");
    }

    #[test]
    fn a_watch_message_is_of_the_kind_watch() {
        assert_eq!(IpcMessage::Watch.kind(), "Watch");
    }

    #[cfg(debug_assertions)]
    #[test]
    fn a_dev_seed_message_is_of_the_kind_dev_seed_and_keeps_its_wire_shape() {
        let message = IpcMessage::DevSeed {
            years: 3,
            yes: true,
            showcase: false,
        };
        let wire =
            serde_json::json!({ "type": "devSeed", "years": 3, "yes": true, "showcase": false });

        assert_eq!(message.kind(), "DevSeed");
        assert_eq!(serde_json::to_value(&message).unwrap(), wire);
        assert_eq!(serde_json::from_value::<IpcMessage>(wire).unwrap(), message);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn a_dev_seed_message_without_showcase_is_not_a_showcase() {
        let wire = serde_json::json!({ "type": "devSeed", "years": 3, "yes": true });

        assert_eq!(
            serde_json::from_value::<IpcMessage>(wire).unwrap(),
            IpcMessage::DevSeed {
                years: 3,
                yes: true,
                showcase: false
            }
        );
    }

    fn report_message() -> IpcMessage {
        IpcMessage::Report {
            output: ReportOutput::Text,
            request: ReportRequest {
                range: Range::Custom {
                    from: day("2026-10-01"),
                    to: day("2026-10-07"),
                },
                group_by: Group::Tag,
                rounding: Rounding::Up15,
                clients: vec!["acme".into()],
                projects: vec![],
                tags: vec!["admin".into()],
            },
        }
    }

    #[test]
    fn a_report_message_is_of_the_kind_report() {
        assert_eq!(report_message().kind(), "Report");
    }

    fn record_message() -> IpcMessage {
        IpcMessage::Record {
            request: RecordRequest {
                project: Some("acme".into()),
                spans: vec![
                    Span {
                        start: 1_000,
                        end: 2_000,
                    },
                    Span {
                        start: 3_000,
                        end: 4_000,
                    },
                ],
                note: "fix login".into(),
                tag: Some("internal meeting".into()),
                overlap: Overlap::Replace,
                final_write: true,
                source: None,
            },
        }
    }

    #[tokio::test]
    async fn cli_messages_round_trip_one_way_through_the_socket() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        let mut rx = serve_into_channel(&socket);

        for message in cli_messages() {
            assert_eq!(forward(&socket, message.clone()).await, Forward::Delivered);
            assert_eq!(rx.recv().await.unwrap(), message);
        }
    }

    #[tokio::test]
    async fn cli_messages_round_trip_as_requests_through_the_socket() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        let mut rx = serve_into_channel(&socket);

        for message in cli_messages() {
            let reply = ask(&socket, message.clone()).await.unwrap().unwrap();
            assert!(reply.ok, "{message:?}");
            assert_eq!(rx.recv().await.unwrap(), message);
        }
    }

    #[tokio::test]
    async fn every_cli_message_reaches_the_handler_as_its_own_kind() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        serve_answering(&socket);

        for message in cli_messages() {
            let kind = message.kind();
            let reply = ask(&socket, message).await.unwrap().unwrap();
            assert_eq!(reply.result, serde_json::json!({ "handled": kind }));
        }
    }

    #[test]
    fn messages_have_their_wire_shape() {
        let cases = [
            (
                IpcMessage::Show {
                    day: Some(day("2026-10-21")),
                    focus: true,
                },
                serde_json::json!({ "type": "show", "day": "2026-10-21", "focus": true }),
            ),
            (
                IpcMessage::Show {
                    day: None,
                    focus: false,
                },
                serde_json::json!({ "type": "show", "day": null, "focus": false }),
            ),
            (IpcMessage::Hide, serde_json::json!({ "type": "hide" })),
            (
                IpcMessage::Pause {
                    until: PauseUntil::Minutes(15),
                },
                serde_json::json!({ "type": "pause", "until": { "minutes": 15 } }),
            ),
            (
                IpcMessage::Pause {
                    until: PauseUntil::Tomorrow,
                },
                serde_json::json!({ "type": "pause", "until": "tomorrow" }),
            ),
            (
                IpcMessage::ActivityStop {
                    source: "planner".into(),
                    end: None,
                },
                serde_json::json!({ "type": "activityStop", "source": "planner", "end": null }),
            ),
            (
                record_message(),
                serde_json::json!({
                    "type": "record",
                    "request": {
                        "project": "acme",
                        "spans": [
                            { "start": 1000, "end": 2000 },
                            { "start": 3000, "end": 4000 }
                        ],
                        "note": "fix login",
                        "tag": "internal meeting", "overlap": "replace", "final": true
                    }
                }),
            ),
            (
                suggestion_message(),
                serde_json::json!({
                    "type": "record",
                    "request": {
                        "project": "acme",
                        "spans": [{ "start": 1000, "end": 2000 }],
                        "note": "",
                        "tag": null, "overlap": "cancel", "final": false, "source": "planner"
                    }
                }),
            ),
            (
                IpcMessage::TimerStatus,
                serde_json::json!({ "type": "timerStatus" }),
            ),
            (
                IpcMessage::TimerPicker,
                serde_json::json!({ "type": "timerPicker" }),
            ),
            (
                IpcMessage::TimerRecent { limit: 5 },
                serde_json::json!({ "type": "timerRecent", "limit": 5 }),
            ),
            (
                IpcMessage::TrackerStatus,
                serde_json::json!({ "type": "trackerStatus" }),
            ),
            (IpcMessage::Watch, serde_json::json!({ "type": "watch" })),
            (
                timer_update_message(),
                serde_json::json!({
                    "type": "timerUpdate",
                    "request": {
                        "project": "acme", "note": "ID-123: Payment bug",
                        "tag": null, "clearTag": true
                    }
                }),
            ),
            (
                edit_message(),
                serde_json::json!({
                    "type": "edit",
                    "request": {
                        "id": "s1", "project": null, "note": "standup",
                        "tag": "internal meeting", "clearTag": false,
                        "spans": [{ "start": 1000, "end": 2000 }],
                        "final": true, "overlap": "cancel"
                    }
                }),
            ),
            (
                IpcMessage::Delete {
                    id: "s1".into(),
                    final_write: true,
                },
                serde_json::json!({ "type": "delete", "id": "s1", "final": true }),
            ),
            (
                IpcMessage::Approve {
                    ids: vec!["s1".into(), "rule:1000:2000".into()],
                },
                serde_json::json!({ "type": "approve", "ids": ["s1", "rule:1000:2000"] }),
            ),
            (
                IpcMessage::Recordings {
                    range: Range::Yesterday,
                },
                serde_json::json!({ "type": "recordings", "range": { "kind": "yesterday" } }),
            ),
            (
                IpcMessage::Suggestions {
                    range: Range::Custom {
                        from: day("2026-10-01"),
                        to: day("2026-10-07"),
                    },
                },
                serde_json::json!({
                    "type": "suggestions",
                    "range": { "kind": "custom", "from": "2026-10-01", "to": "2026-10-07" }
                }),
            ),
            (
                IpcMessage::Projects { archived: true },
                serde_json::json!({ "type": "projects", "archived": true }),
            ),
            (
                IpcMessage::Clients { archived: false },
                serde_json::json!({ "type": "clients", "archived": false }),
            ),
            (IpcMessage::Tags, serde_json::json!({ "type": "tags" })),
            (IpcMessage::Undo, serde_json::json!({ "type": "undo" })),
            (IpcMessage::Redo, serde_json::json!({ "type": "redo" })),
            (
                report_message(),
                serde_json::json!({
                    "type": "report",
                    "request": {
                        "range": { "kind": "custom", "from": "2026-10-01", "to": "2026-10-07" },
                        "groupBy": "tag", "rounding": "up15",
                        "clients": ["acme"], "projects": [], "tags": ["admin"]
                    }
                }),
            ),
            (
                IpcMessage::Report {
                    output: ReportOutput::Text,
                    request: ReportRequest {
                        range: Range::LastWeek,
                        group_by: Group::Project,
                        rounding: Rounding::None,
                        clients: vec![],
                        projects: vec![],
                        tags: vec![],
                    },
                },
                serde_json::json!({
                    "type": "report",
                    "request": {
                        "range": { "kind": "lastWeek" },
                        "groupBy": "project", "rounding": "none",
                        "clients": [], "projects": [], "tags": []
                    }
                }),
            ),
            (
                IpcMessage::Report {
                    output: ReportOutput::Notes,
                    request: ReportRequest {
                        range: Range::Today,
                        group_by: Group::Project,
                        rounding: Rounding::None,
                        clients: vec![],
                        projects: vec![],
                        tags: vec![],
                    },
                },
                serde_json::json!({
                    "type": "report",
                    "request": {
                        "range": { "kind": "today" },
                        "groupBy": "project", "rounding": "none",
                        "clients": [], "projects": [], "tags": []
                    },
                    "output": "notes"
                }),
            ),
            (
                timer_start_message(),
                serde_json::json!({
                    "type": "timerStart",
                    "request": {
                        "project": "acme", "note": "weekly call", "tag": "external meeting"
                    }
                }),
            ),
            (
                IpcMessage::TimerStop,
                serde_json::json!({ "type": "timerStop" }),
            ),
            (
                IpcMessage::ActivityStart {
                    input: planner_input(None),
                },
                serde_json::json!({
                    "type": "activityStart",
                    "input": {
                        "source": "planner", "title": "Plan", "start": 1000, "end": null,
                        "app": null, "url": null, "path": null
                    }
                }),
            ),
        ];
        for (message, wire) in cases {
            assert_eq!(serde_json::to_value(&message).unwrap(), wire, "{message:?}");
            assert_eq!(
                serde_json::from_value::<IpcMessage>(wire.clone()).unwrap(),
                message,
                "{wire}"
            );
        }
    }

    #[test]
    fn replies_carry_the_error_kind_and_message_on_the_wire() {
        let reply = IpcReply {
            id: "7".into(),
            ok: false,
            result: serde_json::Value::Null,
            error: Some(IpcError::new(ErrorKind::Overlap, "Overlaps activity 3")),
        };
        assert_eq!(
            serde_json::to_value(&reply).unwrap(),
            serde_json::json!({
                "id": "7",
                "ok": false,
                "result": null,
                "error": { "kind": "overlap", "message": "Overlaps activity 3" }
            })
        );
    }

    #[test]
    fn ipc_error_from_an_app_error_keeps_its_kind_and_display_text() {
        let cases = [
            (
                AppError::Overlap {
                    message: "Overlaps activity 3".into(),
                    details: serde_json::json!({ "ids": [3] }),
                },
                IpcError::new(ErrorKind::Overlap, "Overlaps activity 3"),
            ),
            (
                AppError::InvalidField {
                    field: "title".into(),
                    message: "must not be empty".into(),
                },
                IpcError::new(ErrorKind::InvalidInput, "`title`: must not be empty"),
            ),
            (
                AppError::Io("denied".into()),
                IpcError::new(ErrorKind::Io, "File error: denied"),
            ),
            (
                AppError::TrackerPaused("Paused".into()),
                IpcError::new(ErrorKind::TrackerPaused, "Paused"),
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(IpcError::from(error.clone()), expected, "{error:?}");
        }
    }

    #[test]
    fn a_failed_reply_without_an_error_gets_a_generic_message() {
        let reply = IpcReply {
            id: "1".into(),
            ok: false,
            result: serde_json::Value::Null,
            error: None,
        };
        assert_eq!(
            reply.into_result(),
            Err(IpcError::new(ErrorKind::Io, "The request failed"))
        );
    }

    #[test]
    fn a_successful_reply_turns_into_its_result() {
        let reply = IpcReply {
            id: "1".into(),
            ok: true,
            result: serde_json::json!({ "ids": [1, 2] }),
            error: None,
        };
        assert_eq!(
            reply.into_result(),
            Ok(serde_json::json!({ "ids": [1, 2] }))
        );
    }

    #[tokio::test]
    async fn request_over_64_kib_gets_no_reply_and_returns_an_error() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        let mut rx = serve_into_channel(&socket);

        let huge = IpcMessage::Error {
            message: "a".repeat(MAX_MESSAGE_BYTES as usize + 1),
        };
        assert!(ask(&socket, huge).await.is_err());

        assert_eq!(forward(&socket, activate()).await, Forward::Delivered);
        assert_eq!(rx.recv().await.unwrap(), activate());
    }

    #[tokio::test]
    async fn one_way_message_is_acknowledged_with_ok_and_reaches_the_handler() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        let mut rx = serve_into_channel(&socket);

        let line = format!("{}\n", serde_json::to_string(&activate()).unwrap());
        assert_eq!(raw_exchange(&socket, line.as_bytes()).await, "ok\n");
        assert_eq!(rx.recv().await.unwrap(), activate());

        assert_eq!(forward(&socket, activate()).await, Forward::Delivered);
        assert_eq!(rx.recv().await.unwrap(), activate());
    }

    #[tokio::test]
    async fn messages_round_trip_through_the_socket() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        let mut rx = serve_into_channel(&socket);

        for message in [
            IpcMessage::Error {
                message: "boom".into(),
            },
            IpcMessage::Activate { hidden: true },
            activate(),
        ] {
            assert_eq!(forward(&socket, message.clone()).await, Forward::Delivered);
            assert_eq!(rx.recv().await.unwrap(), message);
        }
    }

    #[tokio::test]
    async fn malformed_line_gets_no_ack_and_reaches_no_handler() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        let mut rx = serve_into_channel(&socket);

        assert_eq!(raw_exchange(&socket, b"{\"type\": nope\n").await, "");

        assert_eq!(forward(&socket, activate()).await, Forward::Delivered);
        assert_eq!(rx.recv().await.unwrap(), activate());
    }

    #[tokio::test]
    async fn line_over_64_kib_is_refused() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        let mut rx = serve_into_channel(&socket);

        let mut huge = br#"{"type":"error","message":""#.to_vec();
        huge.extend(std::iter::repeat_n(b'a', 70 * 1024));
        huge.extend_from_slice(b"\"}\n");
        assert_eq!(raw_exchange(&socket, &huge).await, "");

        assert_eq!(forward(&socket, activate()).await, Forward::Delivered);
        assert_eq!(rx.recv().await.unwrap(), activate());
    }

    #[tokio::test]
    async fn two_clients_at_once_are_both_served() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        let mut rx = serve_into_channel(&socket);

        let first = IpcMessage::Error {
            message: "first".into(),
        };
        let second = IpcMessage::Error {
            message: "second".into(),
        };
        let (a, b) = tokio::join!(
            forward(&socket, first.clone()),
            forward(&socket, second.clone())
        );
        assert_eq!((a, b), (Forward::Delivered, Forward::Delivered));

        let mut received = vec![rx.recv().await.unwrap(), rx.recv().await.unwrap()];
        received.sort_by_key(|m| format!("{m:?}"));
        assert_eq!(received, vec![first, second]);
    }

    #[tokio::test]
    async fn a_watch_request_gets_its_reply_and_then_every_change() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        let watchers = serve_answering(&socket);

        let (reply, lines) = open_watch(&socket).await;
        assert!(reply.ok);
        assert_eq!(reply.result, serde_json::json!({ "handled": "Watch" }));

        watchers.send(projects_changed());
        let (line, lines) = next_line(lines).await;
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&line).unwrap(),
            serde_json::json!({ "event": "data", "data": { "kind": "projects", "day": null } })
        );

        watchers.send(projects_changed());
        let (line, _lines) = next_line(lines).await;
        assert_eq!(
            serde_json::from_str::<crate::watch::WatchEvent>(&line).unwrap(),
            projects_changed()
        );
    }

    #[tokio::test]
    async fn a_failed_watch_request_gets_its_error_and_no_stream() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        let watchers = serve_refusing_watch(&socket);

        let (reply, lines) = open_watch(&socket).await;
        assert_eq!(
            reply.into_result(),
            Err(IpcError::new(ErrorKind::Io, "no watching"))
        );

        wait_until_unwatched(&watchers).await;
        watchers.send(projects_changed());
        assert_eq!(next_line(lines).await.0, "");
    }

    #[tokio::test]
    async fn closing_the_watching_client_ends_its_subscription() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        let watchers = serve_answering(&socket);

        let (_, lines) = open_watch(&socket).await;
        assert!(watchers.is_watched());

        drop(lines);
        wait_until_unwatched(&watchers).await;
    }

    #[tokio::test]
    async fn a_watcher_that_falls_too_far_behind_is_disconnected() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        let watchers = serve_answering(&socket);

        let (_, lines) = open_watch(&socket).await;
        for _ in 0..100 {
            watchers.send(projects_changed());
        }

        assert_eq!(next_line(lines).await.0, "");
        wait_until_unwatched(&watchers).await;
    }

    #[tokio::test]
    async fn a_one_way_watch_is_acknowledged_and_closed() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        let mut rx = serve_into_channel(&socket);

        let line = format!("{}\n", serde_json::to_string(&IpcMessage::Watch).unwrap());
        assert_eq!(raw_exchange(&socket, line.as_bytes()).await, "ok\n");
        assert_eq!(rx.recv().await.unwrap(), IpcMessage::Watch);
    }

    #[tokio::test]
    async fn other_requests_get_one_reply_and_are_closed_while_someone_watches() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        let watchers = serve_answering(&socket);
        let (_, _lines) = open_watch(&socket).await;

        let answer = raw_exchange(&socket, request_line(IpcMessage::TimerStatus).as_bytes()).await;
        watchers.send(projects_changed());

        let reply: IpcReply = serde_json::from_str(answer.trim_end()).unwrap();
        assert_eq!(answer.lines().count(), 1);
        assert_eq!(reply.id, "r1");
        assert_eq!(
            reply.result,
            serde_json::json!({ "handled": "TimerStatus" })
        );
    }

    #[test]
    fn open_request_without_a_socket_returns_none() {
        let dir = temp_config_dir();
        let opened = open_request(&dir.path().join("none.sock"), IpcMessage::Watch).unwrap();
        assert!(opened.is_none());
    }

    #[test]
    fn activate_without_hidden_deserializes_as_not_hidden() {
        let message: IpcMessage = serde_json::from_str(r#"{"type":"activate"}"#).unwrap();
        assert_eq!(message, IpcMessage::Activate { hidden: false });
    }

    #[test]
    fn record_and_delete_without_final_deserialize_as_not_final() {
        let record = serde_json::json!({
            "type": "record",
            "request": {
                "project": "acme",
                "spans": [{ "start": 1000, "end": 2000 }],
                "note": "", "tag": null, "overlap": "cancel"
            }
        });
        let delete = serde_json::json!({ "type": "delete", "id": "s1" });

        assert_eq!(
            serde_json::from_value::<IpcMessage>(record).unwrap(),
            suggestion_message_from(None)
        );
        assert_eq!(
            serde_json::from_value::<IpcMessage>(delete).unwrap(),
            IpcMessage::Delete {
                id: "s1".into(),
                final_write: false
            }
        );
    }

    fn suggestion_message_from(source: Option<&str>) -> IpcMessage {
        let IpcMessage::Record { request } = suggestion_message() else {
            unreachable!("a record message")
        };
        IpcMessage::Record {
            request: RecordRequest {
                source: source.map(String::from),
                ..request
            },
        }
    }

    #[test]
    fn forwarding_without_running_instance_reports_no_instance() {
        let dir = temp_config_dir();
        let forwarded = try_forward(&dir.path().join("none.sock"), &activate()).unwrap();
        assert_eq!(forwarded, Forward::NoInstance);
    }

    #[test]
    fn request_without_a_socket_returns_none() {
        let dir = temp_config_dir();
        let reply = request(&dir.path().join("none.sock"), activate()).unwrap();
        assert_eq!(reply, None);
    }

    #[test]
    fn stale_socket_file_means_no_instance_and_is_replaced_by_bind() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        drop(UnixListener::bind(&socket).unwrap());
        assert!(socket.exists());

        assert_eq!(
            try_forward(&socket, &activate()).unwrap(),
            Forward::NoInstance
        );
        assert!(bind(&socket).is_ok());
    }

    #[test]
    fn bind_refuses_while_a_listener_is_alive() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        let _alive = bind(&socket).unwrap();
        let err = bind(&socket).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AddrInUse);
    }

    #[test]
    fn instance_that_closes_without_ack_rejects_the_message() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(stream).read_line(&mut line).unwrap();
        });

        let forwarded = try_forward(&socket, &activate()).unwrap();
        server.join().unwrap();
        assert_eq!(forwarded, Forward::Rejected);
    }

    #[test]
    fn socket_is_private_to_the_user() {
        let dir = temp_config_dir();
        let socket = dir.path().join("tt.sock");
        let _listener = bind(&socket).unwrap();
        let mode = std::fs::metadata(&socket).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
