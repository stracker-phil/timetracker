//! The menu bar (TimeTracker, File, Edit, View, Go, Day, Window, Help), built from one table
//! (`items::MENUS`). Every custom item's id is the bus command it runs; `page:<id>` items run the focused
//! window's own commands. The standard items (About, Services, Hide, Cut, Copy, Paste, Minimize, Enter Full
//! Screen) are macOS's own. Plain-letter and ⌥ keys stay in the labels (`Previous (P)`), never accelerators,
//! so they never take a typed key from a text field.

mod build;
mod items;
pub mod palette;
pub mod sync;

use tauri::{AppHandle, Manager};

use crate::bus;
use crate::state::AppState;

pub use build::build;
pub use items::{MENUS, default_binding, is_menu_command};
pub use sync::MenuSync;

/// Rebuilds the menu so changed key bindings show as accelerators.
pub fn rebuild(app: &AppHandle) {
    let state = app.state::<AppState>();
    let result = build(app, &state.keybindings).and_then(|menu| app.set_menu(menu));
    if let Err(e) = result {
        log::warn!("Couldn't rebuild the menu: {e}");
    }
}

/// Shows again what can run now and what is checked, after something the bus's checks read changed.
pub fn refresh(app: &AppHandle) {
    sync::apply(app);
}

/// Runs the command a clicked item names, from the menu bar or the tray; ids the bus doesn't know (macOS's
/// own items, the tray's recent timers) are left to others.
pub fn clicked(app: &AppHandle, id: &str) {
    if !app.state::<bus::Commands>().knows(id) {
        return;
    }
    // macOS toggled the checkmark on click; the bus decides it.
    if is_menu_command(id) {
        refresh(app);
    }
    bus::spawn(app, id);
}
