//! Single-instance handoff: a second `timetracker` process sends its request over a Unix socket
//! (mode 0600, same user only) to the running app and exits.
//!
//! One JSON line per connection. A bare `IpcMessage` (the one-way form) is acknowledged with
//! the line `ok` before it is handled; an `IpcRequest` is answered with an `IpcReply` line
//! once it is handled.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::OwnedWriteHalf;

use crate::activity::ActivityInput;
use crate::db::time::{Day, Ms};
use crate::error::{AppError, ErrorKind};
use crate::recordings::cli::RecordRequest;
use crate::reports::Range;
use crate::reports::cli::{ReportOutput, ReportRequest};
use crate::scripting::writes::EditRequest;
use crate::timers::commands::{TimerStartRequest, TimerUpdateRequest};
use crate::tracker::PauseUntil;

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

#[derive(Deserialize)]
#[serde(untagged)]
enum Incoming {
    Request(IpcRequest),
    OneWay(IpcMessage),
}

const ACK: &str = "ok";
/// Longer lines get no answer; the CLI splits larger activity batches (`cli::split`).
pub const MAX_MESSAGE_BYTES: u64 = 64 * 1024;
const CLIENT_TIMEOUT: Duration = Duration::from_secs(5);
/// Seeding years of data takes seconds.
#[cfg(debug_assertions)]
const SEED_TIMEOUT: Duration = Duration::from_secs(120);
const ACCEPT_BACKOFF: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forward {
    Delivered,
    /// Nothing listens on the socket; this process may become the app.
    NoInstance,
    /// An instance listens but did not acknowledge the message.
    Rejected,
}

fn no_listener(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
    )
}

/// Sends `value` as one line and reads the answer line; `None` when nothing listens.
fn send_line(
    socket: &Path,
    value: &impl Serialize,
    timeout: Duration,
) -> io::Result<Option<String>> {
    let mut stream = match UnixStream::connect(socket) {
        Ok(s) => s,
        Err(e) if no_listener(&e) => return Ok(None),
        Err(e) => return Err(e),
    };
    stream.set_read_timeout(Some(timeout))?;
    let mut line = serde_json::to_string(value).map_err(io::Error::other)?;
    line.push('\n');
    stream.write_all(line.as_bytes())?;
    let mut answer = String::new();
    BufReader::new(stream).read_line(&mut answer)?;
    Ok(Some(answer))
}

/// A timeout or other socket error is an `Err`: an instance may be alive but hung.
pub fn try_forward(socket: &Path, message: &IpcMessage) -> io::Result<Forward> {
    Ok(match send_line(socket, message, CLIENT_TIMEOUT)? {
        None => Forward::NoInstance,
        Some(answer) if answer.trim() == ACK => Forward::Delivered,
        Some(_) => Forward::Rejected,
    })
}

/// Sends `message` as a request and waits for its reply; `None` when nothing listens.
pub fn request(socket: &Path, message: IpcMessage) -> io::Result<Option<IpcReply>> {
    let timeout = message.reply_timeout();
    let request = IpcRequest {
        id: uuid::Uuid::new_v4().to_string(),
        message,
    };
    let Some(answer) = send_line(socket, &request, timeout)? else {
        return Ok(None);
    };
    match serde_json::from_str::<IpcReply>(&answer) {
        Ok(reply) if reply.id == request.id => Ok(Some(reply)),
        _ => Err(io::Error::other(
            "the running TimeTracker instance did not answer the request; it may be older than this command",
        )),
    }
}

/// Binds the socket, replacing a stale file but never a live listener. Blocking std
/// listener, so it can be created outside the async runtime; `serve` converts it.
pub fn bind(socket: &Path) -> io::Result<UnixListener> {
    match UnixStream::connect(socket) {
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "another TimeTracker instance is listening on the socket",
            ));
        }
        Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => std::fs::remove_file(socket)?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let listener = UnixListener::bind(socket)?;
    listener.set_nonblocking(true)?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

