//! The system-wide shortcut that opens the timer picker (`Settings.timerShortcut`).

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use super::picker;
use crate::error::{AppError, AppResult};
use crate::store::keybindings::{accelerator, is_global};

/// Replaces the registered shortcut with `binding`; refused when it is not usable or another app holds it.
pub fn register(app: &AppHandle, binding: &str) -> AppResult<()> {
    let refused = |reason: String| {
        AppError::InvalidInput(format!(
            "Can't use {binding} as the timer shortcut: {reason}"
        ))
    };
    let accelerator = accelerator(binding)
        .filter(|_| is_global(binding))
        .ok_or_else(|| refused("it needs ⌘ or ⌃ and a key".into()))?;
    let shortcuts = app.global_shortcut();
    shortcuts
        .unregister_all()
        .map_err(|e| refused(e.to_string()))?;
    shortcuts
        .on_shortcut(accelerator.as_str(), |app, _, event| {
            if event.state == ShortcutState::Pressed {
                picker::toggle(app);
            }
        })
        .map_err(|e| refused(e.to_string()))
}
