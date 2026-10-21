//! User shortcut overrides (`keybindings.json`): `{ "commandId": "Mod+Shift+X" | null }`.
//! The webview resolves most shortcuts; the native menu reads the same file for its
//! accelerators so the two never disagree.

use super::json_store::JsonStore;
use crate::error::AppResult;
use crate::menu;
use std::collections::BTreeMap;
use std::path::Path;

/// `None` means the user unbound the command.
pub type Keybindings = BTreeMap<String, Option<String>>;

pub struct KeybindingsStore {
    store: JsonStore<Keybindings>,
}

impl KeybindingsStore {
    pub fn open(dir: &Path) -> Self {
        Self {
            store: JsonStore::open_with(dir.join("keybindings.json"), sanitize),
        }
    }

    pub fn get(&self) -> Keybindings {
        self.store.read(Keybindings::clone)
    }

    /// Replaces all overrides; malformed entries are dropped (see `sanitize`).
    pub async fn update(&self, bindings: Keybindings) -> AppResult<Keybindings> {
        let bindings = sanitize(bindings);
        self.store
            .update_async(move |current| {
                *current = bindings.clone();
                bindings
            })
            .await
    }

    /// The binding a menu item should show: the user's override, else the default.
    pub fn menu_binding(&self, id: &str) -> Option<String> {
        if let Some(custom) = self.store.read(|current| current.get(id).cloned()) {
            return custom;
        }
        menu::default_binding(id).map(str::to_string)
    }
}

const MODIFIERS: &[&str] = &["Mod", "Ctrl", "Alt", "Shift"];

/// Drops overrides whose command id or binding the webview could never match. Ids are not
/// checked against the registry: the frontend owns most commands and Rust does not know them.
pub fn sanitize(bindings: Keybindings) -> Keybindings {
    bindings
        .into_iter()
        .filter(|(id, binding)| {
            let valid = is_command_id(id) && binding.as_deref().is_none_or(|b| split(b).is_some());
            if !valid {
                log::warn!("Ignoring key binding {id:?} = {binding:?}");
            }
            valid
        })
        .collect()
}

fn is_command_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
}

/// `Mod+Shift+K` into its modifiers and key. A trailing `++` (or a bare `+`) is the `+` key.
fn split(binding: &str) -> Option<(Vec<&str>, &str)> {
    let (modifiers, key) = if binding == "+" {
        ("", "+")
    } else if let Some(modifiers) = binding.strip_suffix("++") {
        (modifiers, "+")
    } else {
        binding.rsplit_once('+').unwrap_or(("", binding))
    };
    let modifiers: Vec<&str> = match modifiers {
        "" => Vec::new(),
        some => some.split('+').collect(),
    };
    let modifiers_valid = modifiers.iter().all(|m| MODIFIERS.contains(m));
    let single_char = key.chars().count() == 1 && !key.trim().is_empty();
    let named_key = !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric());
    let key_valid = !MODIFIERS.contains(&key) && (single_char || named_key);
    (modifiers_valid && key_valid).then_some((modifiers, key))
}

/// `Mod+Shift+K` → the `CmdOrCtrl+Shift+K` form Tauri accelerators expect; `None` when the
/// binding is malformed or its key has no Tauri accelerator name.
pub fn accelerator(binding: &str) -> Option<String> {
    let (modifiers, key) = split(binding)?;
    let mut parts: Vec<&str> = modifiers
        .iter()
        .map(|m| if *m == "Mod" { "CmdOrCtrl" } else { m })
        .collect();
    // Tauri accelerators name physical keys and have none for `+`: Shift+Equal on US layouts.
    if key == "+" && !parts.contains(&"Shift") {
        parts.push("Shift");
    }
    let key = match key {
        "+" | "=" => "Equal",
        "-" => "Minus",
        single if single.chars().count() == 1 && !is_accelerator_char(single) => return None,
        other => other,
    };
    parts.push(key);
    Some(parts.join("+"))
}

/// Whether `binding` can be a system-wide shortcut: a chord with a key Tauri can name.
pub fn is_global(binding: &str) -> bool {
    is_chord(binding) && accelerator(binding).is_some()
}

