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

- **Selection.** Drag on the lanes or the axis, click a block (⇧-click adds blocks), or V on the focused
  block (⇧J and ⇧K add the next or previous one): `--sel` fill with
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

## Welcome

- A fresh config dir opens a sheet over the Day view with three short panels, "1 of 3" left of Back and
  Next: what is recorded (app, window title, URL or document, away time), what never is, and the data
  folder's path; Accessibility, with Grant access, a live status line and Skip for now (Next waits for one
  of the two); Launch at login and the global timer shortcut as a key cap, then Done.
- Done or Escape closes it for good; Settings → General → "Show welcome again" brings it back. The
  permission dialog waits until the welcome is closed.
- Launch at login starts the app hidden: only the tray icon shows.

## Away prompt

- After an away span of at least the set minutes (Settings → Tracker; not after a pause), a small sheet on
  `--pop` opens on top of everything: `You were away 42 min`, `12:03–12:45 · idle` (asleep, locked), then
  project (the last used one), From and To (snapped), note, tags, and Record (↵) / Skip (Escape).
- A recording in the way reads `A recording already covers 12:10–12:30 (Project)` above the buttons; the
  form stays, so the times can change. The sheet stays open when another app takes focus.

## List view

- ⌘⇧L or Day mode → List shows the day as rows grouped like the timeline, longest first: app in `--o2`, the
  label in mono, URL or path below, the duration right with a thin covered bar (`--sel` over `--line`) and
  `· 3 spans` when the activity came in parts.
- A strip of project chips (recent first, More… for the rest) sits above the rows. Drag a row onto a chip
  to record it: the editor opens with the row's first start to last end and, for several spans, the warning
  `This activity has gaps (3 spans, 0:40 in between). Adjust the times if needed.`
- Keyboard: `/` search, ↑↓ rows, ↵ record (pick the project), Space show on the timeline.

## Rules and suggestions

- **Suggestions** sit in the recordings column under the recordings: a dashed outline in the project's shade
  on a 7% tint, a "+" top right, the project (sans 500) and the rule's name (`--o2`), the duration bottom
  right. The cursor is `copy`. A click records it; ⌥-click records every suggestion of that rule on the
  day. Each is a button, so Tab reaches it, ↵ accepts it and ⌥↵ every suggestion of its rule. The tooltip names the rule, project, span and note.
- **Drop strip**: ⌥-dragging an activity block shows the recent projects on `--pop` over the top of the
  recordings column; the one under the pointer gets a dashed accent outline. Dropping opens the rule sheet,
  filled in; nothing is saved before Save. From the keyboard, the block menu's Make a rule… (⌃↵) opens the
  sheet with the block's conditions and no project.
- **Rules screen**: rows with a grip, the enable switch, the name, the project's dot and name and the
  conditions in mono `--o2`; a disabled rule is dimmed. The order is the priority; drag the grip, or ⌥↑ ⌥↓ with the focus in a row.
- **Rule sheet**: the form on the left, "Last 7 days" on the right with the matching time and the hits
  (day and time in mono `--o2`, the app, the title). Below 720px the preview moves under the form.

## Timer

- A running recording grows to now in the Day view, with a pulsing accent dot at its top right (still with
  reduced motion); it has no grips. The menu bar item's title reads `⏱ 1:23 Acme` while it runs, and the
  status bar shows `⏱ Acme 1:23` (a click stops it).
- The picker (⌘⇧T from any app, ⌘T in the window, Start Timer… in the tray) is a small floating panel on
  `--pop` under the tray icon: project, note, tags, Start (↵), Cancel (Escape), and the recent timers below.
  While a timer runs it shows the timer with Stop and "Start another…". It hides when it loses focus.

## Week and Month

- **Week**: the time axis and seven day columns (`--lane-min-w` like lanes); each head shows `Mon 20` and the
  day's recorded hours, today's in accent text; today's column has a faint tint. Recordings look and drag
  as in the Day view; dropped on another column they move there, with ⌥ they are copied. A double-click on
  an empty spot records an hour there. The header totals read `recorded 32:10 · tracked 41:05`.
- **Month**: a calendar of whole weeks; each day a cell with its number (`--o3` outside the month, accent
  on today), the recorded hours in mono, and a 3 px strip along the bottom of the three projects with the
  most time, as wide as their shares. A click opens the day.

## Reports

The Reports screen: the range as title, Copy as text and Save report… on the right; below, the filter bar
(range presets, clients, projects, tags, group by, rounding, archived projects). Each group is its label
with a color dot (by project), the hours in mono on the right, the notes in `--o2` below, and "3
recordings" that unfolds a table. The total closes the list; with rounding, the unrounded hours stand
beside it in `--o3`. Saved reports sit under Reports in the sidebar.

The copied text, also what `timetracker report` prints:

