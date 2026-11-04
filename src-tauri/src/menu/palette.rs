//! The menus' items as the command palette lists them, and every command `timetracker commands` lists.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use ts_rs::TS;

use super::items::{PALETTE_COMMAND, all_items, binding_key};
use crate::bus::{self, CommandStatus, PAGE_PREFIX};
use crate::state::AppState;
use crate::store::keybindings::{KeybindingsStore, key_label};

#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct MenuCommand {
    /// The menu's title, the palette's section.
    pub menu: String,
    pub label: String,
    pub command: String,
    /// The binding the item shows, as `⌘⇧C`; `None` when unbound.
    pub keys: Option<String>,
    /// A choice that is on, such as the shown view.
    pub checked: bool,
}

/// Every item of the menus in menu order, Command Palette… itself left out; app items only while they
/// can run. `page:` items all come: the palette is open when it asks, which holds the page's commands
/// back, so the page drops those it couldn't run when the palette opened.
pub fn listed(commands: &dyn CommandStatus, bindings: &KeybindingsStore) -> Vec<MenuCommand> {
    all_items()
        .filter(|(_, item)| item.command != PALETTE_COMMAND && commands.knows(item.command))
        .filter(|(_, item)| {
            item.command.starts_with(PAGE_PREFIX) || commands.is_enabled(item.command)
        })
        .map(|(menu, item)| MenuCommand {
            menu: menu.to_string(),
            label: item.label.to_string(),
            command: item.command.to_string(),
            keys: bindings
                .menu_binding(binding_key(item))
                .and_then(|binding| key_label(&binding)),
            checked: item.check && commands.is_checked(item.command),
        })
        .collect()
}

#[tauri::command]
pub fn menu_commands(app: AppHandle) -> Vec<MenuCommand> {
    let registry = app.state::<bus::Commands>();
    listed(
        &registry.available(&app),
        &app.state::<AppState>().keybindings,
    )
}

/// One line of `timetracker commands`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ListedCommand {
    /// A family shows its argument: `tracker:pause:<15|60|tomorrow|resumed>`.
    pub id: String,
    pub title: String,
    /// The menu that holds it, for the page's commands.
    pub menu: Option<String>,
    pub enabled: bool,
}

