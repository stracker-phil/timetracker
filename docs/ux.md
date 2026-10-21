# Design language

The reference for how TimeTracker looks and behaves. Update this page with every change that builds or
changes an item. Decisions: [ADR 0016](adr/0016-color-schemes.md) (schemes),
[ADR 0018](adr/0018-one-line-notes-tags-copyable-reports.md) (notes, tags, reports). Implementation (files,
registries, CSS structure): [frontend.md](frontend.md#styling).

## Principles

- Logging a day feels like arranging blocks, not filling a form. The Day view is the product; automation
  only suggests.
- Fully keyboard-usable, light and dark. Every action is a command with a rebindable shortcut and a palette
  entry.
- Mono for data values (times, durations, titles, URLs, paths), sans for chrome and labels.
- Hierarchy comes from opacity and space, not from size, color or boxes. No pills, badges or cards.
- The accent marks interactive things only: keys, links, focus, selection. A client's color is one of the
  scheme's nine accent hues (`--a-<name>`); its projects are shades of it: the hue and chroma of the accent at
  one of nine lightness steps (`shadeColor` in `components/Accent.tsx`, OKLCH relative colors).
- Native and light: the tracker is never noticed; the window opens instantly.

## Surfaces

| Token | Used for |
|---|---|
| `--chrome` | toolbar, sidebar, status bar |
| `--bg` | content: the main view |
| `--pop` | overlays: palette, popovers, context menus, dialogs, toasts; clearly brighter than `--bg` |

Separators between surfaces are `--line` hairlines. No inset panels, nested surfaces or tinted cards. Bars
inside content stay on `--bg` and are separated by hairlines. The window is transparent with the macOS
`sidebar` vibrancy behind the chrome.

## Color schemes and tokens

A scheme is a complete token set in `src/styles/schemes/<id>.css`, applied through `[data-scheme="<id>"]` on
`<html>` (and on the swatches that preview it in Settings). Settings → Appearance picks one scheme for light
and one for dark; the theme setting (system, light, dark) decides which applies. Built-in dark: `ayu-mirage`
(default), `monokai`, `nord`, `solarized-dark`; light: `ayu-light` (default), `catppuccin-latte`,
`github-light`, `solarized-light`. No color literal outside a scheme file; a Vitest scan
(`src/styles/colors.test.ts`) enforces it.

| Token | Ayu Mirage | Ayu Light | Role |
|---|---|---|---|
| `--chrome` | `#1f2430` | `#eef0f2` | chrome surface |
| `--bg` | `#242936` | `#fcfcfc` | content surface |
| `--bg-quiet` | `#1b1f29` | `#f1f2f4` | a pane beside the active one |
| `--pop` | `#323a4c` | `#ffffff` | overlay surface |
| `--field` | `#1a1f29` | `#ffffff` | input background (inset) |
| `--field-line` | `#3b4254` | `#cdd1d6` | input and overlay border, key caps |
| `--text` | `#cccac2` | `#5c6166` | primary text; `--o2`/`--o3` derive from it |
| `--text-strong` | `#ffffff` | `#000000` | the selected segment's label |
| `--line` | `#2f3545` | `#e6e8eb` | hairlines: rows, separators |
| `--band` | `#2a3040` | `#f2f3f5` | section header band |
| `--row-hover` | `#2b3142` | `#f0f2f5` | hovered row |
| `--shadow` | glow, zero offset | glow, zero offset | overlays |
| `--shadow-small`, `--shadow-inset` | per scheme | per scheme | switch thumb; field inset |
| `--scrim` | `rgba(10,12,18,.55)` | `rgba(40,50,70,.2)` | behind modal dialogs |
| `--on-accent` | `#1f2430` | `#ffffff` | text on an accent fill |
| `--accent-text-max-l` | `1` | `0.5` | the highest OKLCH lightness of `--accent-text` |
| `--knob` | `#cccac2` | `#ffffff` | switch thumb while off |
| `--a-orange`, `--a-gold`, `--a-red`, `--a-magenta`, `--a-purple`, `--a-indigo`, `--a-blue`, `--a-teal`, `--a-graphite` | per scheme | per scheme | the nine accent hues (`ACCENTS`); the app accent and project colors |
| `--style-red`, `-green`, `-yellow`, `-blue`, `-purple` | per scheme | per scheme | status tints (an error background is a `color-mix()` of `--style-red`) |

Derived tokens in `src/styles/theme.css`, the same for every scheme:

| Token | Value | Role |
|---|---|---|
| `--accent` | `var(--a-<choice>)` via `[data-accent]` on `<html>` | interactive things only |
| `--o2`, `--o3` | `--text` at 62% and 40% | secondary and tertiary text |
| `--sel` | `--accent` at 20% | selected row or sidebar entry |
| `--accent-text` | `--accent` with its lightness capped at `--accent-text-max-l` | the accent as text, readable on light surfaces |
| `--ui-font-size`, `--data-font-size` | 13px, 12px | set from Settings by `applyAppearance` (`lib/theme.ts`) |
| `--fs-small`, `--fs-base`, `--fs-large` | 11.5, 13 and 17px in rem | see type |
| `--font-ui`, `--font-mono` | Geist, JetBrains Mono | bundled defaults; Settings accepts any installed family |
| `--radius-sm`, `--radius`, `--radius-lg` | 6, 9, 12px | buttons and fields, overlays, dialogs |
| `--traffic-lights` | 88px | the toolbar's left inset for the window buttons (px: they don't scale) |

