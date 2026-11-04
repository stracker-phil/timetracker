//! The bus inside the app: the app is its host, menus and the tray spawn commands, the page runs them
//! with `run_command`.

use tauri::{AppHandle, Manager};

use super::{CommandBus, Host};
use crate::error::{AppError, AppResult};
use crate::ipc::IpcMessage;
use crate::launch;

/// The app's commands, registered once at startup.
pub struct Commands(CommandBus<AppHandle>);

impl Commands {
    /// Every feature's commands.
    pub fn registered() -> Self {
        let mut bus = CommandBus::default();
        super::register(&mut bus);
        crate::timers::commands::register(&mut bus);
        crate::tracker::commands::register(&mut bus);
        Self(bus)
    }
}

impl std::ops::Deref for Commands {
    type Target = CommandBus<AppHandle>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A command that fails from a menu or the tray shows as an error toast in the window.
impl Host for AppHandle {
    fn report_failure(&self, _id: &str, error: &AppError) {
        launch::queue(
            self,
            IpcMessage::Error {
                message: error.to_string(),
            },
        );
    }
}

/// Runs a command without waiting for it (menu and tray items).
pub fn spawn(app: &AppHandle, id: &str) {
    let app = app.clone();
    let id = id.to_string();
    tauri::async_runtime::spawn(async move {
        let commands = app.state::<Commands>();
        commands.run(app.clone(), &id).await;
    });
}

/// The page runs bus commands by id; a failure comes back as the error.
#[tauri::command]
pub async fn run_command(app: AppHandle, id: String) -> AppResult<()> {
    let work = app.state::<Commands>().start(app.clone(), &id)?;
    work.await
}
