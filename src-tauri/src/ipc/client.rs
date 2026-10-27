//! The sending side: the CLI, a second launch and the MCP server ask the running app.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use serde::Serialize;

use super::{ACK, CLIENT_TIMEOUT, IpcMessage, IpcReply, IpcRequest};

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

/// Sends `value` as one line and reads the answer line, keeping the connection for any lines after it;
/// `None` when nothing listens.
fn exchange(
    socket: &Path,
    value: &impl Serialize,
    timeout: Duration,
) -> io::Result<Option<(String, BufReader<UnixStream>)>> {
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
    let mut reader = BufReader::new(stream);
    reader.read_line(&mut answer)?;
    Ok(Some((answer, reader)))
}

fn send_line(
    socket: &Path,
    value: &impl Serialize,
    timeout: Duration,
) -> io::Result<Option<String>> {
    Ok(exchange(socket, value, timeout)?.map(|(answer, _)| answer))
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
    Ok(send_request(socket, message)?.map(|(reply, _)| reply))
}

/// Like `request`, and keeps the connection, without a read timeout, for the lines that follow the
/// reply (`watch`).
pub fn open_request(
    socket: &Path,
    message: IpcMessage,
) -> io::Result<Option<(IpcReply, BufReader<UnixStream>)>> {
    let Some((reply, reader)) = send_request(socket, message)? else {
        return Ok(None);
    };
    // macOS refuses this once the app closed the connection; the reader then just ends.
    let _ = reader.get_ref().set_read_timeout(None);
    Ok(Some((reply, reader)))
}

fn send_request(
    socket: &Path,
    message: IpcMessage,
) -> io::Result<Option<(IpcReply, BufReader<UnixStream>)>> {
    let timeout = message.reply_timeout();
    let request = IpcRequest {
        id: uuid::Uuid::new_v4().to_string(),
        message,
    };
    let Some((answer, reader)) = exchange(socket, &request, timeout)? else {
        return Ok(None);
    };
    match serde_json::from_str::<IpcReply>(&answer) {
        Ok(reply) if reply.id == request.id => Ok(Some((reply, reader))),
        _ => Err(io::Error::other(
            "the running TimeTracker instance did not answer the request; it may be older than this command",
        )),
    }
}