```text
2026-10-19 – 2026-10-25 · rounded to 15 min

Northwind / Website relaunch  3:30 h
  - fix login (Mon, Wed)
  - review (Fri)

Internal  1:15 h

Total  4:45 h
```

Notes carry their weekdays when the range is longer than a day (dates beyond a week), except by day.
Grouped by tag, `Tags overlap: a recording with several tags counts in each.` follows the total when it
applies. No recordings: the header, then `No recordings.`

## Data folder

Settings → Data → Move… opens the macOS folder panel (it can create a folder), then a sheet: the database is
copied there and used from then on, tracking pauses while it copies, and the old files stay where they were
(both paths shown, mono, selectable). Move is the default button; while the copy runs it reads `Moving…`,
both buttons are disabled and Escape does nothing. Success is a toast naming the new and the old folder. A
folder TimeTracker refuses (inside the current folder, already holding data, not writable) is explained in red
under the Location row instead of a toast.

## Keys

Every action is a command (`APP_COMMANDS` in `src/commands/index.ts`) with a rebindable key (Settings →
Shortcuts, `keybindings.json`) or, without a default key, an entry in the palette (⌘K, `>` lists the commands
of the active scopes). `?` lists the bound commands of the scopes active when it opens, most specific first.

Rules of the keys (`handleKeydown`):

- Bare keys never fire in text fields; there only ⌘ and ⌃ combinations and Escape do.
- While a dialog, the palette, the recording editor or a context menu is open, only Settings, `?`, the
  palette, zoom, Close window and Quit run; every other key belongs to the overlay.
- ↵ and Space on a focused button or link activate it. J, K, H, L, V, ⇧J, ⇧K, N and ⇧N give the keys back to
  the timeline: they take the focus off a Tabbed-to button.
- `Tab` is never bound: it moves the keyboard focus. Dialogs keep it inside (`Modal`), in reading order:
  fields first, then Cancel and the default button; the first control, or the one a dialog marks, has the
  focus, and closing returns it to where it was.

Scopes are the registry's (`src/commands/scopes.ts`): `global`; `calendar` under it, active in Day, Week and
Month; and one scope per view (`day`, `week`, `month` under `calendar`; `reports`, `projects`, `rules` under
`global`), active while that view shows.

| Command | Label | Default key | Scope |
|---|---|---|---|
| `settings.toggle` | Settings… | ⌘, | global |
| `shortcuts.show` | Keyboard shortcuts | ? | global |
| `palette.toggle` | Command palette | ⌘K | global |
| `sidebar.toggle` | Toggle sidebar | ⌘⌃S | global |
| `view.day` … `view.rules` | Show Day, Week, Month, Reports, Projects, Rules | ⌘1 … ⌘6 | global |
| `zoom.in` / `zoom.out` / `zoom.reset` | Zoom in / out / Reset zoom (the timeline in the Day view, the text elsewhere) | ⌘= / ⌘- / ⌘0 | global |
| `window.hide` | Close window | ⌘W | global |
| `tracker.pause` | Pause tracking (until resumed) | palette | global |
| `tracker.resume` | Resume tracking | palette | global |
| `timer.start` | Start timer… | ⌘T (⌘⇧T from any app) | global |
| `timer.stop` | Stop timer | ⌘⇧. | global |
| `timer.toggle` | Start or stop timer | palette | global |
| `edit.undo` / `edit.redo` | Undo / Redo recording change (outside text fields) | ⌘Z / ⌘⇧Z | global |
| `logs.open` | Open log folder | palette | global |
| `app.quit` | Quit TimeTracker | ⌘Q | global |
| `nav.prev` / `nav.next` | Previous / Next day, week or month | ⌘← / ⌘→ | calendar |
| `nav.today` | Go to today | T | calendar |
| `day.nextBlock` / `day.prevBlock` | Next / Previous block in time | J / K | day |
| `day.prevLane` / `day.nextLane` | Block in the lane to the left / right | H / L | day |
| `day.details` | Block details | Space | day |
| `day.blockMenu` | Block actions… (app settings, copy, Make a rule…) | ⌃↵ | day |
| `day.selectBlock` | Select block (the focused one, as a click does) | V | day |
| `day.extendNext` / `day.extendPrev` | Extend selection to the next / previous block (as a ⇧-click does) | ⇧J / ⇧K | day |
| `day.toggleList` | Toggle timeline and list | ⌘⇧L | day |
| `activity.deleteDay` | Delete activity for this day… | – | day |
| `activity.deleteRange` | Delete activity in a range… | – | day |
| `day.settings` | Timeline settings | palette | day |
| `day.clearFocus` | Clear selection (and the block and recording focus) | Escape | day |
| `recording.next` / `recording.prev` | Next / Previous recording | N / ⇧N | day |
| `recording.open` | Record selection or edit recording | ↵ | day |
| `recording.create` | New recording from selection | palette, menu | day |
| `recording.edit` | Edit recording | palette, menu | day |
| `recording.delete` | Delete recording (Undo in the toast) | ⌫ | day |
| `recording.nudgeEarlier` / `recording.nudgeLater` | Move recording earlier / later by one snap step | ⌥↑ / ⌥↓ | day |
| `recording.extend` / `recording.shrink` | Extend / Shorten recording by one snap step | ⌥⇧↓ / ⌥⇧↑ | day |
| `recording.moveToPrevDay` / `recording.moveToNextDay` | Move recording to the previous / next day | ⌥← / ⌥→ | day |
| `recording.copyToPrevDay` / `recording.copyToNextDay` | Copy recording to the previous / next day | ⌥⇧← / ⌥⇧→ | day |
| `reports.copyText` | Copy report as text | ⌘⇧C | reports |
| `reports.save` | Save report… | palette | reports |
| `reports.rename` / `reports.delete` | Rename / Delete saved report… (the one opened from the sidebar, until the filter changes) | palette | reports |
| `projects.new` | New project… | ⌘N | projects |
| `clients.new` | New client… | palette | projects |
| `rules.new` | New rule… | ⌘N | rules |
| `rules.moveUp` / `rules.moveDown` | Move rule up / down (the rule whose row has the focus) | ⌥↑ / ⌥↓ | rules |

