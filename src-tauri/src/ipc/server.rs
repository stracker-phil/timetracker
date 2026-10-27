//! The app's side: the listener, one task per connection, and `watch` connections kept open.

use std::io;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, Take};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

use super::{
    ACK, CLIENT_TIMEOUT, HandlerResult, IpcMessage, IpcReply, IpcRequest, MAX_MESSAGE_BYTES,
};
use crate::watch::{self, Watchers};

const ACCEPT_BACKOFF: Duration = Duration::from_millis(100);

#[derive(Deserialize)]
#[serde(untagged)]
enum Incoming {
    Request(IpcRequest),
    OneWay(IpcMessage),
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

/// Calls `handler` for every message on the listener and writes each request's result back to its client;
/// a `watch` request then gets the changes `watchers` is sent.
pub async fn serve<F, Fut>(listener: UnixListener, watchers: Watchers, handler: F)
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
        tokio::spawn(handle_connection(stream, handler.clone(), watchers.clone()));
    }
}

async fn handle_connection<F, Fut>(
    stream: tokio::net::UnixStream,
    handler: Arc<F>,
    watchers: Watchers,
) where
    F: Fn(IpcMessage) -> Fut,
    Fut: Future<Output = HandlerResult>,
{
    let (incoming, read, mut write) =
        match tokio::time::timeout(CLIENT_TIMEOUT, receive(stream)).await {
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
            // Subscribed before the reply is built, so no change falls between the two.
            let events = matches!(message, IpcMessage::Watch).then(|| watchers.subscribe());
            let reply = IpcReply::new(id, handler(message).await);
            match serde_json::to_string(&reply) {
                Ok(line) => write_line(&mut write, &line).await,
                Err(e) => log::warn!("IPC reply not serializable: {e}"),
            }
            if reply.ok
                && let Some(events) = events
            {
                watch::stream(events, read, write).await;
            }
        }
    }
}

async fn write_line(write: &mut OwnedWriteHalf, line: &str) {
    if let Err(e) = write.write_all(format!("{line}\n").as_bytes()).await {
        log::debug!("IPC reply failed: {e}");
    }
}

type Reader = tokio::io::BufReader<Take<OwnedReadHalf>>;

/// Reads one message or request; `None` when the client is refused. The reader stays for `watch`,
/// which ends when the client closes it.
async fn receive(stream: tokio::net::UnixStream) -> Option<(Incoming, Reader, OwnedWriteHalf)> {
    if !same_user(&stream) {
        log::warn!("Refusing IPC connection from another user");
        return None;
    }
    let (read, write) = stream.into_split();
    let mut line = String::new();
    let mut reader = tokio::io::BufReader::new(read.take(MAX_MESSAGE_BYTES));
    if let Err(e) = reader.read_line(&mut line).await {
        log::warn!("IPC read failed: {e}");
        return None;
    }
    if !line.ends_with('\n') {
        log::warn!("Ignoring incomplete or oversized IPC message");
        return None;
    }
    match serde_json::from_str::<Incoming>(&line) {
        Ok(incoming) => Some((incoming, reader, write)),
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
