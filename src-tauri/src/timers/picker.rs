//! The timer picker: a small window under the tray icon to start, change or stop a timer from anywhere; on
//! macOS a panel (`panel.rs`).

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_positioner::{Position, WindowExt};

#[cfg(target_os = "macos")]
use super::panel;

const LABEL: &str = "timer";
const URL: &str = "src/windows/timer/index.html";
const SIZE: (f64, f64) = (380.0, 380.0);

/// Shows the picker under the tray icon (centered when the tray's place is unknown) and focuses it.
pub fn open(app: &AppHandle) {
    let handle = app.clone();
    // The panel's AppKit calls need the main thread; the shortcut and the commands may come from others.
    if let Err(e) = app.run_on_main_thread(move || open_on_main_thread(&handle)) {
        log::warn!("Couldn't open the timer picker: {e}");
    }
}

fn open_on_main_thread(app: &AppHandle) {
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
    focus(&window);
}

#[cfg(target_os = "macos")]
fn focus(window: &WebviewWindow) {
    let Ok(ns_window) = window.ns_window() else {
        return show_and_focus(window);
    };
    // SAFETY: the window is alive and this runs on the main thread.
    if unsafe { panel::make_panel(ns_window) } {
        unsafe { panel::show(ns_window) }
    } else {
        show_and_focus(window);
    }
}

#[cfg(not(target_os = "macos"))]
fn focus(window: &WebviewWindow) {
    show_and_focus(window);
}

fn show_and_focus(window: &WebviewWindow) {
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
