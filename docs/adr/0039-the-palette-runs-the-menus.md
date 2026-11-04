# 0039. The palette runs the menus

## Status

Accepted (2026-11-04), implemented. Inherited from Taskboard ADR 0043. Builds on
[0038](0038-app-commands-live-in-a-rust-bus.md) and [0037](0037-menus-are-drawn-in-the-page.md).

## Context

The palette listed the page's registered commands of the active scopes. Once app commands moved to the Rust
bus (0038), the page no longer had them, and its list already differed from the menu bar: different titles
("Settings…" and "Settings…", "Close window" and "Close Window"), commands only one of them had, and its own
order. What the page can do now (a focused recording, an open saved report, the tracker line's Pause menu)
lives in the page, which Rust can't see.

## Decision

- **The menus are the list.** `menu/palette.rs` (`menu_commands`) lists the items of `MENUS`, the table the menu
  bar is built from, in menu order with the menu's title, the label, the keys and the checkmark; Command
  Palette… itself is left out, app items only while the bus enables them. A new menu item is a palette entry
  without further work, and a command reaches the palette by getting a menu item (Pause Tracking, Save
  Report…, Rename and Delete Saved Report… joined the File menu for that).
- **The page decides its own items.** A `page:` item comes from Rust unfiltered and is listed only when its
  command could run as the palette opened (`notePaletteRunnable`): open, the palette is an overlay that holds
  page commands back, and its field takes the focus some of them read. It runs the page command directly, an
  app item `api.runCommand`.
- **Menus on screen add their rows.** Components offer theirs while mounted with `usePaletteMenu`
  (`components/palette/menus.ts`), noted as the palette opens and flattened ("Pause tracking: For 1 hour").
- **Ranked on the page,** with TimeTracker's own scoring and sections (Go to, Settings, Projects, Clients, Saved
  reports), the menus' items one section per menu.
- Rejected: listing the page's commands in Rust (the page would report every command's state on every change);
  a palette list of its own beside the menus (the two drift, as they had); Rust filtering `page:` items by the
  window's report (taken while the palette is open, it greys all of them).

## Consequences

- The palette and the menu bar can't disagree: same items, same labels, same order.
- Key-only page commands (H, L, Space, ⌥↑, moving rules) are no longer in the palette.
- The palette asks Rust once per opening; its menu rows arrive a moment after the sheet.
