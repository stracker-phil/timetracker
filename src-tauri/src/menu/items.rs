//! The menu items that run app commands: one entry per command id, mirrored by `menu: true` on the
//! TypeScript command in `src/commands`.

pub struct MenuItemDef {
    pub id: &'static str,
    pub label: &'static str,
    /// Mirrors the `defaultBinding` in `src/commands` of the command whose binding the item shows;
    /// empty means none.
    pub default_binding: &'static str,
    /// The command whose binding the item shows, for an item that does one half of what that key does.
    pub binding_of: Option<&'static str>,
    /// A checkbox item; the page reports which are checked.
    pub check: bool,
}

const fn item(id: &'static str, label: &'static str, default_binding: &'static str) -> MenuItemDef {
    MenuItemDef {
        id,
        label,
        default_binding,
        binding_of: None,
        check: false,
    }
}

const fn check(
    id: &'static str,
    label: &'static str,
    default_binding: &'static str,
) -> MenuItemDef {
    MenuItemDef {
        check: true,
        ..item(id, label, default_binding)
    }
}

pub const MENU_ITEMS: &[MenuItemDef] = &[
    // TimeTracker
    item("settings.toggle", "Settings…", "Mod+,"),
    check("app.launchAtLogin", "Launch at Login", ""),
    item("welcome.show", "Show Welcome Again", ""),
    item("app.quit", "Quit TimeTracker", "Mod+Q"),
    // File
    item("recording.new", "New Recording…", ""),
    item("timer.start", "Start Timer…", "Mod+T"),
    item("timer.stop", "Stop Timer", "Mod+Shift+."),
    item("projects.new", "New Project…", "Mod+N"),
    item("clients.new", "New Client…", ""),
    item("rules.new", "New Rule…", "Mod+N"),
    item("data.exportBackup", "Export Backup…", ""),
    item("window.hide", "Close Window", "Mod+W"),
    // Edit
    item("edit.undo", "Undo", "Mod+Z"),
    item("edit.redo", "Redo", "Mod+Shift+Z"),
    item("recording.delete", "Delete Recording", "Backspace"),
    item("reports.copyText", "Copy Report as Text", "Mod+Shift+C"),
    item("reports.copyNotes", "Copy Report Notes", "Mod+Alt+C"),
    // View
    check("view.day", "Day", "Mod+1"),
    check("view.week", "Week", "Mod+2"),
    check("view.month", "Month", "Mod+3"),
    check("view.reports", "Reports", "Mod+4"),
    check("view.projects", "Projects", "Mod+5"),
    check("view.rules", "Rules", "Mod+6"),
    check("day.toggleList", "Show as List", "Mod+Shift+L"),
    item("sidebar.toggle", "Toggle Sidebar", "Mod+Ctrl+S"),
    item("zoom.in", "Zoom In", "Mod+="),
    item("zoom.out", "Zoom Out", "Mod+-"),
    item("zoom.reset", "Reset Zoom", "Mod+0"),
    item("day.settings", "Timeline Settings…", ""),
    // Go
    item("nav.prev", "Previous", "P"),
    item("nav.next", "Next", "N"),
    item("nav.today", "Today", "O"),
    item("nav.goToDate", "Go to Date…", ""),
    item("day.nextBlock", "Next Block", "J"),
    item("day.prevBlock", "Previous Block", "K"),
    item("recording.next", "Next Recording", "]"),
    item("recording.prev", "Previous Recording", "["),
    // Day
    MenuItemDef {
        binding_of: Some("recording.open"),
        ..item("recording.create", "Record Selection", "Enter")
    },
    item("recording.edit", "Edit Recording…", ""),
    item(
        "recording.moveToPrevDay",
        "Move Recording to Previous Day",
        "Alt+ArrowLeft",
    ),
    item(
        "recording.moveToNextDay",
        "Move Recording to Next Day",
        "Alt+ArrowRight",
    ),
    item(
        "recording.copyToPrevDay",
        "Copy Recording to Previous Day",
        "Alt+Shift+ArrowLeft",
    ),
    item(
        "recording.copyToNextDay",
        "Copy Recording to Next Day",
        "Alt+Shift+ArrowRight",
    ),
    item("suggestion.approve", "Approve Suggestion", "Enter"),
    item("suggestion.edit", "Edit Suggestion…", "E"),
    item("suggestion.dismiss", "Dismiss Suggestion", "Backspace"),
    item(
        "suggestion.approveRule",
        "Approve All Suggestions of Rule",
        "",
    ),
    item("suggestion.approveAll", "Approve All Suggestions", ""),
    item("suggestion.dismissAll", "Dismiss All Suggestions", ""),
    item("activity.deleteDay", "Delete Activity for This Day…", ""),
    item("activity.deleteRange", "Delete Activity in a Range…", ""),
    // Help
    item("shortcuts.show", "Keyboard Shortcuts", "Shift+?"),
    item("palette.toggle", "Command Palette…", "Mod+K"),
    item("logs.open", "Open Log Folder", ""),
    item("data.reveal", "Show Data Folder", ""),
    item("help.readme", "README", ""),
];

