use tauri::{AppHandle, Emitter, Manager};

use crate::events::WINDOW_OPENED_EVENT;

const MAIN: &str = "main";

/// Skipped with `TIMETRACKER_BACKGROUND=1` in debug builds so scripts/dev never steal focus.
pub fn bring_to_front(app: &AppHandle) {
    #[cfg(debug_assertions)]
    if crate::dev::background() {
        return;
    }
    // Failing only means the window is gone; there is nothing left to bring forward.
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Shows the hidden or minimized main window and brings it to the front.
pub fn show_main(app: &AppHandle) {
    show_main_in_background(app);
    bring_to_front(app);
}

/// Shows the window without bringing the app to the front; emits `WINDOW_OPENED_EVENT` when it was hidden.
pub fn show_main_in_background(app: &AppHandle) {
    set_dock_icon(app, true);
    let opening = !main_visible(app);
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.show();
    }
    if opening && let Err(e) = app.emit(WINDOW_OPENED_EVENT, ()) {
        log::warn!("Couldn't tell the page the window opened: {e}");
    }
}

pub fn main_visible(app: &AppHandle) -> bool {
    app.get_webview_window(MAIN)
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false)
}

/// Hides the window and the Dock icon; the app keeps recording from the menu bar.
pub fn hide_to_menu_bar(app: &AppHandle) {
    hide_main(app);
    set_dock_icon(app, false);
}

fn set_dock_icon(app: &AppHandle, shown: bool) {
    #[cfg(target_os = "macos")]
    if let Err(e) = app.set_dock_visibility(shown) {
        log::warn!("Couldn't change the Dock icon: {e}");
    }
}

pub fn hide_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.hide();
    }
}
