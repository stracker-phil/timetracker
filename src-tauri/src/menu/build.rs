//! Building the menu bar from `MENUS`: every item labelled with its binding, enabled and checked as the
//! bus says, with macOS's standard items around them.

use tauri::menu::{
    AboutMetadata, CheckMenuItemBuilder, Menu, MenuItemBuilder, MenuItemKind, SubmenuBuilder,
};
use tauri::{AppHandle, Manager, Wry};

use super::items::{
    APP_MENU, DAY_MENU, EDIT_MENU, FILE_MENU, GO_MENU, HELP_MENU, Item, MenuDef, VIEW_MENU,
    binding_key,
};
use crate::bus::{self, CommandStatus};
use crate::store::keybindings::{KeybindingsStore, accelerator, is_chord, key_label};

/// A binding without ⌘ or ⌃ would take its key from text fields as a menu accelerator, so its keys go
/// into the label instead: `Previous (P)`.
pub(super) fn label_and_accelerator(
    label: &str,
    binding: Option<String>,
) -> (String, Option<String>) {
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

/// What every item is built with: the app, its bindings and which commands run now.
struct Ctx<'a> {
    app: &'a AppHandle,
    bindings: &'a KeybindingsStore,
    commands: &'a dyn CommandStatus,
}

type Builder<'a> = SubmenuBuilder<'a, Wry, AppHandle>;

fn build_item(ctx: &Ctx, item: &Item) -> tauri::Result<MenuItemKind<Wry>> {
    let (label, accel) =
        label_and_accelerator(item.label, ctx.bindings.menu_binding(binding_key(item)));
    let enabled = ctx.commands.is_enabled(item.command);
    if item.check {
        let mut builder = CheckMenuItemBuilder::with_id(item.command, label)
            .enabled(enabled)
            .checked(ctx.commands.is_checked(item.command));
        if let Some(accel) = accel {
            builder = builder.accelerator(accel);
        }
        return Ok(MenuItemKind::Check(builder.build(ctx.app)?));
    }
    let mut builder = MenuItemBuilder::with_id(item.command, label).enabled(enabled);
    if let Some(accel) = accel {
        builder = builder.accelerator(accel);
    }
    Ok(MenuItemKind::MenuItem(builder.build(ctx.app)?))
}

fn with_items<'a>(
    ctx: &Ctx<'a>,
    mut builder: Builder<'a>,
    items: &[Item],
) -> tauri::Result<Builder<'a>> {
    for item in items {
        if !ctx.commands.knows(item.command) {
            log::warn!("The menu item {} runs no known command", item.command);
            continue;
        }
        builder = builder.item(&build_item(ctx, item)?);
    }
    Ok(builder)
}

/// Groups with a separator before each but the first.
fn with_groups<'a>(
    ctx: &Ctx<'a>,
    mut builder: Builder<'a>,
    groups: &[&[Item]],
) -> tauri::Result<Builder<'a>> {
    for (i, items) in groups.iter().enumerate() {
        if i > 0 {
            builder = builder.separator();
        }
        builder = with_items(ctx, builder, items)?;
    }
    Ok(builder)
}

fn submenu<'a>(ctx: &Ctx<'a>, menu: &MenuDef) -> tauri::Result<Builder<'a>> {
    with_groups(ctx, SubmenuBuilder::new(ctx.app, menu.title), menu.groups)
}

fn app_submenu<'a>(ctx: &Ctx<'a>) -> tauri::Result<Builder<'a>> {
    let [settings, login, leave] = APP_MENU.groups else {
        unreachable!("the app menu has three groups");
    };
    let builder = SubmenuBuilder::new(ctx.app, APP_MENU.title)
        .about(Some(AboutMetadata::default()))
        .separator();
    let builder = with_items(ctx, builder, settings)?.separator();
    let builder = with_items(ctx, builder, login)?
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator();
    with_items(ctx, builder, leave)
}

fn edit_submenu<'a>(ctx: &Ctx<'a>) -> tauri::Result<Builder<'a>> {
    let [undo, rest @ ..] = EDIT_MENU.groups else {
        unreachable!("the Edit menu starts with Undo and Redo");
    };
    let builder = with_items(ctx, SubmenuBuilder::new(ctx.app, EDIT_MENU.title), undo)?
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .separator();
    with_groups(ctx, builder, rest)
}

pub fn build(app: &AppHandle, bindings: &KeybindingsStore) -> tauri::Result<Menu<Wry>> {
    let registry = app.state::<bus::Commands>();
    let commands = registry.available(app);
    let ctx = Ctx {
        app,
        bindings,
        commands: &commands,
    };
    let app_menu = app_submenu(&ctx)?.build()?;
    let file = submenu(&ctx, &FILE_MENU)?.build()?;
    let edit = edit_submenu(&ctx)?.build()?;
    let view = submenu(&ctx, &VIEW_MENU)?
        .separator()
        .fullscreen()
        .build()?;
    let go = submenu(&ctx, &GO_MENU)?.build()?;
    let day = submenu(&ctx, &DAY_MENU)?.build()?;
    let window = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .separator()
        .bring_all_to_front()
        .build()?;
    let help = submenu(&ctx, &HELP_MENU)?.build()?;
    Menu::with_items(
        app,
        &[&app_menu, &file, &edit, &view, &go, &day, &window, &help],
    )
}

#[cfg(test)]
mod tests {
    use super::super::items::{all_items, menu_item};
    use super::*;

    #[test]
    fn every_menu_item_has_a_label() {
        for (menu, item) in all_items() {
            assert!(
                !item.label.is_empty(),
                "{} in {menu} has no label",
                item.command
            );
        }
    }

    #[test]
    fn the_timer_items_show_their_default_chords_as_accelerators() {
        for (command, label, accelerator_text) in [
            ("timer:start", "Start Timer…", "CmdOrCtrl+T"),
            ("timer:stop", "Stop Timer", "CmdOrCtrl+Shift+."),
        ] {
            let item = menu_item(command).unwrap();
            assert_eq!(
                label_and_accelerator(item.label, Some(item.default_binding.into())),
                (label.to_string(), Some(accelerator_text.to_string())),
                "{command}"
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
        for (command, expected) in [
            ("page:nav.prev", "Previous (P)"),
            ("page:nav.next", "Next (N)"),
            ("page:recording.create", "Record Selection (↵)"),
            ("page:recording.delete", "Delete Recording (⌫)"),
            (
                "page:recording.moveToPrevDay",
                "Move Recording to Previous Day (⌥←)",
            ),
            (
                "page:recording.copyToNextDay",
                "Copy Recording to Next Day (⌥⇧→)",
            ),
        ] {
            let item = menu_item(command).unwrap();
            let binding = crate::menu::default_binding(binding_key(item)).map(str::to_string);
            assert_eq!(
                label_and_accelerator(item.label, binding),
                (expected.to_string(), None),
                "{command}"
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
        let item = menu_item("page:sidebar.toggle").unwrap();
        assert_eq!(
            label_and_accelerator(item.label, Some(item.default_binding.into())),
            (
                "Toggle Sidebar".to_string(),
                Some("CmdOrCtrl+Ctrl+S".to_string())
            )
        );
    }
}
