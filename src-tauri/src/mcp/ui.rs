//! The tools that ask the running app over the single-instance socket, as the CLI does.

use std::path::{Path, PathBuf};

use crate::ipc::{self, IpcMessage};

pub const NOT_RUNNING: &str = "TimeTracker is not running";

pub async fn ask(socket: &Path, message: IpcMessage) -> Result<serde_json::Value, String> {
    let socket: PathBuf = socket.to_path_buf();
    tokio::task::spawn_blocking(move || ipc::request(&socket, message))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("Could not reach TimeTracker: {e}"))?
        .ok_or_else(|| NOT_RUNNING.to_string())?
        .into_result()
        .map_err(|e| e.message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn asking_without_a_listening_socket_says_timetracker_is_not_running() {
        let dir = tempfile::Builder::new()
            .prefix("tt")
            .tempdir_in("/tmp")
            .unwrap();

        let answer = ask(&dir.path().join("s.sock"), IpcMessage::Status).await;

        assert_eq!(answer, Err(NOT_RUNNING.to_string()));
        assert_eq!(NOT_RUNNING, "TimeTracker is not running");
    }
}
