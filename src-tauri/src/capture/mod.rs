//! What the tracker reads from the system. `Source` hides macOS (`macos.rs`) so the tracker runs
//! against `FakeSource` in tests.

pub mod browsers;
#[cfg(any(test, feature = "test-util"))]
pub mod fake;
#[cfg(target_os = "macos")]
mod macos;
pub mod private;
pub mod url;

use std::sync::Arc;

use serde::Serialize;
use tokio::sync::broadcast;
use ts_rs::TS;

use crate::db::time::Ms;
use crate::store::settings::CaptureSettings;

/// The frontmost app and its focused window at `at`. `private` marks a private browser window.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub at: Ms,
    pub app: String,
    pub bundle_id: Option<String>,
    pub pid: i32,
    pub title: String,
    pub url: Option<String>,
    pub path: Option<String>,
    pub private: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemEvent {
    /// Another app became frontmost; the tracker polls at once instead of on its next tick.
    Activated,
    Sleep,
    Wake,
    Lock,
    Unlock,
    /// Fast user switching moved to another account.
    SessionInactive,
    SessionActive,
}

/// The Accessibility permission, which window titles need.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum PermissionStatus {
    Granted,
    Denied,
    Unknown,
}

pub trait Source: Send + Sync {
    /// `None` when nothing is frontmost (the login window). `capture` says whether reading the address and
    /// the document path is worth the cost; the tracker drops them anyway when they are off.
    fn snapshot(&self, capture: &CaptureSettings) -> Option<Snapshot>;
    /// Seconds since the last keyboard, mouse or trackpad input.
    fn idle_seconds(&self) -> f64;
    fn events(&self) -> broadcast::Receiver<SystemEvent>;
    fn permission(&self) -> PermissionStatus;
    /// Shows the system's permission prompt, at most once per app run.
    fn request_permission(&self);
}

#[cfg(target_os = "macos")]
pub fn new_source(app: &tauri::AppHandle) -> Arc<dyn Source> {
    macos::MacSource::new(app)
}
