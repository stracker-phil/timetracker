//! The menu bar item: a status line, Open and Quit.

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

use crate::window;

const TRAY_ID: &str = "main";
const STATUS_ID: &str = "status";
const OPEN_ID: &str = "open";
const QUIT_ID: &str = "quit";

/// Template image: macOS tints it for the menu bar's appearance.
const ICON: &[u8] = include_bytes!("../icons/tray/tray@2x.png");

/// The status item, kept so `set_status` can relabel it.
pub struct TrayStatus(MenuItem<Wry>);

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let status = MenuItem::with_id(app, STATUS_ID, "Tracking not started", false, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, OPEN_ID, "Open TimeTracker", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, QUIT_ID, "Quit TimeTracker", true, None::<&str>)?,
        ],
    )?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(ICON)?)
        .icon_as_template(true)
        .tooltip("TimeTracker")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            OPEN_ID => window::show_main(app),
            QUIT_ID => app.exit(0),
            _ => {}
        })
        .build(app)?;
    app.manage(TrayStatus(status));
    Ok(())
}

pub fn set_status(app: &AppHandle, text: &str) {
    let Some(status) = app.try_state::<TrayStatus>() else {
        return;
    };
    if let Err(e) = status.0.set_text(text) {
        log::warn!("Couldn't update the tray status: {e}");
    }
}
