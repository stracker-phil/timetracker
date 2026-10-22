//! The `away` window: a small sheet on the screen in use. It stays open when it loses focus, since the user
//! may look something up first; closing it skips the prompt.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use super::AwayPromptState;

pub const LABEL: &str = "away";
const URL: &str = "src/windows/away/index.html";
const SIZE: (f64, f64) = (420.0, 430.0);

pub fn is_open(app: &AppHandle) -> bool {
    app.get_webview_window(LABEL).is_some()
}

/// Shows the window (building it on the main thread when needed) and focuses it.
pub fn open(app: &AppHandle) {
    let handle = app.clone();
    let shown = app.run_on_main_thread(move || {
        let window = match handle.get_webview_window(LABEL) {
            Some(window) => window,
            None => match build(&handle) {
                Ok(window) => window,
                Err(e) => {
                    log::warn!("Couldn't open the away prompt: {e}");
                    return;
                }
            },
        };
        let _ = window.show();
        let _ = window.set_focus();
    });
    if let Err(e) = shown {
        log::warn!("Couldn't open the away prompt: {e}");
    }
}

pub fn close(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.destroy();
    }
}

fn build(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App(URL.into()))
        .title("Away")
        .inner_size(SIZE.0, SIZE.1)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .center()
        .focused(true)
        .build()?;
    let app = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { .. } = event {
            app.state::<AwayPromptState>().clear();
        }
    });
    Ok(window)
}
