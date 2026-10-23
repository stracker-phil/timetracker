//! The page reports which items can run and which are checked; Rust shows that on the menu.

use serde::Deserialize;
use std::sync::Mutex;
use tauri::menu::MenuItemKind;
use tauri::{AppHandle, Manager, State, Wry};
use ts_rs::TS;

use super::items::is_menu_command;
use crate::lock::MutexExt;

/// What the page reports: the items whose commands can run now and the checked ones.
#[derive(Deserialize, Debug, Clone, Default, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct MenuState {
    pub enabled: Vec<String>,
    pub checked: Vec<String>,
}

/// The page's last `MenuState`; `None` until the page reports one.
#[derive(Default)]
pub struct MenuSync(Mutex<Option<MenuState>>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemState {
    pub enabled: bool,
    pub checked: bool,
}

/// Quitting and hiding work even when the page is broken, so they never wait for the page to enable them.
const ALWAYS_ENABLED: &[&str] = &["app.background", "app.quit"];

/// How an item shows under the page's report; before the first report every item is enabled.
pub fn item_state(state: Option<&MenuState>, id: &str) -> ItemState {
    let Some(state) = state else {
        return ItemState {
            enabled: true,
            checked: false,
        };
    };
    let listed = |ids: &[String]| ids.iter().any(|listed| listed == id);
    ItemState {
        enabled: ALWAYS_ENABLED.contains(&id) || listed(&state.enabled),
        checked: listed(&state.checked),
    }
}

pub(super) fn current_state<M: Manager<Wry>>(app: &M) -> Option<MenuState> {
    app.try_state::<MenuSync>()
        .and_then(|sync| sync.0.lock_unpoisoned().clone())
}

fn apply_to(items: Vec<MenuItemKind<Wry>>, state: Option<&MenuState>) -> tauri::Result<()> {
    for item in items {
        let id: &str = item.id().as_ref();
        match &item {
            MenuItemKind::Submenu(sub) => apply_to(sub.items()?, state)?,
            MenuItemKind::MenuItem(i) if is_menu_command(id) => {
                i.set_enabled(item_state(state, id).enabled)?;
            }
            MenuItemKind::Check(i) if is_menu_command(id) => {
                let shown = item_state(state, id);
                i.set_enabled(shown.enabled)?;
                i.set_checked(shown.checked)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Shows the page's last report on the current menu.
pub(super) fn apply(app: &AppHandle) {
    let Some(menu) = app.menu() else {
        return;
    };
    let state = current_state(app);
    if let Err(e) = menu
        .items()
        .and_then(|items| apply_to(items, state.as_ref()))
    {
        log::warn!("Couldn't update the menu: {e}");
    }
}

/// The page reports which items can run and which are checked, whenever that changes.
#[tauri::command]
pub fn menu_sync(app: AppHandle, sync: State<'_, MenuSync>, state: MenuState) {
    *sync.0.lock_unpoisoned() = Some(state);
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
        for id in ["view.day", "app.quit", "unknown.command"] {
            assert_eq!(item_state(None, id), shown(true, false), "{id}");
        }
    }

    #[test]
    fn items_missing_from_the_enabled_list_are_disabled() {
        let state = report(&["view.day"], &[]);
        assert_eq!(item_state(Some(&state), "view.day"), shown(true, false));
        assert_eq!(item_state(Some(&state), "view.week"), shown(false, false));
    }

    #[test]
    fn quit_stays_enabled_when_the_page_does_not_list_it() {
        let state = report(&[], &[]);
        assert_eq!(item_state(Some(&state), "app.quit"), shown(true, false));
    }

    #[test]
    fn hide_to_menu_bar_stays_enabled_when_the_page_does_not_list_it() {
        let state = report(&[], &[]);
        assert_eq!(
            item_state(Some(&state), "app.background"),
            shown(true, false)
        );
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
            "enabled": ["view.day", "app.quit"],
            "checked": ["view.day"],
        }))
        .unwrap();
        assert_eq!(state, report(&["view.day", "app.quit"], &["view.day"]));
    }
}
