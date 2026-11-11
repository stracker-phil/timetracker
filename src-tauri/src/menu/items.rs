//! The menu bar's own items, grouped as the menus show them (ADR 0039). Each item's id is the bus command
//! it runs: an app command (`app:settings`, `timer:start`) or `page:<id>`, one of the page's commands, whose
//! TypeScript command in `src/commands` has `menu: true` and the same default binding.

/// A menu item that runs a command.
pub struct Item {
    pub label: &'static str,
    pub command: &'static str,
    /// The default of the binding the item shows (`binding_key`); empty means none.
    pub default_binding: &'static str,
    /// The command whose binding the item shows, for an item that does one half of what that key does.
    pub binding_of: Option<&'static str>,
    /// A checkbox item; the bus says whether it is checked.
    pub check: bool,
}

const fn item(label: &'static str, command: &'static str, default_binding: &'static str) -> Item {
    Item {
        label,
        command,
        default_binding,
        binding_of: None,
        check: false,
    }
}

const fn check(label: &'static str, command: &'static str, default_binding: &'static str) -> Item {
    Item {
        check: true,
        ..item(label, command, default_binding)
    }
}

/// A menu's title and its own items, in the groups it splits with separators; `build.rs` places macOS's
/// standard items around them.
pub struct MenuDef {
    pub title: &'static str,
    pub groups: &'static [&'static [Item]],
}

/// Settings…, then Launch at Login and the welcome; Hide to Menu Bar and Quit come after macOS's Hide items.
pub const APP_MENU: MenuDef = MenuDef {
    title: crate::APP_NAME,
    groups: &[
        &[item("Settings…", "app:settings", "Mod+,")],
        &[
            check("Launch at Login", "app:launchAtLogin", ""),
            item("Show Welcome Again", "page:welcome.show", ""),
        ],
        // ⌘Q only hides: the app keeps recording until Quit is chosen.
        &[
            item("Hide to Menu Bar", "app:background", "Mod+Q"),
            item("Quit TimeTracker", "app:quit", ""),
        ],
    ],
};

pub const FILE_MENU: MenuDef = MenuDef {
    title: "File",
    groups: &[
        &[item("New Recording…", "page:recording.new", "")],
        &[
            item("Start Timer…", "timer:start", "Mod+T"),
            item("Stop Timer", "timer:stop", "Mod+Shift+."),
        ],
        &[
            item("Pause Tracking", "tracker:pause:resumed", ""),
            item("Resume Tracking", "tracker:resume", ""),
        ],
        &[
            item("New Project…", "page:projects.new", "Mod+N"),
            item("New Client…", "page:clients.new", ""),
            item("New Tag…", "page:tags.new", "Mod+N"),
            item("New Rule…", "page:rules.new", "Mod+N"),
        ],
        &[
            item("Save Report…", "page:reports.save", ""),
            item("Rename Saved Report…", "page:reports.rename", ""),
            item("Delete Saved Report…", "page:reports.delete", ""),
        ],
        &[item("Export Backup…", "page:data.exportBackup", "")],
        &[item("Close Window", "window:close", "Mod+W")],
    ],
};

/// Undo and Redo first, macOS's Cut, Copy, Paste and Select All after them, then the rest.
pub const EDIT_MENU: MenuDef = MenuDef {
    title: "Edit",
    groups: &[
        &[
            item("Undo", "page:edit.undo", "Mod+Z"),
            item("Redo", "page:edit.redo", "Mod+Shift+Z"),
        ],
        &[item(
            "Delete Recording",
            "page:recording.delete",
            "Backspace",
        )],
        &[
            item(
                "Copy Report as Text",
                "page:reports.copyText",
                "Mod+Shift+C",
            ),
            item("Copy Report Notes", "page:reports.copyNotes", "Mod+Alt+C"),
        ],
    ],
};

/// Ends with macOS's Enter Full Screen.
pub const VIEW_MENU: MenuDef = MenuDef {
    title: "View",
    groups: &[
        &[
            check("Day", "page:view.day", "Mod+1"),
            check("Week", "page:view.week", "Mod+2"),
            check("Month", "page:view.month", "Mod+3"),
            check("Reports", "page:view.reports", "Mod+4"),
            check("Projects", "page:view.projects", "Mod+5"),
            check("Tags", "page:view.tags", "Mod+6"),
            check("Rules", "page:view.rules", "Mod+7"),
        ],
        &[item("Toggle Sidebar", "page:sidebar.toggle", "Mod+Ctrl+S")],
        &[
            item("Zoom In", "page:zoom.in", "Mod+="),
            item("Zoom Out", "page:zoom.out", "Mod+-"),
            item("Reset Zoom", "page:zoom.reset", "Mod+0"),
        ],
        &[item("View Options…", "page:day.settings", "")],
    ],
};

