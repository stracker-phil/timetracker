//! The timer picker: a small window under the tray icon to start, change or stop a timer from anywhere; on
//! macOS a panel (`panel.rs`).

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_positioner::{Position, WindowExt};

#[cfg(target_os = "macos")]
use super::panel;

const LABEL: &str = "timer";
const URL: &str = "src/windows/timer/index.html";
const SIZE: (f64, f64) = (380.0, 380.0);

#[derive(Clone, Copy)]
pub enum Place {
    /// Under the tray icon, for the tray's menu.
    UnderTray,
    /// On the screen the user works on, for the keyboard.
    OnFocusedScreen,
}

pub fn open(app: &AppHandle, place: Place) {
    let handle = app.clone();
    // The panel's AppKit calls need the main thread; the shortcut and the commands may come from others.
    if let Err(e) = app.run_on_main_thread(move || open_on_main_thread(&handle, place)) {
        log::warn!("Couldn't open the timer picker: {e}");
    }
}

fn open_on_main_thread(app: &AppHandle, place: Place) {
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
    move_to(&window, place);
    focus(&window);
}

fn move_to(window: &WebviewWindow, place: Place) {
    let moved = match place {
        Place::UnderTray => window.move_window(Position::TrayCenter).is_ok(),
        Place::OnFocusedScreen => on_focused_screen(window),
    };
    if !moved {
        let _ = window.move_window(Position::Center);
    }
}

#[cfg(target_os = "macos")]
fn on_focused_screen(window: &WebviewWindow) -> bool {
    let Ok(ns_window) = window.ns_window() else {
        return false;
    };
    // SAFETY: the window is alive and this runs on the main thread.
    unsafe { super::screen::place_on_focused_screen(ns_window) };
    true
}

#[cfg(not(target_os = "macos"))]
fn on_focused_screen(_window: &WebviewWindow) -> bool {
    false
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
        _ => open(app, Place::OnFocusedScreen),
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
