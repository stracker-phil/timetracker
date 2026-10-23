//! The system-wide shortcut that opens the timer picker (`Settings.timerShortcut`).

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use super::picker;
use crate::error::{AppError, AppResult};
use crate::store::keybindings::{accelerator, is_global};

/// Replaces the registered shortcut with `binding`, or only removes it when `None`; refused when the
/// binding is not usable or another app holds it.
pub fn register(app: &AppHandle, binding: Option<&str>) -> AppResult<()> {
    let refused = |reason: String| {
        AppError::InvalidInput(format!(
            "Can't use {} as the timer shortcut: {reason}",
            binding.unwrap_or("none")
        ))
    };
    // Checked before the current one goes, so a refused binding leaves it working.
    let accelerator = binding
        .map(|b| {
            accelerator(b)
                .filter(|_| is_global(b))
                .ok_or_else(|| refused("it needs ⌘ or ⌃ and a key".into()))
        })
        .transpose()?;
    let shortcuts = app.global_shortcut();
    shortcuts
        .unregister_all()
        .map_err(|e| refused(e.to_string()))?;
    let Some(accelerator) = accelerator else {
        return Ok(());
    };
    shortcuts
        .on_shortcut(accelerator.as_str(), |app, _, event| {
            if event.state == ShortcutState::Pressed {
                picker::toggle(app);
            }
        })
        .map_err(|e| refused(e.to_string()))
}