pub const GO_MENU: MenuDef = MenuDef {
    title: "Go",
    groups: &[
        &[
            item("Previous", "page:nav.prev", "P"),
            item("Next", "page:nav.next", "N"),
            item("Today", "page:nav.today", "O"),
            item("Go to Date…", "page:nav.goToDate", ""),
            item(
                "Find Recording or Suggestion…",
                "page:palette.reveal",
                "Mod+Shift+F",
            ),
        ],
        &[
            item("Next Activity", "page:day.nextBlock", "J"),
            item("Previous Activity", "page:day.prevBlock", "K"),
        ],
        &[
            item("Next Recording", "page:recording.next", "]"),
            item("Previous Recording", "page:recording.prev", "["),
        ],
    ],
};

pub const DAY_MENU: MenuDef = MenuDef {
    title: "Day",
    groups: &[
        &[
            Item {
                binding_of: Some("recording.open"),
                ..item("Record Selection", "page:recording.create", "Enter")
            },
            item("Edit Recording…", "page:recording.edit", ""),
        ],
        &[
            item(
                "Move Recording to Previous Day",
                "page:recording.moveToPrevDay",
                "Alt+ArrowLeft",
            ),
            item(
                "Move Recording to Next Day",
                "page:recording.moveToNextDay",
                "Alt+ArrowRight",
            ),
            item(
                "Copy Recording to Previous Day",
                "page:recording.copyToPrevDay",
                "Alt+Shift+ArrowLeft",
            ),
            item(
                "Copy Recording to Next Day",
                "page:recording.copyToNextDay",
                "Alt+Shift+ArrowRight",
            ),
        ],
        &[
            item("Approve Suggestion", "page:suggestion.approve", "Enter"),
            item("Edit Suggestion…", "page:suggestion.edit", "E"),
            item("Dismiss Suggestion", "page:suggestion.dismiss", "Backspace"),
            item(
                "Approve All Suggestions of Rule",
                "page:suggestion.approveRule",
                "",
            ),
        ],
        &[
            item("Approve All Suggestions", "page:suggestion.approveAll", ""),
            item("Dismiss All Suggestions", "page:suggestion.dismissAll", ""),
        ],
        &[
            item(
                "Delete Activity for This Day…",
                "page:activity.deleteDay",
                "",
            ),
            item(
                "Delete Activity in a Range…",
                "page:activity.deleteRange",
                "",
            ),
        ],
    ],
};

pub const HELP_MENU: MenuDef = MenuDef {
    title: "Help",
    groups: &[
        &[
            item("Keyboard Shortcuts", "page:shortcuts.show", "Shift+?"),
            item("Command Palette…", "page:palette.toggle", "Mod+K"),
        ],
        &[
            item("Open Log Folder", "app:logs", ""),
            item("Show Data Folder", "app:dataFolder", ""),
            item("README", "app:readme", ""),
        ],
    ],
};

/// The palette's own item, which the palette leaves out.
pub const PALETTE_COMMAND: &str = "page:palette.toggle";

/// Every menu with items of its own, in menu bar order (Window has only macOS's items).
pub const MENUS: &[MenuDef] = &[
    APP_MENU, FILE_MENU, EDIT_MENU, VIEW_MENU, GO_MENU, DAY_MENU, HELP_MENU,
];

/// Items that share a default accelerator. Their commands' scopes, or the focus they need, exclude each
/// other, so at most one of them is enabled and macOS gives the key to that one.
#[cfg(test)]
pub const SHARED_ACCELERATORS: &[&[&str]] = &[
    &["page:projects.new", "page:tags.new", "page:rules.new"],
    &["page:recording.create", "page:suggestion.approve"],
    &["page:recording.delete", "page:suggestion.dismiss"],
];

/// Every item, in menu order with its menu's title.
pub fn all_items() -> impl Iterator<Item = (&'static str, &'static Item)> {
    MENUS.iter().flat_map(|menu| {
        menu.groups
            .iter()
            .flat_map(|group| group.iter())
            .map(move |item| (menu.title, item))
    })
}

pub fn menu_item(command: &str) -> Option<&'static Item> {
    all_items()
        .map(|(_, item)| item)
        .find(|item| item.command == command)
}

pub fn is_menu_command(id: &str) -> bool {
    menu_item(id).is_some()
}

