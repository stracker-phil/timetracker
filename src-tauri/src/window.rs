use tauri::{AppHandle, Manager};

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

/// Shows the window without bringing the app to the front.
pub fn show_main_in_background(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.show();
    }
}

pub fn main_visible(app: &AppHandle) -> bool {
    app.get_webview_window(MAIN)
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false)
}

pub fn hide_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.hide();
    }
}
