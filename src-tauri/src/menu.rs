use tauri::menu::{AboutMetadata, Menu, MenuItem, MenuItemBuilder, Submenu, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::events::COMMAND_EVENT;
use crate::state::AppState;
use crate::store::keybindings::{KeybindingsStore, accelerator, is_chord, key_label};

pub struct MenuItemDef {
    pub id: &'static str,
    pub label: &'static str,
    /// Mirrors the command's `defaultBinding` in `src/commands`; empty means none.
    pub default_binding: &'static str,
}

const fn item(id: &'static str, label: &'static str, default_binding: &'static str) -> MenuItemDef {
    MenuItemDef {
        id,
        label,
        default_binding,
    }
}

pub const MENU_ITEMS: &[MenuItemDef] = &[
    item("settings.toggle", "Settings…", "Mod+,"),
    item("app.quit", "Quit TimeTracker", "Mod+Q"),
    item("palette.toggle", "Command Palette…", "Mod+K"),
    item("sidebar.toggle", "Toggle Sidebar", "Mod+Ctrl+S"),
    item("view.day", "Day", "Mod+1"),
    item("view.week", "Week", "Mod+2"),
    item("view.month", "Month", "Mod+3"),
    item("view.reports", "Reports", "Mod+4"),
    item("view.projects", "Projects", "Mod+5"),
    item("zoom.in", "Zoom In", "Mod+="),
    item("zoom.out", "Zoom Out", "Mod+-"),
    item("zoom.reset", "Reset Zoom", "Mod+0"),
    item("window.hide", "Close Window", "Mod+W"),
    item("shortcuts.show", "Keyboard Shortcuts", "Shift+?"),
    item("logs.open", "Open Log Folder", ""),
    item("recording.create", "New Recording from Selection", ""),
    item("recording.edit", "Edit Recording", ""),
    item("recording.delete", "Delete Recording", "Backspace"),
];

fn menu_item(id: &str) -> Option<&'static MenuItemDef> {
    MENU_ITEMS.iter().find(|def| def.id == id)
}

pub fn is_menu_command(id: &str) -> bool {
    menu_item(id).is_some()
}

/// A binding without ⌘ or ⌃ would take its key from text fields as a menu accelerator, so its keys go
/// into the label instead: `Copy Value (Y)`.
fn label_and_accelerator(label: &str, binding: Option<String>) -> (String, Option<String>) {
    let Some(binding) = binding else {
        return (label.to_string(), None);
    };
    if !is_chord(&binding) {
        return match key_label(&binding) {
            Some(keys) => (format!("{label} ({keys})"), None),
            None => (label.to_string(), None),
        };
    }
    let accel = accelerator(&binding);
    if accel.is_none() {
        log::warn!("No menu accelerator for {label} = {binding:?}");
    }
    (label.to_string(), accel)
}

fn build_item<M: Manager<Wry>>(
    app: &M,
    bindings: &KeybindingsStore,
    id: &str,
) -> tauri::Result<MenuItem<Wry>> {
    let label = menu_item(id).map_or_else(
        || {
            log::warn!("Menu item {id} is missing from MENU_ITEMS");
            id
        },
        |def| def.label,
    );
    let (label, accel) = label_and_accelerator(label, bindings.menu_binding(id));
    let mut builder = MenuItemBuilder::with_id(id, label);
    if let Some(accel) = accel {
        builder = builder.accelerator(accel);
    }
    builder.build(app)
}

fn submenu<M: Manager<Wry>>(
    app: &M,
    bindings: &KeybindingsStore,
    title: &str,
    groups: &[&[&str]],
) -> tauri::Result<Submenu<Wry>> {
    let mut builder = SubmenuBuilder::new(app, title);
    for (i, group) in groups.iter().enumerate() {
        if i > 0 {
            builder = builder.separator();
        }
        for id in *group {
            builder = builder.item(&build_item(app, bindings, id)?);
        }
    }
    builder.build()
}