/// Holds ⌘ or ⌃, so it never types a character.
pub fn is_chord(binding: &str) -> bool {
    split(binding)
        .is_some_and(|(modifiers, _)| modifiers.iter().any(|m| matches!(*m, "Mod" | "Ctrl")))
}

/// `Shift+Y` → `⇧Y`, as `bindingLabel` in `src/commands/registry.ts` shows it.
pub fn key_label(binding: &str) -> Option<String> {
    let (modifiers, key) = split(binding)?;
    let mut label: String = modifiers
        .iter()
        .map(|m| match *m {
            "Mod" => "⌘",
            "Ctrl" => "⌃",
            "Alt" => "⌥",
            "Shift" => "⇧",
            other => unreachable!("split admits only MODIFIERS, got {other}"),
        })
        .collect();
    label.push_str(match key {
        "Enter" => "↵",
        "Escape" => "⎋",
        "Tab" => "⇥",
        "Backspace" => "⌫",
        "Delete" => "⌦",
        "ArrowUp" => "↑",
        "ArrowDown" => "↓",
        "ArrowLeft" => "←",
        "ArrowRight" => "→",
        "Space" => "␣",
        other => other,
    });
    Some(label)
}

fn is_accelerator_char(key: &str) -> bool {
    key.chars()
        .all(|c| c.is_ascii_alphanumeric() || "`\\[],.';/".contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_config_dir;

    fn overrides(entries: &[(&str, Option<&str>)]) -> Keybindings {
        entries
            .iter()
            .map(|(id, binding)| (id.to_string(), binding.map(String::from)))
            .collect()
    }

    #[test]
    fn menu_binding_without_override_is_the_default() {
        let dir = temp_config_dir();
        let store = KeybindingsStore::open(dir.path());
        assert_eq!(
            store.menu_binding("settings.toggle").as_deref(),
            Some("Mod+,")
        );
    }

    #[tokio::test]
    async fn override_persists_across_reopen() {
        let dir = temp_config_dir();
        KeybindingsStore::open(dir.path())
            .update(overrides(&[("settings.toggle", Some("Mod+Shift+P"))]))
            .await
            .unwrap();
        let reopened = KeybindingsStore::open(dir.path());
        assert_eq!(
            reopened.menu_binding("settings.toggle").as_deref(),
            Some("Mod+Shift+P")
        );
        assert_eq!(
            reopened.menu_binding("window.hide").as_deref(),
            Some("Mod+W")
        );
    }

    #[tokio::test]
    async fn null_override_unbinds_a_default() {
        let dir = temp_config_dir();
        let store = KeybindingsStore::open(dir.path());
        store
            .update(overrides(&[("window.hide", None)]))
            .await
            .unwrap();
        assert_eq!(store.menu_binding("window.hide"), None);
        assert_eq!(
            KeybindingsStore::open(dir.path()).menu_binding("window.hide"),
            None
        );
    }

    #[test]
    fn record_open_shows_enter_by_default() {
        let dir = temp_config_dir();
        let store = KeybindingsStore::open(dir.path());
        assert_eq!(
            store.menu_binding("recording.open").as_deref(),
            Some("Enter")
        );
    }

    #[tokio::test]
    async fn override_of_record_open_wins_over_enter() {
        let dir = temp_config_dir();
        let store = KeybindingsStore::open(dir.path());
        store
            .update(overrides(&[("recording.open", Some("Shift+Enter"))]))
            .await
            .unwrap();
        assert_eq!(
            store.menu_binding("recording.open").as_deref(),
            Some("Shift+Enter")
        );
    }

    #[test]
    fn unknown_id_has_no_menu_binding() {
        let dir = temp_config_dir();
        let store = KeybindingsStore::open(dir.path());
        assert_eq!(store.menu_binding("unknown"), None);
    }

    #[test]
    fn command_without_default_has_no_menu_binding() {
        let dir = temp_config_dir();
        let store = KeybindingsStore::open(dir.path());
        assert_eq!(store.menu_binding("logs.open"), None);
    }

    #[test]
    fn only_bindings_with_command_or_control_are_chords() {
        for (binding, expected) in [
            ("Mod+S", true),
            ("Ctrl+Tab", true),
            ("Mod+Ctrl+S", true),
            ("Y", false),
            ("Shift+Y", false),
            ("Alt+X", false),
            ("Shift+?", false),
            ("Mod+", false),
        ] {
            assert_eq!(is_chord(binding), expected, "{binding}");
        }
    }

    #[test]
    fn only_chords_with_a_tauri_key_can_be_global_shortcuts() {
        for (binding, expected) in [
            ("Mod+Shift+T", true),
            ("Ctrl+Alt+K", true),
            ("Mod+,", true),
            ("Shift+T", false),
            ("Alt+K", false),
            ("T", false),
            ("Mod+Ä", false),
            ("Mod+", false),
            ("Hyper+K", false),
            ("", false),
        ] {
            assert_eq!(is_global(binding), expected, "{binding}");
        }
    }

    #[test]
    fn key_labels_use_mac_symbols() {
        for (binding, expected) in [
            ("Y", Some("Y")),
            ("Shift+Y", Some("⇧Y")),
            ("Shift+?", Some("⇧?")),
            ("Mod+Shift+Enter", Some("⌘⇧↵")),
            ("Mod+Alt+ArrowLeft", Some("⌘⌥←")),
            ("Ctrl+Tab", Some("⌃⇥")),
            ("Mod+", None),
        ] {
            assert_eq!(key_label(binding).as_deref(), expected, "{binding}");
        }
    }

    #[test]
    fn bindings_convert_to_tauri_accelerators() {
        for (binding, expected) in [
            ("Mod+Shift+K", Some("CmdOrCtrl+Shift+K")),
            ("Mod+[", Some("CmdOrCtrl+[")),
            ("Mod+=", Some("CmdOrCtrl+Equal")),
            ("Mod+,", Some("CmdOrCtrl+,")),
            ("Ctrl+Tab", Some("Ctrl+Tab")),
        ] {
            assert_eq!(accelerator(binding).as_deref(), expected, "{binding}");
        }
    }

    #[test]
    fn minus_and_plus_keys_convert_to_physical_key_names() {
        for (binding, expected) in [
            ("Mod+-", Some("CmdOrCtrl+Minus")),
            ("Mod++", Some("CmdOrCtrl+Shift+Equal")),
            ("Mod+Shift++", Some("CmdOrCtrl+Shift+Equal")),
            ("Mod+Ä", None),
        ] {
            assert_eq!(accelerator(binding).as_deref(), expected, "{binding}");
        }
    }

    #[test]
    fn sanitize_drops_bad_syntax_and_bad_ids() {
        let cleaned = sanitize(overrides(&[
            ("a.bad", Some("Mod+")),
            ("b.bad", Some("Hyper+K")),
            ("c.bad", Some("Mod+K+J")),
            ("bad id!", Some("Mod+K")),
            ("", Some("Mod+K")),
            ("settings.toggle", Some("Mod+Shift+P")),
            ("window.hide", None),
            ("zoom.in", Some("Mod++")),
        ]));
        assert_eq!(
            cleaned,
            overrides(&[
                ("settings.toggle", Some("Mod+Shift+P")),
                ("window.hide", None),
                ("zoom.in", Some("Mod++")),
            ])
        );
    }

    #[tokio::test]
    async fn update_returns_the_sanitized_map_and_persists_it() {
        let dir = temp_config_dir();
        let store = KeybindingsStore::open(dir.path());
        let saved = store
            .update(overrides(&[
                ("settings.toggle", Some("Mod+Shift+P")),
                ("window.hide", Some("Hyper+K")),
            ]))
            .await
            .unwrap();
        let expected = overrides(&[("settings.toggle", Some("Mod+Shift+P"))]);
        assert_eq!(saved, expected);
        assert_eq!(KeybindingsStore::open(dir.path()).get(), expected);
    }

    #[test]
    fn hand_edited_file_with_a_malformed_entry_loads_without_it() {
        let dir = temp_config_dir();
        std::fs::write(
            dir.path().join("keybindings.json"),
            r#"{"settings.toggle": "Mod+", "window.hide": "Mod+Q"}"#,
        )
        .unwrap();
        let store = KeybindingsStore::open(dir.path());
        assert_eq!(store.get(), overrides(&[("window.hide", Some("Mod+Q"))]));
        assert_eq!(
            store.menu_binding("settings.toggle").as_deref(),
            Some("Mod+,")
        );
    }
}
