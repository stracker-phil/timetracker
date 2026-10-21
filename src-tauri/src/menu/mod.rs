mod items;
pub mod sync;

use tauri::menu::{
    AboutMetadata, CheckMenuItemBuilder, Menu, MenuItemBuilder, MenuItemKind, SubmenuBuilder,
};
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::events::COMMAND_EVENT;
use crate::state::AppState;
use crate::store::keybindings::{KeybindingsStore, accelerator, is_chord, key_label};

pub use items::default_binding;
use items::{binding_owner, menu_item};
pub use sync::MenuSync;
use sync::{MenuState, apply, current_state, item_state};

/// A binding without ⌘ or ⌃ would take its key from text fields as a menu accelerator, so its keys go
/// into the label instead: `Previous (P)`.
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
    state: Option<&MenuState>,
    id: &str,
) -> tauri::Result<MenuItemKind<Wry>> {
    let Some(def) = menu_item(id) else {
        log::warn!("Menu item {id} is missing from MENU_ITEMS");
        return Ok(MenuItemKind::MenuItem(
            MenuItemBuilder::with_id(id, id).build(app)?,
        ));
    };
    let (label, accel) = label_and_accelerator(def.label, bindings.menu_binding(binding_owner(id)));
    let shown = item_state(state, id);
    if def.check {
        let mut builder = CheckMenuItemBuilder::with_id(id, label)
            .enabled(shown.enabled)
            .checked(shown.checked);
        if let Some(accel) = accel {
            builder = builder.accelerator(accel);
        }
        return Ok(MenuItemKind::Check(builder.build(app)?));
    }
    let mut builder = MenuItemBuilder::with_id(id, label).enabled(shown.enabled);
    if let Some(accel) = accel {
        builder = builder.accelerator(accel);
    }
    Ok(MenuItemKind::MenuItem(builder.build(app)?))
}

/// Adds groups of items, with a separator before each group but the first.
fn groups<'m, M: Manager<Wry>>(
    app: &'m M,
    mut builder: SubmenuBuilder<'m, Wry, M>,
    bindings: &KeybindingsStore,
    state: Option<&MenuState>,
    groups: &[&[&str]],
) -> tauri::Result<SubmenuBuilder<'m, Wry, M>> {
    for (i, group) in groups.iter().enumerate() {
        if i > 0 {
            builder = builder.separator();
        }
        for id in *group {
            builder = builder.item(&build_item(app, bindings, state, id)?);
        }
    }
    Ok(builder)
}

pub fn build<M: Manager<Wry>>(app: &M, bindings: &KeybindingsStore) -> tauri::Result<Menu<Wry>> {
    let state = current_state(app);
    let state = state.as_ref();
    let sub = |title: &str, ids: &[&[&str]]| {
        groups(app, SubmenuBuilder::new(app, title), bindings, state, ids)
    };
    let app_menu = SubmenuBuilder::new(app, "TimeTracker")
        .about(Some(AboutMetadata::default()))
        .separator()
        .item(&build_item(app, bindings, state, "settings.toggle")?)
        .separator()
        .item(&build_item(app, bindings, state, "app.launchAtLogin")?)
        .item(&build_item(app, bindings, state, "welcome.show")?)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .item(&build_item(app, bindings, state, "app.quit")?)
        .build()?;
    let file = sub(
        "File",
        &[
            &["recording.new"],
            &["timer.start", "timer.stop"],
            &["projects.new", "clients.new", "rules.new"],
            &["data.exportBackup"],
            &["window.hide"],
        ],
    )?
    .build()?;
    let edit = sub("Edit", &[&["edit.undo", "edit.redo"]])?
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .separator()
        .item(&build_item(app, bindings, state, "recording.delete")?)
        .separator()
        .item(&build_item(app, bindings, state, "reports.copyText")?)
        .build()?;
    let view = sub(
        "View",
        &[
            &[
                "view.day",
                "view.week",
                "view.month",
                "view.reports",
                "view.projects",
                "view.rules",
            ],
            &["day.toggleList"],
            &["sidebar.toggle"],
            &["zoom.in", "zoom.out", "zoom.reset"],
            &["day.settings"],
        ],
    )?
    .separator()
    .fullscreen()
    .build()?;
    let go = sub(
        "Go",
        &[
            &["nav.prev", "nav.next", "nav.today", "nav.goToDate"],
            &["day.nextBlock", "day.prevBlock"],
            &["recording.next", "recording.prev"],
        ],
    )?
    .build()?;
    let day = sub(
        "Day",
        &[
            &["recording.create", "recording.edit"],
            &[
                "recording.moveToPrevDay",
                "recording.moveToNextDay",
                "recording.copyToPrevDay",
                "recording.copyToNextDay",
            ],
            &["suggestion.accept", "suggestion.acceptRule"],
            &["activity.deleteDay", "activity.deleteRange"],
        ],
    )?
    .build()?;
    let window = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .separator()
        .bring_all_to_front()
        .build()?;
    let help = sub(
        "Help",
        &[
            &["shortcuts.show", "palette.toggle"],
            &["logs.open", "data.reveal", "help.readme"],
        ],
    )?
    .build()?;
    Menu::with_items(
        app,
        &[&app_menu, &file, &edit, &view, &go, &day, &window, &help],
    )
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
    let Some(def) = menu_item(id) else {
        return;
    };
    if id == "app.quit" {
        app.exit(0);
        return;
    }
    // macOS toggled the checkmark on click; the page's next report decides it.
    if def.check {
        apply(app);
    }
    if let Err(e) = app.emit(COMMAND_EVENT, id) {
        log::warn!("Couldn't forward menu command {id}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::items::MENU_ITEMS;
    use super::*;

    #[test]
    fn every_menu_item_has_a_label() {
        for item in MENU_ITEMS {
            assert!(!item.label.is_empty(), "{} has no label", item.id);
        }
    }

    #[test]
    fn the_timer_commands_are_menu_items_with_their_default_chords() {
        for (id, label, accelerator_text) in [
            ("timer.start", "Start Timer…", "CmdOrCtrl+T"),
            ("timer.stop", "Stop Timer", "CmdOrCtrl+Shift+."),
        ] {
            let def = menu_item(id).unwrap();
            assert_eq!(
                label_and_accelerator(def.label, Some(def.default_binding.into())),
                (label.to_string(), Some(accelerator_text.to_string())),
                "{id}"
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
    fn single_letter_alt_and_enter_defaults_go_into_the_titles() {
        for (id, expected) in [
            ("nav.prev", "Previous (P)"),
            ("nav.next", "Next (N)"),
            ("recording.create", "Record Selection (↵)"),
            ("recording.delete", "Delete Recording (⌫)"),
            (
                "recording.moveToPrevDay",
                "Move Recording to Previous Day (⌥←)",
            ),
            (
                "recording.copyToNextDay",
                "Copy Recording to Next Day (⌥⇧→)",
            ),
        ] {
            let def = menu_item(id).unwrap();
            let binding = default_binding(binding_owner(id)).map(str::to_string);
            assert_eq!(
                label_and_accelerator(def.label, binding),
                (expected.to_string(), None),
                "{id}"
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
