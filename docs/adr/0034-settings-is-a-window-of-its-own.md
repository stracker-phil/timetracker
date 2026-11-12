# 0034. Settings is a window of its own

## Status

Accepted (2026-11-04), implemented. Inherited from Taskboard ADR 0010 (its Settings part). Amends
[0024](0024-native-macos-look.md), which kept Settings as a panel in the main window. Superseded in part by
[0049](0049-the-settings-search-finds-every-row.md) (the Search bullet).

## Context

Settings was a modal panel inside the main window (`SettingsDialog`, opened by `settings.toggle`), with no
search. While it was open the day behind it was hidden and its keys were blocked. Changes made there didn't
reach the timer picker or the away prompt until they reloaded on focus. System Settings, the model for its
layout, is a window of its own with a searchable page list.

## Decision

- **A window.** `window/settings.rs` builds the `settings` window on first use (about 880 × 720, at least
  760 × 560, not maximizable, overlay title bar) and hides it on close, so it reopens on the page it showed.
  `open(app, page)` shows it and brings it forward. A page or section id travels as `?page=` on first open
  and as `open_settings` after. The page is `src/windows/settings/`.
- **Opening.** ⌘, (the menu's `settings.toggle`), the tray's Settings…, the palette, the page's gear buttons
  and `timetracker://command/app:settings[:<page>]` all open or focus the window. ⌘W and Esc hide it.
- **One state across windows.** Rust announces `settings_changed` (the saved `Settings`) after every save
  and `keybindings_changed` after a shortcut change, to every window. The main window, the timer picker and
  the away prompt follow them instead of reloading on focus. `data_changed { kind: settings }` stays for
  view reloads.
- **Search.** The page list has a search field that narrows it by page label, group, section title and the
  section's row labels (`registry/settingsPages.ts`). ↵ opens the first hit, and a section hit opens its
  page scrolled to that group. ⌘F focuses the field.
- **Fields commit on blur or ↵** (`fields.tsx`), and Rust's `invalid_input` message shows under the field.
- Rejected:
  - the modal panel, because it hid the day, blocked its keys and could not be searched;
  - Settings as a view of the main window, because it would replace the view the owner is checking a
    setting against.

## Consequences

- Settings runs in its own webview with its own store: it loads the settings itself and follows the events.
  The welcome sheet stays the main window's (`show_welcome`).
- Escape in Settings belongs to a focused field or an open sheet first.
