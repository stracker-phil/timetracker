//! Each window's page reports which of its commands can run and which are checked; the menu's `page:`
//! items show the focused window's report (else the main window's), the other items what the bus says.

use std::collections::HashMap;

use serde::Deserialize;
use std::sync::Mutex;
use tauri::menu::MenuItemKind;
use tauri::{AppHandle, Manager, State, WebviewWindow, Wry};
use ts_rs::TS;

use super::items::is_menu_command;
use crate::bus::{self, CommandStatus};
use crate::lock::MutexExt;
use crate::window::MAIN;

/// What a page reports: its page commands (ids without `page:`) that can run now and the checked ones.
#[derive(Deserialize, Debug, Clone, Default, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct MenuState {
    pub enabled: Vec<String>,
    pub checked: Vec<String>,
}

#[derive(Default)]
struct Reports {
    /// The last report of each window, by label.
    by_window: HashMap<String, MenuState>,
    /// The label of the window in front.
    focused: Option<String>,
}

impl Reports {
    /// The focused window when it reports, else the main window.
    fn target(&self) -> &str {
        match &self.focused {
            Some(label) if self.by_window.contains_key(label) => label,
            _ => MAIN,
        }
    }
}

/// The pages' reports and which window is in front.
#[derive(Default)]
pub struct MenuSync(Mutex<Reports>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemState {
    pub enabled: bool,
    pub checked: bool,
}

/// How a page command shows under a report; before the first report every one is enabled.
pub fn item_state(state: Option<&MenuState>, id: &str) -> ItemState {
    let Some(state) = state else {
        return ItemState {
            enabled: true,
            checked: false,
        };
    };
    let listed = |ids: &[String]| ids.iter().any(|listed| listed == id);
    ItemState {
        enabled: listed(&state.enabled),
        checked: listed(&state.checked),
    }
}

/// How the page command `id` shows: by the report of the window `page_target` names.
pub fn page_item(app: &AppHandle, id: &str) -> ItemState {
    let Some(sync) = app.try_state::<MenuSync>() else {
        return item_state(None, id);
    };
    let reports = sync.0.lock_unpoisoned();
    item_state(reports.by_window.get(reports.target()), id)
}

/// The window that runs the menus' page commands: the focused one when its page reports, else main.
pub fn page_target(app: &AppHandle) -> String {
    app.try_state::<MenuSync>()
        .map_or(MAIN.to_string(), |sync| {
            sync.0.lock_unpoisoned().target().to_string()
        })
}

fn apply_to(items: Vec<MenuItemKind<Wry>>, commands: &dyn CommandStatus) -> tauri::Result<()> {
    for item in items {
        let id: &str = item.id().as_ref();
        match &item {
            MenuItemKind::Submenu(sub) => apply_to(sub.items()?, commands)?,
            MenuItemKind::MenuItem(i) if is_menu_command(id) => {
                i.set_enabled(commands.is_enabled(id))?;
            }
            MenuItemKind::Check(i) if is_menu_command(id) => {
                i.set_enabled(commands.is_enabled(id))?;
                i.set_checked(commands.is_checked(id))?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Shows on the current menu what can run now and what is checked.
pub fn apply(app: &AppHandle) {
    let Some(menu) = app.menu() else {
        return;
    };
    let registry = app.state::<bus::Commands>();
    let commands = registry.available(app);
    if let Err(e) = menu.items().and_then(|items| apply_to(items, &commands)) {
        log::warn!("Couldn't update the menu: {e}");
    }
}

/// A window came to the front or went to the back: the menu shows the report of the one in front.
pub fn focus_changed(app: &AppHandle, label: &str, focused: bool) {
    let Some(sync) = app.try_state::<MenuSync>() else {
        return;
    };
    {
        let mut reports = sync.0.lock_unpoisoned();
        if focused {
            reports.focused = Some(label.to_string());
        } else if reports.focused.as_deref() == Some(label) {
            reports.focused = None;
        }
    }
    apply(app);
}

pub fn window_destroyed(app: &AppHandle, label: &str) {
    if let Some(sync) = app.try_state::<MenuSync>() {
        sync.0.lock_unpoisoned().by_window.remove(label);
    }
}

/// A page reports which of its commands can run and which are checked, whenever that changes.
#[tauri::command]
pub fn menu_sync(
    app: AppHandle,
    window: WebviewWindow,
    sync: State<'_, MenuSync>,
    state: MenuState,
) {
    sync.0
        .lock_unpoisoned()
        .by_window
        .insert(window.label().to_string(), state);
    apply(&app);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(enabled: &[&str], checked: &[&str]) -> MenuState {
        MenuState {
            enabled: enabled.iter().map(|s| s.to_string()).collect(),
            checked: checked.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn shown(enabled: bool, checked: bool) -> ItemState {
        ItemState { enabled, checked }
    }

    #[test]
    fn before_the_first_report_every_item_is_enabled_and_unchecked() {
        for id in ["view.day", "edit.undo", "unknown.command"] {
            assert_eq!(item_state(None, id), shown(true, false), "{id}");
        }
    }

    #[test]
    fn a_report_replaces_the_state_before_it() {
        let state = report(&["view.day"], &["view.day"]);
        assert_eq!(item_state(None, "view.week"), shown(true, false));
        assert_eq!(item_state(Some(&state), "view.week"), shown(false, false));
        assert_eq!(item_state(Some(&state), "view.day"), shown(true, true));
    }

    #[test]
    fn an_empty_report_disables_every_item() {
        let state = report(&[], &[]);
        for id in ["view.day", "edit.undo", "welcome.show"] {
            assert_eq!(item_state(Some(&state), id), shown(false, false), "{id}");
        }
    }

    #[test]
    fn items_missing_from_the_enabled_list_are_disabled() {
        let state = report(&["view.day"], &[]);
        assert_eq!(item_state(Some(&state), "view.day"), shown(true, false));
        assert_eq!(item_state(Some(&state), "view.week"), shown(false, false));
    }

    #[test]
    fn checked_follows_the_checked_list() {
        let state = report(&["view.day", "view.week"], &["view.week", "view.rules"]);
        assert_eq!(item_state(Some(&state), "view.day"), shown(true, false));
        assert_eq!(item_state(Some(&state), "view.week"), shown(true, true));
        assert_eq!(item_state(Some(&state), "view.rules"), shown(false, true));
    }

    #[test]
    fn menu_state_reads_camel_case_json() {
        let state: MenuState = serde_json::from_value(serde_json::json!({
            "enabled": ["view.day", "edit.undo"],
            "checked": ["view.day"],
        }))
        .unwrap();
        assert_eq!(state, report(&["view.day", "edit.undo"], &["view.day"]));
    }
}