/// The visible app commands in registration order, then the menus' page items in menu order.
pub fn command_list(bus: &bus::CommandBus<AppHandle>, app: &AppHandle) -> Vec<ListedCommand> {
    let commands = bus
        .list()
        .filter(|listed| !listed.info().hidden)
        .map(|listed| {
            let (id, enabled) = match &listed {
                bus::Listed::Command { id, .. } => (id.to_string(), bus.is_enabled(app, id)),
                bus::Listed::Family(family) => (format!("{}{}", family.prefix, family.arg), true),
            };
            ListedCommand {
                id,
                title: listed.info().title.to_string(),
                menu: None,
                enabled,
            }
        });
    let pages = all_items()
        .filter(|(_, item)| item.command.starts_with(PAGE_PREFIX))
        .map(|(menu, item)| ListedCommand {
            id: item.command.to_string(),
            title: item.label.to_string(),
            menu: Some(menu.to_string()),
            enabled: bus.is_enabled(app, item.command),
        });
    commands.chain(pages).collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::test_support::temp_config_dir;

    /// Knows and enables everything except what a test takes away.
    #[derive(Default)]
    struct Fake {
        unknown: HashSet<&'static str>,
        disabled: HashSet<&'static str>,
        checked: HashSet<&'static str>,
    }

    impl CommandStatus for Fake {
        fn knows(&self, id: &str) -> bool {
            !self.unknown.contains(id)
        }

        fn is_enabled(&self, id: &str) -> bool {
            !self.disabled.contains(id)
        }

        fn is_checked(&self, id: &str) -> bool {
            self.checked.contains(id)
        }
    }

    fn listed_with(commands: &Fake) -> Vec<MenuCommand> {
        let dir = temp_config_dir();
        listed(commands, &KeybindingsStore::open(dir.path()))
    }

    fn entry<'a>(list: &'a [MenuCommand], command: &str) -> &'a MenuCommand {
        list.iter()
            .find(|entry| entry.command == command)
            .unwrap_or_else(|| panic!("{command} is not listed"))
    }

    fn commands_of(list: &[MenuCommand]) -> Vec<&str> {
        list.iter().map(|entry| entry.command.as_str()).collect()
    }

    #[test]
    fn items_come_in_menu_order_with_their_menu_and_label() {
        let list = listed_with(&Fake::default());

        let first: Vec<(&str, &str, &str)> = list
            .iter()
            .take(3)
            .map(|e| (e.menu.as_str(), e.label.as_str(), e.command.as_str()))
            .collect();
        assert_eq!(
            first,
            [
                (crate::APP_NAME, "Settings…", "app:settings"),
                (crate::APP_NAME, "Launch at Login", "app:launchAtLogin"),
                (crate::APP_NAME, "Show Welcome Again", "page:welcome.show"),
            ]
        );
        let expected: Vec<&str> = all_items()
            .map(|(_, item)| item.command)
            .filter(|command| *command != PALETTE_COMMAND)
            .collect();
        assert_eq!(commands_of(&list), expected);
    }

    #[test]
    fn the_command_palette_item_is_left_out() {
        let list = listed_with(&Fake::default());

        assert!(!commands_of(&list).contains(&PALETTE_COMMAND));
    }

    #[test]
    fn unknown_commands_are_left_out() {
        let fake = Fake {
            unknown: HashSet::from(["timer:start", "page:view.day"]),
            ..Fake::default()
        };

        let list = listed_with(&fake);

        let commands = commands_of(&list);
        assert!(!commands.contains(&"timer:start"));
        assert!(!commands.contains(&"page:view.day"));
        assert!(commands.contains(&"timer:stop"));
    }

    #[test]
    fn disabled_app_items_are_dropped_but_page_items_stay() {
        let fake = Fake {
            disabled: HashSet::from(["timer:stop", "tracker:resume", "page:edit.undo"]),
            ..Fake::default()
        };

        let list = listed_with(&fake);

        let commands = commands_of(&list);
        assert!(!commands.contains(&"timer:stop"));
        assert!(!commands.contains(&"tracker:resume"));
        assert!(commands.contains(&"page:edit.undo"));
        assert!(commands.contains(&"timer:start"));
    }

    #[test]
    fn keys_are_labelled_for_the_mac() {
        let list = listed_with(&Fake::default());

        for (command, keys) in [
            ("app:settings", Some("⌘,")),
            ("timer:stop", Some("⌘⇧.")),
            ("page:nav.prev", Some("P")),
            ("page:view.week", Some("⌘2")),
            ("page:recording.create", Some("↵")),
            ("page:recording.edit", None),
            ("app:quit", None),
        ] {
            assert_eq!(entry(&list, command).keys.as_deref(), keys, "{command}");
        }
    }

    #[tokio::test]
    async fn keys_follow_the_users_overrides() {
        let dir = temp_config_dir();
        let bindings = KeybindingsStore::open(dir.path());
        bindings
            .update(
                [
                    ("app:settings".to_string(), Some("Mod+Shift+P".to_string())),
                    ("timer:stop".to_string(), None),
                ]
                .into(),
            )
            .await
            .unwrap();

        let list = listed(&Fake::default(), &bindings);

        assert_eq!(entry(&list, "app:settings").keys.as_deref(), Some("⌘⇧P"));
        assert_eq!(entry(&list, "timer:stop").keys, None);
    }

    #[test]
    fn only_check_items_can_be_checked() {
        let fake = Fake {
            checked: HashSet::from(["page:view.week", "app:launchAtLogin", "app:quit"]),
            ..Fake::default()
        };

        let list = listed_with(&fake);

        let checked: Vec<&str> = list
            .iter()
            .filter(|entry| entry.checked)
            .map(|entry| entry.command.as_str())
            .collect();
        assert_eq!(checked, ["app:launchAtLogin", "page:view.week"]);
    }

    #[test]
    fn a_menu_command_serializes_with_camel_case_fields() {
        let list = listed_with(&Fake::default());

        assert_eq!(
            serde_json::to_value(entry(&list, "app:settings")).unwrap(),
            serde_json::json!({
                "menu": crate::APP_NAME,
                "label": "Settings…",
                "command": "app:settings",
                "keys": "⌘,",
                "checked": false,
            })
        );
    }
}