Keys of controls, not commands (they act on the focused control and are not rebindable):

| Where | Keys |
|---|---|
| List mode | `/` search, ↑↓ rows, ↵ record the row (pick the project), Space show it on the timeline; Tab to a project chip and ↵ records the focused row |
| Suggestion (Tab reaches it) | ↵ records it, ⌥↵ records every suggestion of its rule on the day |
| Recording editor, away prompt, timer picker | project picker: type, ↑↓, ↵ picks (the last option creates a project); From and To: type `HH:MM` or ↑↓ by one snap step; ↵ saves, records or starts; in the editor also ⌘↵, and ⌘⌫ deletes; Escape cancels or skips |
| Context menu (⌃↵ on a block) | ↑↓ items, ↵ runs, Escape closes |
| Pickers in the Reports filter | type, ↑↓, ↵ adds, ⌫ on an empty field removes the last |
| Week and Month | Tab to a day head (Week) or a day cell (Month), ↵ opens that day |
| Settings → Shortcuts | a key field records the next key; Escape stops, ⌫ unbinds |

Left to the pointer, each with a keyboard way to the same result: dragging a selection (V, ⇧J, ⇧K, or the
List mode), dragging or resizing a recording (⌥↑ ⌥↓ ⌥⇧↑ ⌥⇧↓), dragging a recording to another day in the
Week view (⌥← ⌥→, ⌥⇧ to copy, in the Day view), double-clicking the Week view (select in the Day view and ↵),
⌥-dragging a block onto a project (⌃↵ → Make a rule…), the rules' grip (⌥↑ ⌥↓), the hover card (Space), the
saved reports' right-click menu (`reports.rename`, `reports.delete`), and the Projects screen's right-click
menu (Edit and Archive are in the project sheet). Only the right-click "Copy id" of a project has no key; the
CLI and the MCP server list the ids.

### Record a day

From any view, with the documented default keys only:

1. ⌘1 shows the Day view, T today.
2. J focuses the first block of the day (K, H and L move on); Space shows its details.
3. V selects the focused block's span; ⇧J (or ⇧K) grows the selection over the next (or previous) block, as
   far as the work went. The status bar shows the span and its length; Escape clears it.
4. ↵ opens the editor for the selection, with the project field focused: type part of the name, ↑↓, ↵ picks
   it (or the last option, `New project "x"…`, opens the project sheet).
5. Tab to From and To: type `09:15` or press ↑↓ to move by one snap step; Tab to the note and the tags.
6. ⌘↵ saves (↵ in a time or the note field does too). If it overlaps a recording, ↵ in the dialog replaces
   the overlapped part and Escape goes back.
7. The new recording has the focus: ⌥↑ ⌥↓ move it, ⌥⇧↓ ⌥⇧↑ change its end, ↵ edits it again, ⌫ deletes it,
   ⌘Z undoes the last change. N and ⇧N step through the day's recordings.
8. Back to step 2 for the next stretch (J moves on from the last focused block). Rule suggestions are
   buttons in the recordings column: Tab to one, ↵ records it.
9. ⌘← and ⌘→ step to other days. ⌘2 shows the week (⌘← ⌘→ by week; Tab to a day head and ↵ opens it), ⌘3
   the month (by month; Tab to a day and ↵ opens it).
10. ⌘4 shows Reports: Tab to the range (This week, Last week, …) and ↵, on to the client, project and tag
    pickers, Group by and Rounding; ⌘⇧C copies the report as text, ⌘K `> save` saves it under a name.
