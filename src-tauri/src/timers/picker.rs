//! The timer picker: a small window under the tray icon to start or stop a timer from anywhere.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_positioner::{Position, WindowExt};

const LABEL: &str = "timer";
const URL: &str = "src/windows/timer/index.html";
const SIZE: (f64, f64) = (380.0, 380.0);

/// Shows the picker under the tray icon (centered when the tray's place is unknown) and focuses it.
pub fn open(app: &AppHandle) {
    let window = match app.get_webview_window(LABEL) {
        Some(window) => window,
        None => match build(app) {
            Ok(window) => window,
            Err(e) => {
                log::warn!("Couldn't open the timer picker: {e}");
                return;
            }
        },
    };
    if window.move_window(Position::TrayCenter).is_err() {
        let _ = window.move_window(Position::Center);
    }
    let _ = window.show();
    let _ = window.set_focus();
}

pub fn toggle(app: &AppHandle) {
    match app.get_webview_window(LABEL) {
        Some(window) if window.is_visible().unwrap_or(false) => hide(app),
        _ => open(app),
    }
}

pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.hide();
    }
}

fn build(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App(URL.into()))
        .title("Start Timer")
        .inner_size(SIZE.0, SIZE.1)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build()?;
    // Like a menu: it goes away when the user turns to something else.
    let app = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            hide(&app);
        }
    });
    Ok(window)
}
