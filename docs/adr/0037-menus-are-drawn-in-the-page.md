# 0037. Menus are drawn in the page

## Status

Accepted (2026-11-04), implemented. Inherited from Taskboard ADR 0034. Supersedes the native pop-up buttons of
[0024](0024-native-macos-look.md); the app's menu bar and the tray stay native.

## Context

TimeTracker had two kinds of menu. Context menus (a block, a saved report, a project row, the Pause durations,
the ⋯ day actions, Copy as text's grouping) were `ContextMenu.tsx`, a page-drawn list each caller mounted itself
through `useContextMenu`, with only ↑ ↓ for keys and its own placement. Choices (the report's Period and
Rounding, the chart's By, a project's Client, a rule's comparison, the away prompt's minutes) were native
`<select>` pop-up buttons, as 0024 decided. The two looked and behaved differently: WebKit's pop-up menu takes
the system's look, can't follow the app's palettes or accent, keeps its own keys, can't be tested in jsdom
beyond `selectOptions`, and sits beside page-drawn popovers. Taskboard, the sibling app with the same stack,
replaced both with one in-page menu (its ADR 0034) and found it closer to a native macOS menu than either.

## Decision

- **One in-page menu.** `components/menu` draws every context menu, every button's menu and every pop-up
  button's choices. The store slice `state/slices/menu.ts` holds the open menu; `MenuHost`, mounted once per
  window that has menus (the main window and Settings), shows it portaled to `<body>`; `openMenu(entries, at,
  label)` and `openContextMenu(e, entries, label)` open it from anywhere. Rows carry a check, a danger style, a
  hint, a second line (`detail`), a tooltip (`title`), an icon, a caption or a submenu.
- **It behaves like a macOS menu.** It opens at the mouse or under its button, flips and shifts to stay inside
  the window, closes on Escape, a press outside, a scroll, a resize or the window losing focus, moves with the
  arrow keys, Home, End and typed letters, runs a row on ↩ or Space, and gives the focus back to its opener.
  Roles are `menu`, `menuitem`, `menuitemradio` and `menuitemcheckbox`.
- **Pop-up buttons are `PopUpButton`.** A push button with the current choice and a chevron, as wide as its
  widest choice, opening its choices as `menuitemradio` rows with the current one checked. A `<label htmlFor>`
  keeps naming it. No `<select>` remains.
- `ContextMenu.tsx` and `useContextMenu` are gone.
- Rejected:
  - native menus with custom item icons (Tauri's menu API from the page): no detail lines, captions or colors
    from the palettes, an IPC round trip per menu, and still a second look beside the popovers;
  - keeping `<select>` for choices: WebKit's own pop-up look and keys beside the in-page context menus, no
    check-and-accent styling, and tests limited to `selectOptions`.

## Consequences

- Menus follow the palettes (light and dark) and the accent, and are tested like any component, without IPC.
- A menu can't extend past the window's edges as a native one can; it flips and shifts inside instead.
- Accessibility, keys and placement are the app's to keep right (`docs/ux.md` → Menus); a component test that
  opens a menu renders `MenuHost` beside the component.
- Escape belongs to an open menu first: `Modal` and the Settings window's close command leave it alone.
