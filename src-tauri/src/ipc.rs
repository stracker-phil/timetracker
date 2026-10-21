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
    /// Runs JavaScript in the window (used by scripts/dev to drive the UI).
    #[cfg(debug_assertions)]
    Eval {
        js: String,
    },
}

impl IpcMessage {
    /// The message type for logs; never its contents (scripts).
    pub fn kind(&self) -> &'static str {
        match self {
            IpcMessage::Activate { .. } => "Activate",
            IpcMessage::Error { .. } => "Error",
            #[cfg(debug_assertions)]
            IpcMessage::Eval { .. } => "Eval",
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
    pub error: Option<String>,
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
            Err(self.error.unwrap_or_else(|| "The request failed".into()))
        }
    }
}

/// What handling a message produced: a JSON result, or an error for the sender.
pub type HandlerResult = Result<serde_json::Value, String>;

#[derive(Deserialize)]
#[serde(untagged)]
enum Incoming {
    Request(IpcRequest),
    OneWay(IpcMessage),
}

const ACK: &str = "ok";
const MAX_MESSAGE_BYTES: u64 = 64 * 1024;
const CLIENT_TIMEOUT: Duration = Duration::from_secs(5);
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
fn send_line(socket: &Path, value: &impl Serialize) -> io::Result<Option<String>> {
    let mut stream = match UnixStream::connect(socket) {
        Ok(s) => s,
        Err(e) if no_listener(&e) => return Ok(None),
        Err(e) => return Err(e),
    };
    stream.set_read_timeout(Some(CLIENT_TIMEOUT))?;
    let mut line = serde_json::to_string(value).map_err(io::Error::other)?;
    line.push('\n');
    stream.write_all(line.as_bytes())?;
    let mut answer = String::new();
    BufReader::new(stream).read_line(&mut answer)?;
    Ok(Some(answer))
}

/// A timeout or other socket error is an `Err`: an instance may be alive but hung.
pub fn try_forward(socket: &Path, message: &IpcMessage) -> io::Result<Forward> {
    Ok(match send_line(socket, message)? {
        None => Forward::NoInstance,
        Some(answer) if answer.trim() == ACK => Forward::Delivered,
        Some(_) => Forward::Rejected,
    })
}

/// Sends `message` as a request and waits for its reply; `None` when nothing listens.
pub fn request(socket: &Path, message: IpcMessage) -> io::Result<Option<IpcReply>> {
    let request = IpcRequest {
        id: uuid::Uuid::new_v4().to_string(),
        message,
    };
    let Some(answer) = send_line(socket, &request)? else {
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
                log::warn!("Handling {kind} from another process failed: {e}");
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
                IpcMessage::Error { message } => Err(format!("failed: {message}")),
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
        assert_eq!(reply.error.as_deref(), Some("failed: x"));
        assert_eq!(reply.into_result(), Err("failed: x".to_string()));
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