**Accent:** Settings → Appearance "Accent" picks the app accent from the nine swatches.

**Fonts:** Geist (400, 500, 600) and JetBrains Mono (400, 500, 600, italic 400) are bundled (SIL OFL).
Settings → Appearance "UI font" and "Mono font" take any installed family name; empty means the bundled one.

## Type

| Token | Size | Used for |
|---|---|---|
| `--fs-small` | 11.5px | captions, counts, labels in bands |
| `--fs-base` | 13px (sans, the UI size); mono values use `--data-font-size`, 12px | everything else |
| `--fs-large` | 17px | view titles (the day in the Day view header) |

Both sizes are settings: Appearance "UI size" (13px, 10–24) and "Data size" (12px, 9–28); ⌘= and ⌘- zoom
both, except in the Day view, where they zoom the timeline. Chrome sizes are in rem of the UI size. Weights 400, 500 and 600. Numbers and times use tabular
figures. Durations read `1 h 05 min` (`formatDuration`), clock times `09:30` (`formatClock`).

## Opacity levels

| Level | Token | Opacity | Used for |
|---|---|---|---|
| Primary | `--text` | 100% | the thing you came for |
| Secondary | `--o2` | 62% | labels, units, relative times, inactive items |
| Tertiary | `--o3` | 40% | counts, empty states, key caps, placeholders |

## Rows

Every list shares one row: sidebar entries, palette results, settings lists. A 20px line with padding above
and below, a 1px `--line` hairline below, `--row-hover` on hover, `--sel` when selected. Sidebar entries have
an 8px inset and a 5px radius; the active one is `--sel` with its label in the accent.