/// The key binding the item shows: its own, else the one it borrows. A page item's is its page command's
/// id (`recording.create`), an app item's its bus id (`app:settings`), as `keybindings.json` stores them.
pub fn binding_key(item: &Item) -> &'static str {
    item.binding_of.unwrap_or_else(|| {
        item.command
            .strip_prefix(crate::bus::PAGE_PREFIX)
            .unwrap_or(item.command)
    })
}

/// The default binding of a command a menu item shows, directly or through `binding_of`.
pub fn default_binding(command: &str) -> Option<&'static str> {
    all_items()
        .map(|(_, item)| item)
        .find(|item| binding_key(item) == command)
        .map(|item| item.default_binding)
        .filter(|b| !b.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::Commands;
    use crate::store::keybindings::{accelerator, is_chord};

    fn commands() -> Vec<&'static str> {
        all_items().map(|(_, item)| item.command).collect()
    }

    #[test]
    fn menu_item_commands_are_not_repeated() {
        let mut ids = commands();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), before);
    }

    #[test]
    fn only_listed_commands_are_menu_commands() {
        for id in commands() {
            assert!(is_menu_command(id), "{id}");
        }
        for id in ["", "nope:unknown", "view.day", "app:settings:capture"] {
            assert!(!is_menu_command(id), "{id}");
        }
    }

    #[test]
    fn menu_item_finds_an_item_by_its_command() {
        let item = menu_item("tracker:resume").unwrap();
        assert_eq!(item.label, "Resume Tracking");
        assert!(menu_item("timer:toggle").is_none());
    }

    #[test]
    fn every_app_item_runs_a_command_the_bus_knows() {
        let bus = Commands::registered();
        for id in commands()
            .into_iter()
            .filter(|id| !id.starts_with(crate::bus::PAGE_PREFIX))
        {
            assert!(bus.knows(id), "{id}");
        }
    }

    #[test]
    fn default_accelerators_are_shared_only_where_listed() {
        let items: Vec<&Item> = all_items().map(|(_, item)| item).collect();
        for (i, item) in items.iter().enumerate() {
            if item.default_binding.is_empty() {
                continue;
            }
            for other in &items[i + 1..] {
                if other.default_binding != item.default_binding {
                    continue;
                }
                let listed = SHARED_ACCELERATORS
                    .iter()
                    .any(|group| group.contains(&item.command) && group.contains(&other.command));
                assert!(
                    listed,
                    "{} and {} share {}",
                    item.command, other.command, item.default_binding
                );
            }
        }
    }

    #[test]
    fn shared_accelerator_groups_name_items_with_the_same_binding() {
        for group in SHARED_ACCELERATORS {
            let bindings: Vec<&str> = group
                .iter()
                .map(|id| menu_item(id).expect(id).default_binding)
                .collect();
            assert!(bindings.iter().all(|b| *b == bindings[0]), "{group:?}");
        }
    }

    #[test]
    fn the_new_items_of_the_list_views_share_mod_n() {
        assert!(
            SHARED_ACCELERATORS
                .contains(&&["page:projects.new", "page:tags.new", "page:rules.new"][..])
        );
        for command in ["page:projects.new", "page:tags.new", "page:rules.new"] {
            assert_eq!(
                menu_item(command).unwrap().default_binding,
                "Mod+N",
                "{command}"
            );
        }
        assert_eq!(menu_item("page:tags.new").unwrap().label, "New Tag…");
    }

    #[test]
    fn the_suggestion_keys_share_their_accelerators_with_the_recording_keys() {
        assert!(
            SHARED_ACCELERATORS
                .contains(&&["page:recording.create", "page:suggestion.approve"][..])
        );
        assert!(
            SHARED_ACCELERATORS
                .contains(&&["page:recording.delete", "page:suggestion.dismiss"][..])
        );
        assert_eq!(SHARED_ACCELERATORS.len(), 3);
    }

    #[test]
    fn the_go_menu_steps_through_activities() {
        for (command, label, binding) in [
            ("page:day.nextBlock", "Next Activity", "J"),
            ("page:day.prevBlock", "Previous Activity", "K"),
        ] {
            let item = menu_item(command).unwrap();
            assert_eq!(
                (item.label, item.default_binding),
                (label, binding),
                "{command}"
            );
        }
    }

    #[test]
    fn finding_a_recording_or_suggestion_is_a_go_item_bound_to_mod_shift_f() {
        let item = menu_item("page:palette.reveal").unwrap();

        assert_eq!(
            (item.label, item.default_binding),
            ("Find Recording or Suggestion…", "Mod+Shift+F")
        );
        assert!(GO_MENU.groups[0].iter().any(|i| i.command == item.command));
        assert_eq!(default_binding("palette.reveal"), Some("Mod+Shift+F"));
    }

    #[test]
    fn the_suggestion_items_are_labelled_and_bound_for_review() {
        let items: Vec<(&str, &str, &str)> = [
            "page:suggestion.approve",
            "page:suggestion.edit",
            "page:suggestion.dismiss",
            "page:suggestion.approveRule",
            "page:suggestion.approveAll",
            "page:suggestion.dismissAll",
        ]
        .iter()
        .map(|id| {
            let item = menu_item(id).expect(id);
            (item.command, item.label, item.default_binding)
        })
        .collect();

        assert_eq!(
            items,
            [
                ("page:suggestion.approve", "Approve Suggestion", "Enter"),
                ("page:suggestion.edit", "Edit Suggestion…", "E"),
                ("page:suggestion.dismiss", "Dismiss Suggestion", "Backspace"),
                (
                    "page:suggestion.approveRule",
                    "Approve All Suggestions of Rule",
                    ""
                ),
                ("page:suggestion.approveAll", "Approve All Suggestions", ""),
                ("page:suggestion.dismissAll", "Dismiss All Suggestions", ""),
            ]
        );
    }

    #[test]
    fn default_chords_convert_to_accelerators() {
        for (_, item) in all_items().filter(|(_, item)| is_chord(item.default_binding)) {
            assert!(
                accelerator(item.default_binding).is_some(),
                "{}: {}",
                item.command,
                item.default_binding
            );
        }
    }

    #[test]
    fn the_binding_key_of_a_page_item_is_its_page_command_without_the_prefix() {
        assert_eq!(
            binding_key(menu_item("page:edit.undo").unwrap()),
            "edit.undo"
        );
        assert_eq!(binding_key(menu_item("page:view.day").unwrap()), "view.day");
    }

    #[test]
    fn the_binding_key_of_an_app_item_is_its_bus_id() {
        assert_eq!(
            binding_key(menu_item("app:settings").unwrap()),
            "app:settings"
        );
        assert_eq!(
            binding_key(menu_item("tracker:pause:resumed").unwrap()),
            "tracker:pause:resumed"
        );
    }

    #[test]
    fn an_item_borrowing_a_binding_shows_the_other_commands_key() {
        assert_eq!(
            binding_key(menu_item("page:recording.create").unwrap()),
            "recording.open"
        );
        assert_eq!(
            binding_key(menu_item("page:recording.edit").unwrap()),
            "recording.edit"
        );
    }

    #[test]
    fn default_binding_follows_the_borrowed_command() {
        assert_eq!(default_binding("recording.open"), Some("Enter"));
        assert_eq!(default_binding("recording.create"), None);
        assert_eq!(default_binding("recording.edit"), None);
        assert_eq!(default_binding("app:settings"), Some("Mod+,"));
        assert_eq!(default_binding("unknown.command"), None);
    }

    #[test]
    fn the_view_items_are_bound_to_mod_digits_in_order() {
        let bindings: Vec<(&str, &str)> = [
            "page:view.day",
            "page:view.week",
            "page:view.month",
            "page:view.reports",
            "page:view.projects",
            "page:view.tags",
            "page:view.rules",
        ]
        .iter()
        .map(|id| (*id, menu_item(id).expect(id).default_binding))
        .collect();

        assert_eq!(
            bindings,
            [
                ("page:view.day", "Mod+1"),
                ("page:view.week", "Mod+2"),
                ("page:view.month", "Mod+3"),
                ("page:view.reports", "Mod+4"),
                ("page:view.projects", "Mod+5"),
                ("page:view.tags", "Mod+6"),
                ("page:view.rules", "Mod+7"),
            ]
        );
    }

    #[test]
    fn mod_q_hides_to_the_menu_bar_and_quit_has_no_default_binding() {
        assert_eq!(default_binding("app:background"), Some("Mod+Q"));
        assert_eq!(default_binding("app:quit"), None);
    }

    #[test]
    fn copying_the_report_notes_is_bound_to_mod_alt_c() {
        assert_eq!(default_binding("reports.copyNotes"), Some("Mod+Alt+C"));
    }

    #[test]
    fn exactly_the_view_toggles_and_launch_at_login_are_check_items() {
        let mut checks: Vec<&str> = all_items()
            .filter(|(_, item)| item.check)
            .map(|(_, item)| item.command)
            .collect();
        checks.sort_unstable();
        let mut expected = vec![
            "page:view.day",
            "page:view.week",
            "page:view.month",
            "page:view.reports",
            "page:view.projects",
            "page:view.tags",
            "page:view.rules",
            "app:launchAtLogin",
        ];
        expected.sort_unstable();
        assert_eq!(checks, expected);
    }
}