/// Items that share a default accelerator. Their commands' scopes, or the focus they need, exclude each
/// other, so at most one of them is enabled and macOS gives the key to that one.
#[cfg(test)]
pub const SHARED_ACCELERATORS: &[&[&str]] = &[
    &["projects.new", "rules.new"],
    &["recording.create", "suggestion.approve"],
    &["recording.delete", "suggestion.dismiss"],
];

pub fn menu_item(id: &str) -> Option<&'static MenuItemDef> {
    MENU_ITEMS.iter().find(|def| def.id == id)
}

pub fn is_menu_command(id: &str) -> bool {
    menu_item(id).is_some()
}

/// The command whose binding the item shows: its own, unless it borrows one.
pub fn binding_owner(id: &str) -> &str {
    menu_item(id).and_then(|def| def.binding_of).unwrap_or(id)
}

/// The default binding of a command a menu item shows, directly or through `binding_of`.
pub fn default_binding(command: &str) -> Option<&'static str> {
    MENU_ITEMS
        .iter()
        .find(|def| binding_owner(def.id) == command)
        .map(|def| def.default_binding)
        .filter(|b| !b.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::keybindings::{accelerator, is_chord};

    const DOCUMENTED_IDS: &[&str] = &[
        "settings.toggle",
        "app.launchAtLogin",
        "welcome.show",
        "app.quit",
        "recording.new",
        "timer.start",
        "timer.stop",
        "projects.new",
        "clients.new",
        "rules.new",
        "data.exportBackup",
        "window.hide",
        "edit.undo",
        "edit.redo",
        "recording.delete",
        "reports.copyText",
        "reports.copyNotes",
        "view.day",
        "view.week",
        "view.month",
        "view.reports",
        "view.projects",
        "view.rules",
        "day.toggleList",
        "sidebar.toggle",
        "zoom.in",
        "zoom.out",
        "zoom.reset",
        "day.settings",
        "nav.prev",
        "nav.next",
        "nav.today",
        "nav.goToDate",
        "day.nextBlock",
        "day.prevBlock",
        "recording.next",
        "recording.prev",
        "recording.create",
        "recording.edit",
        "recording.moveToPrevDay",
        "recording.moveToNextDay",
        "recording.copyToPrevDay",
        "recording.copyToNextDay",
        "suggestion.approve",
        "suggestion.edit",
        "suggestion.dismiss",
        "suggestion.approveRule",
        "suggestion.approveAll",
        "suggestion.dismissAll",
        "activity.deleteDay",
        "activity.deleteRange",
        "shortcuts.show",
        "palette.toggle",
        "logs.open",
        "data.reveal",
        "help.readme",
    ];

    #[test]
    fn every_documented_menu_command_is_a_menu_item() {
        for id in DOCUMENTED_IDS {
            assert!(menu_item(id).is_some(), "{id}");
        }
        assert_eq!(MENU_ITEMS.len(), DOCUMENTED_IDS.len());
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
    fn menu_item_ids_are_not_repeated() {
        let mut ids: Vec<&str> = MENU_ITEMS.iter().map(|def| def.id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), before);
    }

    #[test]
    fn default_accelerators_are_shared_only_where_listed() {
        for (i, def) in MENU_ITEMS.iter().enumerate() {
            if def.default_binding.is_empty() {
                continue;
            }
            for other in &MENU_ITEMS[i + 1..] {
                if other.default_binding != def.default_binding {
                    continue;
                }
                let listed = SHARED_ACCELERATORS
                    .iter()
                    .any(|group| group.contains(&def.id) && group.contains(&other.id));
                assert!(
                    listed,
                    "{} and {} share {}",
                    def.id, other.id, def.default_binding
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
    fn the_suggestion_keys_share_their_accelerators_with_the_recording_keys() {
        let groups: Vec<Vec<&str>> = SHARED_ACCELERATORS.iter().map(|g| g.to_vec()).collect();

        assert!(groups.contains(&vec!["recording.create", "suggestion.approve"]));
        assert!(groups.contains(&vec!["recording.delete", "suggestion.dismiss"]));
        assert_eq!(groups.len(), 3);
    }

    #[test]
    fn the_suggestion_items_are_labelled_and_bound_for_review() {
        let items: Vec<(&str, &str, &str)> = [
            "suggestion.approve",
            "suggestion.edit",
            "suggestion.dismiss",
            "suggestion.approveRule",
            "suggestion.approveAll",
            "suggestion.dismissAll",
        ]
        .iter()
        .map(|id| {
            let def = menu_item(id).expect(id);
            (def.id, def.label, def.default_binding)
        })
        .collect();

        assert_eq!(
            items,
            [
                ("suggestion.approve", "Approve Suggestion", "Enter"),
                ("suggestion.edit", "Edit Suggestion…", "E"),
                ("suggestion.dismiss", "Dismiss Suggestion", "Backspace"),
                (
                    "suggestion.approveRule",
                    "Approve All Suggestions of Rule",
                    ""
                ),
                ("suggestion.approveAll", "Approve All Suggestions", ""),
                ("suggestion.dismissAll", "Dismiss All Suggestions", ""),
            ]
        );
    }

    #[test]
    fn default_chords_convert_to_accelerators() {
        for def in MENU_ITEMS.iter().filter(|d| is_chord(d.default_binding)) {
            assert!(
                accelerator(def.default_binding).is_some(),
                "{}: {}",
                def.id,
                def.default_binding
            );
        }
    }

    #[test]
    fn an_item_borrowing_a_binding_is_owned_by_the_other_command() {
        assert_eq!(binding_owner("recording.create"), "recording.open");
        assert_eq!(binding_owner("recording.edit"), "recording.edit");
        assert_eq!(binding_owner("unknown.command"), "unknown.command");
    }

    #[test]
    fn default_binding_follows_the_borrowed_command() {
        assert_eq!(default_binding("recording.open"), Some("Enter"));
        assert_eq!(default_binding("recording.create"), None);
        assert_eq!(default_binding("recording.edit"), None);
        assert_eq!(default_binding("settings.toggle"), Some("Mod+,"));
        assert_eq!(default_binding("unknown.command"), None);
    }

    #[test]
    fn copying_the_report_notes_is_bound_to_mod_alt_c() {
        assert_eq!(default_binding("reports.copyNotes"), Some("Mod+Alt+C"));
    }

    #[test]
    fn exactly_the_view_toggles_and_launch_at_login_are_check_items() {
        let mut checks: Vec<&str> = MENU_ITEMS
            .iter()
            .filter(|def| def.check)
            .map(|def| def.id)
            .collect();
        checks.sort_unstable();
        let mut expected = vec![
            "view.day",
            "view.week",
            "view.month",
            "view.reports",
            "view.projects",
            "view.rules",
            "day.toggleList",
            "app.launchAtLogin",
        ];
        expected.sort_unstable();
        assert_eq!(checks, expected);
    }
}