/// Calls `handler` for every message on the listener and writes each request's result back to its client.
pub async fn serve<F, Fut>(listener: UnixListener, handler: F)
where
    F: Fn(IpcMessage) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = HandlerResult> + Send + 'static,
{
    let listener = match tokio::net::UnixListener::from_std(listener) {
        Ok(l) => l,
        Err(e) => {
            log::error!("Cannot serve IPC socket: {e}");
            return;
        }
    };
    let handler = Arc::new(handler);
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                log::warn!("IPC accept failed: {e}");
                tokio::time::sleep(ACCEPT_BACKOFF).await;
                continue;
            }
        };
        tokio::spawn(handle_connection(stream, handler.clone()));
    }
}

async fn handle_connection<F, Fut>(stream: tokio::net::UnixStream, handler: Arc<F>)
where
    F: Fn(IpcMessage) -> Fut,
    Fut: Future<Output = HandlerResult>,
{
    let (incoming, mut write) = match tokio::time::timeout(CLIENT_TIMEOUT, receive(stream)).await {
        Ok(Some(received)) => received,
        Ok(None) => return,
        Err(_) => {
            log::warn!("IPC client timed out");
            return;
        }
    };
    match incoming {
        Incoming::OneWay(message) => {
            write_line(&mut write, ACK).await;
            let kind = message.kind();
            if let Err(e) = handler(message).await {
                log::warn!("Handling {kind} from another process failed: {}", e.message);
            }
        }
        Incoming::Request(IpcRequest { id, message }) => {
            let reply = IpcReply::new(id, handler(message).await);
            match serde_json::to_string(&reply) {
                Ok(line) => write_line(&mut write, &line).await,
                Err(e) => log::warn!("IPC reply not serializable: {e}"),
            }
        }
    }
}

async fn write_line(write: &mut OwnedWriteHalf, line: &str) {
    if let Err(e) = write.write_all(format!("{line}\n").as_bytes()).await {
        log::debug!("IPC reply failed: {e}");
    }
}

/// Reads one message or request; `None` when the client is refused.
async fn receive(stream: tokio::net::UnixStream) -> Option<(Incoming, OwnedWriteHalf)> {
    if !same_user(&stream) {
        log::warn!("Refusing IPC connection from another user");
        return None;
    }
    let (read, write) = stream.into_split();
    let mut line = String::new();
    if let Err(e) = tokio::io::BufReader::new(read.take(MAX_MESSAGE_BYTES))
        .read_line(&mut line)
        .await
    {
        log::warn!("IPC read failed: {e}");
        return None;
    }
    if !line.ends_with('\n') {
        log::warn!("Ignoring incomplete or oversized IPC message");
        return None;
    }
    match serde_json::from_str::<Incoming>(&line) {
        Ok(incoming) => Some((incoming, write)),
        Err(e) => {
            log::warn!("Ignoring malformed IPC message: {e}");
            None
        }
    }
}

fn same_user(stream: &tokio::net::UnixStream) -> bool {
    // SAFETY: getuid has no preconditions and cannot fail.
    let uid = unsafe { libc::getuid() };
    stream.peer_cred().is_ok_and(|cred| cred.uid() == uid)
}

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
        tokio::spawn(serve(listener, move |m| {
            let _ = tx.send(m);
            async { Ok(serde_json::Value::Null) }
        }));
        rx
    }

    fn serve_answering(socket: &Path) {
        let listener = bind(socket).unwrap();
        tokio::spawn(serve(listener, |m| async move {
            match m {
                IpcMessage::Error { message } => Err(IpcError::new(
                    ErrorKind::Overlap,
                    format!("failed: {message}"),
                )),
                other => Ok(serde_json::json!({ "handled": other.kind() })),
            }
        }));
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
                project: "acme".into(),
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
    }

    #[cfg(debug_assertions)]
    #[test]
    fn a_dev_seed_message_is_of_the_kind_dev_seed_and_keeps_its_wire_shape() {
        let message = IpcMessage::DevSeed {
            years: 3,
            yes: true,
        };
        let wire = serde_json::json!({ "type": "devSeed", "years": 3, "yes": true });

        assert_eq!(message.kind(), "DevSeed");
        assert_eq!(serde_json::to_value(&message).unwrap(), wire);
        assert_eq!(serde_json::from_value::<IpcMessage>(wire).unwrap(), message);
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
                project: "acme".into(),
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
                IpcMessage::TrackerStatus,
                serde_json::json!({ "type": "trackerStatus" }),
            ),
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