Lists grouped under a heading (the clients on the Projects screen) use a section header band: `--band`
background, small semibold `--o2` text, 5px radius. Archived rows are dimmed (55% opacity) and keep a
Restore action. A client band and a project row start with a color dot (`AccentDot`, a project's in its shade).
Colors are chosen with nine round swatches (`AccentSwatches` for a hue, `ShadeSwatches` for a shade), the
chosen one ringed. Tags are plain mono text, joined with ` · `
where several show in one place; never pills.

## Inputs

Inputs look like inputs, never like rows.

- **Field:** `--field` background, 1px `--field-line` border, 6px radius, `--shadow-inset`, a leading icon,
  placeholder in `--o3`, optional key cap on the right (`InsetField` in `components/Field.tsx`). Plain
  `.input` controls (dialogs, Settings) share the frame without the icon.
- **Segmented control** (`.segmented`): a field-styled track; segments in `--o2`, hover primary text; the
  selected segment is a rounded tile in `--field-line`, its label in `--text-strong`. Used for the
  Day/Week/Month switch in the toolbar and Settings choices.
- **Switch:** a track in `--field-line` with a `--knob` thumb; on, the track is the accent and the thumb
  `--on-accent`.
- **Key cap** (`KeyCap`): mono 11px, `--o3`, 1px `--field-line` border, 4px radius. Key caps follow the
  user's bindings.

## Overlays

Palette, popovers, context menus, toasts and dialogs: `--pop` background, 1px `--field-line` border, 10px
radius, `--shadow` (a glow with zero offset). Modal dialogs and the palette sit over `--scrim`. Overlays take
`Esc` first. Palette results use the row anatomy; the active one is `--sel` with a 2px accent bar on the left,
and command shortcuts show as key caps. Context menus use the UI font; the hovered or focused item is
`--accent` with `--on-accent` text, disabled items `--o3`, separators a 1px `--line`. The native WebKit
context menu is suppressed outside text fields (`lib/nativeMenu.ts`).

Pickers (project, tags) are a field with a list below it on `--pop`, using the palette's row anatomy and
small `--o3` section labels; the last option creates what was typed (`New project "x"…`).

## Timeline

The Day view is a vertical day: a fixed recordings column (`--recordings-col-w`, empty until recordings
exist), the time axis (`--timeline-axis-w`, hour labels in mono `--o3`; hour hairlines in `--line`, half
hours from 3 px per minute), then one lane per kind of activity (`--lane-min-w` each): Activity, an app's own
lane, an external source's lane (its name in mono). Lane labels stick to the top in `--o3`.

- **Blocks** are flat: `--text` at 10% on `--bg`, a `--line` hairline, 4px radius. The app name in sans
  `--fs-small`, the title in mono `--o2`; short blocks put both on one line, tiny ones show none. Private
  blocks read "Private" in `--o3`. Overlapping blocks of one lane share its width side by side.
- **Focus** (J/K/H/L or a click) is a 2px accent outline; Space opens the details card for it.
- **Hover card** after 300 ms on `--pop` with `--shadow`: app, full title, URL or path (selectable; an
  http(s) URL opens in the browser), `09:12–09:40 · 28 min`, and the source when external.
- **Away spans and gaps** are dashed `--line` outlines across all lanes; away spans read "Away · idle 25
  min" when tall enough, gaps (2 min or more between the day's first and last activity) have no label.
- **Now** is a 1px accent line on today, updated every 30 s.
- **Empty day:** "Nothing tracked on this day" with the tracker's status line; without the Accessibility
  permission "Tracking needs permission" with Allow….
- **Zoom** is 1–8 px per minute (default 2.5), kept in `ui_state.json` as `timelineZoom`.
- **Summary lane** comes first, on `--band`: one block per run of intervals with the same main app.
- **Compact** draws the day at 60% of the zoom, every block on one line.
- **Block gear:** a hovered block shows a gear at its top right; it opens the same menu as a right-click.

### Timeline settings

Settings → Timeline and the Day header's gear (both save at once; the Day view reloads):

| Setting | Values | Does |
|---|---|---|
| Summary lane | on (15 min) | a first lane with the main app of every 5, 10, 15, 20, 30, 45 or 60 minutes |
| Show all activity | on | the activity lane; off leaves the summary, app and source lanes |
| Compact activity | off | shorter rows, one line per block |
| Group by app and website | on | one block per app, whatever its windows are called; off splits by title |
| Continuity | short | joins blocks of one app across gaps under 2 (short), 8 (regular) or 16 (extended) min; never across sleep, lock or a pause |
| Noise cancellation | low | hides blocks under 2 (low), 4 (medium) or 6 (high) min; tracked time still counts them |
| Titles | document | label blocks with the document or page name, or with the app |
| Time snapping | 5 min | the grid recordings snap to: 1, 5, 10 or 15 min |

Per app (Settings → Timeline → Apps, or a block's menu): Group by app and Titles (default follows the
timeline), Hidden (gone from every lane; the header popover lists hidden apps with Unhide), Own lane.

## Recordings

- **Selection.** Drag on the lanes or the axis, or click a block (⇧-click adds blocks): `--sel` fill with
  accent hairlines and the snapped `09:00–10:30` in mono accent text; the status bar repeats it with the
  duration. A dashed accent outline in the recordings column shows where the recording goes.
- **Recording blocks** sit in the recordings column: the project's shade as a 3px bar on the left and a 22%
  tint on `--bg`; project name (sans 500), note (mono), tags (mono `--o3`, ` · `-joined), duration bottom
  right. A dashed top or bottom edge means the recording continues on another day. The top and bottom 6px
  resize; the body moves. Activity under recordings (half or more) is dimmed; hover lifts it.
- **Editor**: a sheet with project, From/To, duration, note, tags; Save, Cancel, Delete. Field errors show
  under their field.
- **Overlap**: a confirm sheet listing the recordings in the way; "Replace overlapped part" trims, splits
  or deletes them.
- Every change ends in a toast with Undo.

## Keys

Defaults; every key is a rebindable command (Settings → Shortcuts, `keybindings.json`). Bare keys never fire
in inputs or dialogs. `Tab` is never bound: it moves the keyboard focus.

| Key | Scope | Action |
|---|---|---|
| ⌘, | global | Settings (`settings.toggle`) |
| ⌘K | global | command palette (`palette.toggle`); `>` lists commands, `p:` projects |
| `?` | global | shortcut overview (`shortcuts.show`) |
| ⌘⌃S | global | toggle the sidebar (`sidebar.toggle`) |
| ⌘1 … ⌘5 | global | Day, Week, Month, Reports, Projects (`view.*`) |
| ⌘= / ⌘- / ⌘0 | global | zoom in / out / reset (`zoom.*`): the timeline in the Day view, the text elsewhere |
| ⌘← / ⌘→ / T | day | previous day, next day, today (`nav.*`) |
| J / K | day | next / previous block in time (`day.nextBlock`, `day.prevBlock`) |
| H / L | day | the block in the lane to the left / right (`day.prevLane`, `day.nextLane`) |
| Space / ⌃↵ | day | the focused block's details card, its menu |
| ↵ | day | record the selection, or edit the focused recording |
| ⌫ | day | delete the focused recording (Undo in the toast) |
| ⌥↑ / ⌥↓, ⌥⇧↓ / ⌥⇧↑ | day | move the focused recording, extend or shorten it, by one snap step |
| Escape | day | clear the selection and the focus |
| ⌘Z / ⌘⇧Z | global | undo / redo the last recording change (outside text fields) |
| ⌘W | global | close (hide) the window (`window.hide`) |
| ⌘Q | menu | quit (`app.quit`) |

Scopes are the registry's (`src/commands/scopes.ts`): `global`, and under it one scope per view (`day`,
`week`, `month`, `reports`, `projects`), active while that view shows.