pub fn build<M: Manager<Wry>>(app: &M, bindings: &KeybindingsStore) -> tauri::Result<Menu<Wry>> {
    let app_menu = SubmenuBuilder::new(app, "TimeTracker")
        .about(Some(AboutMetadata::default()))
        .separator()
        .item(&build_item(app, bindings, "settings.toggle")?)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .item(&build_item(app, bindings, "app.quit")?)
        .build()?;
    let edit = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    let view = submenu(
        app,
        bindings,
        "View",
        &[
            &["sidebar.toggle", "palette.toggle"],
            &[
                "view.day",
                "view.week",
                "view.month",
                "view.reports",
                "view.projects",
            ],
            &["zoom.in", "zoom.out", "zoom.reset"],
        ],
    )?;
    let recording = submenu(
        app,
        bindings,
        "Recording",
        &[&["recording.create", "recording.edit", "recording.delete"]],
    )?;
    let window = SubmenuBuilder::new(app, "Window")
        .item(&build_item(app, bindings, "window.hide")?)
        .minimize()
        .maximize()
        .fullscreen()
        .separator()
        .bring_all_to_front()
        .build()?;
    let help = submenu(
        app,
        bindings,
        "Help",
        &[&["shortcuts.show"], &["logs.open"]],
    )?;
    Menu::with_items(app, &[&app_menu, &edit, &view, &recording, &window, &help])
}

/// Rebuilds the menu so changed key bindings show as accelerators.
pub fn rebuild(app: &AppHandle) {
    let state = app.state::<AppState>();
    let result = build(app, &state.keybindings).and_then(|menu| app.set_menu(menu));
    if let Err(e) = result {
        log::warn!("Couldn't rebuild the menu: {e}");
    }
}

/// Quitting is handled here, so it works even when the page is broken; every other item runs
/// its command in the page.
pub fn forward(app: &AppHandle, id: &str) {
    if !is_menu_command(id) {
        return;
    }
    if id == "app.quit" {
        app.exit(0);
        return;
    }
    if let Err(e) = app.emit(COMMAND_EVENT, id) {
        log::warn!("Couldn't forward menu command {id}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_item_ids_are_unique_and_labelled() {
        for (i, item) in MENU_ITEMS.iter().enumerate() {
            assert!(!item.label.is_empty(), "{} has no label", item.id);
            assert!(
                MENU_ITEMS[..i].iter().all(|other| other.id != item.id),
                "duplicate id {}",
                item.id
            );
        }
    }

    #[test]
    fn only_listed_ids_are_menu_commands() {
        for item in MENU_ITEMS {
            assert!(is_menu_command(item.id), "{}", item.id);
        }
        assert!(!is_menu_command("nope.unknown"));
        assert!(!is_menu_command(""));
    }

    #[test]
    fn default_chord_bindings_convert_to_accelerators() {
        for item in MENU_ITEMS.iter().filter(|i| is_chord(i.default_binding)) {
            assert!(
                accelerator(item.default_binding).is_some(),
                "{}: {}",
                item.id,
                item.default_binding
            );
        }
    }

    #[test]
    fn chord_binding_becomes_an_accelerator_with_the_plain_label() {
        assert_eq!(
            label_and_accelerator("Close Window", Some("Mod+W".into())),
            ("Close Window".to_string(), Some("CmdOrCtrl+W".to_string()))
        );
    }

    #[test]
    fn plain_key_binding_is_appended_to_the_label_without_accelerator() {
        for (label, binding, expected) in [
            ("Copy Value", "Y", "Copy Value (Y)"),
            ("Copy Value", "Shift+Y", "Copy Value (⇧Y)"),
            ("Keyboard Shortcuts", "Shift+?", "Keyboard Shortcuts (⇧?)"),
        ] {
            assert_eq!(
                label_and_accelerator(label, Some(binding.into())),
                (expected.to_string(), None),
                "{binding}"
            );
        }
    }

    #[test]
    fn missing_binding_keeps_the_label_unchanged() {
        assert_eq!(
            label_and_accelerator("Open Log Folder", None),
            ("Open Log Folder".to_string(), None)
        );
    }

    #[test]
    fn sidebar_toggle_gets_command_control_s_accelerator() {
        let def = menu_item("sidebar.toggle").unwrap();
        assert_eq!(
            label_and_accelerator(def.label, Some(def.default_binding.into())),
            (
                "Toggle Sidebar".to_string(),
                Some("CmdOrCtrl+Ctrl+S".to_string())
            )
        );
    }
}
