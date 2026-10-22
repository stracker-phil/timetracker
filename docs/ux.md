# Design language

The reference for how TimeTracker looks and behaves. Update this page with every change that builds or
changes an item. Decisions: [ADR 0016](adr/0016-color-schemes.md) (token sets, no literals),
[ADR 0021](adr/0021-appearance-modes-and-schemes.md) (modes and schemes),
[ADR 0018](adr/0018-one-line-notes-tags-copyable-reports.md) (notes, tags, reports). Implementation (files,
registries, CSS structure): [frontend.md](frontend.md#styling).

## Principles

- It looks like a calendar (Fantastical, Timely), not a code editor: proportional type, filled colored
  blocks, cards, generous spacing, a dark sidebar by default.
- Logging a day feels like arranging blocks, not filling a form. The Day view is the product; automation
  only suggests.
- Fully keyboard-usable, light and dark. Every action is a command with a rebindable shortcut and a palette
  entry.
- Geist for all text, titles included; mono only for times and durations (tabular figures), and for machine
  text such as paths, URLs and commands.
- Hierarchy comes from weight, opacity and space: a strong title, then a normal subtitle, then meta in
  `--o2`. Recordings are filled cards in their project's color; activity is a soft grey surface.
- The accent marks interactive things only: keys, links, focus, selection. A client's color is one of the
  nine accent hues (`--a-<name>`); its projects are shades of it: the hue and chroma of the accent at one of
  nine lightness steps (`shadeColor` in `components/Accent.tsx`, OKLCH relative colors).
- Native and light: the tracker is never noticed; the window opens instantly.

## Surfaces

| Token | Used for |
|---|---|
| `--sidebar-bg` | the sidebar, full height, under the window buttons |
| `--bg` | content: the toolbar and the main view |
| `--chrome` | the status bar, the Settings page list |
| `--pop` | overlays: palette, popovers, context menus, dialogs, toasts, Settings cards |

Separators are `--line` hairlines (`--sidebar-line` beside the sidebar). Bars inside content stay on `--bg`.
Every surface is opaque; the window's `sidebar` vibrancy never shows.

## Shell

One place for views and one place for dates: the sidebar.

- **Sidebar**: full height on its own palette. On top a drag strip under the window buttons, then the date
  navigator, then the views (Day to Rules, ⌘1 to ⌘6) with saved reports under Reports. The navigator shows
  the active period as a heading in `--fs-title` semibold: Day `Wed, Oct 21` (with the year when it is not
  this year's), Week `Oct 19 – 25`, Month `October 2026`, Reports the period's name (`Today`, `This week`, …)
  or its days (`Oct 5 – 14`). Below it ‹ and › and a Today button labelled by the unit (Today, This week,
  This month), disabled while the period holds today. In Reports ‹ › step the period by its own length: a
  month, a week, or its number of days; a step that lands on a preset shows its name (a week back from This
  week is Last week). Projects and Rules keep the navigator's space empty. Clicking the heading opens a month
  calendar under it (today in accent text, the shown day, week or month on `--sel`); a day shows that day,
  its week or month, or in Reports the report's period that holds it.
- **Toolbar**: a thin drag strip beside the sidebar with the app title and the Settings button. Without the
  sidebar it leaves `--traffic-lights` for the window buttons.
- **View headers** keep their titles, totals and view switches, never date controls. The Day header shows
  the shown day's totals only when it is not today.
- **Status bar** (plain text in `--o3` on `--chrome`, beside the sidebar): on the left the sidebar toggle,
  the tracker's state (a dot: accent while running, `--o3` while away or paused, `--style-red` without
  permission; the tray's text; Pause, Resume or Allow…) and the selection; on the right the running timer, the
  billable ratio and today's totals in mono, `recorded 1:00 · tracked 3:12`, in every view. The billable ratio
  is a small bar (a neutral track, filled in the accent; no red or green judgment) with `Billable 80 %`, and
  `Billable 6:24 / 8:00` in its tooltip, for the period the navigator shows (day, week, month, report period;
  today on Projects and Rules). It hides while nothing is recorded in that period. The text zoom has no
  readout: ⌘= ⌘- ⌘0.

## Appearance and tokens

Settings → Appearance has **Mode** (Light, Dark, System; System follows macOS and is the default) and
**Scheme** (three tiles: Default, a dark sidebar beside light content, the default; Light, all light; Dark,
all dark). In dark mode the whole window is dark whatever the scheme, the sidebar a shade darker than the
content; the scheme decides the light-mode look ([ADR 0021](adr/0021-appearance-modes-and-schemes.md)).

| Mode → | Light | Dark |
|---|---|---|
| Default | dark sidebar, light content | dark sidebar, dark content |
| Light | light sidebar, light content | dark sidebar, dark content |
| Dark | dark sidebar, dark content | dark sidebar, dark content |

`resolveLook` (`lib/theme.ts`) turns mode and scheme into a content tone and a sidebar tone and sets
`data-content` and `data-sidebar` on `<html>` (Settings' scheme tiles set them on their own element). Four
palettes in `src/styles/palettes/` answer them, warm-grey neutrals designed for this app: `content-light`,
`content-dark`, `sidebar-light`, `sidebar-dark`. Palettes of a kind define the same tokens; no color literal
appears outside them (`src/styles/colors.test.ts`).

**Content palette** tokens:

| Token | Light | Dark | Role |
|---|---|---|---|
| `--bg` | `#fbfaf8` | `#1f1e1c` | content surface |
| `--chrome` | `#f4f2ee` | `#1a1917` | status bar, Settings page list |
| `--bg-quiet` | `#f5f3f0` | `#1b1a18` | a pane beside the active one |
| `--pop` | `#ffffff` | `#2c2b28` | overlays and Settings cards |
| `--field`, `--field-line` | `#ffffff`, `#d6d1ca` | `#171615`, `#403e3a` | input background and border; overlay border, key caps |
| `--text`, `--text-strong` | `#2a2825`, `#12110f` | `#ebe8e3`, `#ffffff` | primary text (`--o2`, `--o3` derive from it); titles and pressed segments |
| `--line` | `#dbd6cf` | `#383632` | hairlines |
| `--band`, `--row-hover` | `#f1eee9`, `#efece7` | `#262523`, `#2a2926` | section band; hovered row |
| `--track`, `--raised` | `#ebe8e3`, `#ffffff` | `#161514`, `#3a3834` | segmented control track and pressed segment |
| `--shadow`, `--shadow-small`, `--shadow-inset` | soft, warm | deeper | overlays; raised segment, switch thumb; field inset |
| `--scrim` | warm 22% | black 50% | behind sheets and panels |
| `--knob`, `--shade` | | | switch thumb while off; black for `color-mix` darkening |
| `--a-<name>`, `--accent-text-max-l` | deep hues, `0.56` | light hues, `1` | the nine accent hues (`ACCENTS`): app accent and project colors; the highest OKLCH lightness of `--accent-text`. They also apply inside a sidebar of the same tone |
| `--on-accent` | white | `#1a1917` | text on any accent's fill, at least 4.5:1 on every hue |
| `--style-green` | | | the success tint; red, yellow, blue and purple are accent hues (`theme.css`) |
| `--activity-fill`, `-hover`, `-dim` | `#efebe5` … | `#2d2c29` … | activity blocks; hovered; covered by a recording |
| `--away-band` | `#ebe7e1` | `#171615` | away and idle bands on the timeline |
| `--block-fill-l`, `-spread`, `-c` | `0.91`, `0.1`, `0.07` | `0.36`, `0.08`, `0.06` | a filled recording's OKLCH lightness, lightness spread per shade, chroma cap |
| `--block-text-l`, `-c` | `0.32`, `0.1` | `0.94`, `0.035` | its text's lightness and chroma cap |

**Sidebar palette** tokens (everything inside the sidebar uses only these; `theme.css` maps the generic
tokens of shared controls to them inside `.sidebar`, and the accent hues come from the content palette of the
sidebar's tone):

| Token | Dark | Light | Role |
|---|---|---|---|
| `--sidebar-bg` | `#161514` | `#efece7` | the surface |
| `--sidebar-text`, `--sidebar-strong` | `#e9e6e1`, `#ffffff` | `#2a2825`, `#12110f` | entries; the selected entry and the date heading |
| `--sidebar-muted` | `#a29d95` | `#615c55` | icons, key caps, saved reports |
| `--sidebar-hover`, `--sidebar-selected` | `#22211f`, `#2e2c29` | `#e6e2dc`, `#e0dbd3` | hovered and selected entry |
| `--sidebar-line` | `#2a2826` | `#ddd8d1` | its border, the Today button |

Derived tokens in `src/styles/theme.css`, the same for every palette:

| Token | Value | Role |
|---|---|---|
| `--accent` | `var(--a-<choice>)` via `[data-accent]`; re-derived inside the sidebar | interactive things only |
| `--style-red`, `-yellow`, `-blue`, `-purple` | `--a-red`, `--a-gold`, `--a-blue`, `--a-purple` | status tints (an error background is a `color-mix()` of `--style-red`) |
| `--o2`, `--o3` | `--text` at 68% and 52% | secondary and tertiary text |
| `--sel` | `--accent` at 20% (`--sidebar-selected` in the sidebar) | selected row |
| `--accent-text` | `--accent` with its lightness capped at `--accent-text-max-l` | the accent as text |
| `--block-fill`, `--block-text`, `--block-meta` | from the project's shade `--rec` on `.card`, `.suggestion`, `.project-chip` | a filled recording: the fill, its text, its foot (text 75% over the fill) |
| `--ui-font-size`, `--data-font-size` | 13px, 12px | set from Settings by `applyAppearance` (`lib/theme.ts`) |
| `--fs-title`, `--fs-body`, `--fs-meta`, `--fs-caption` | 17, 13, 12 and 11px in rem | the type scale |
| `--space-1` … `--space-6`, `--space-8` | 4 to 24px and 32px in rem | the 4px spacing grid |
| `--font-ui`, `--font-mono` | Geist, JetBrains Mono | bundled defaults; Settings accepts any installed family |
| `--radius-sm`, `--radius`, `--radius-lg` | 6, 9, 12px | buttons and fields; menus and popovers; sheets and panels |
| `--radius-block`, `--radius-card` | 6, 8px | activity blocks; recording cards and Settings cards |
| `--traffic-lights` | 88px | the toolbar's left inset for the window buttons without the sidebar (px: they don't scale) |

**Accent:** Settings → Appearance "Accent" picks the app accent from the nine swatches.

**Fonts:** Geist (400, 500, 600) and JetBrains Mono (400, 500, 600, italic 400) are bundled (SIL OFL).
Settings → Appearance "UI font" and "Mono font" take any installed family name; empty means the bundled one.

## Type

| Token | Size | Used for |
|---|---|---|
| `--fs-title` | 17px | view titles, the sidebar's date heading, sheet titles |
| `--fs-body` | 13px (the UI size) | everything else, block and card titles included |
| `--fs-meta` | 12px | meta lines, labels, descriptions, counts, the status bar |
| `--fs-caption` | 11px | hour labels, key caps |

Times and durations are mono at `--data-font-size` (12px). Both sizes are settings: Appearance "UI size"
(13px, 10–24) and "Data size" (12px, 9–28); ⌘= and ⌘- zoom both, except in the Day view, where they zoom
the timeline. Chrome sizes are in rem of the UI size. Weights 400, 500 and 600. Numbers and times use
tabular figures. Durations read `1 h 05 min` (`formatDuration`), clock times `09:30` (`formatClock`).

## Spacing and radius

Spacing sits on a 4px grid (`--space-1` 4px to `--space-8` 32px, in rem so it zooms). Recording cards have an
8px radius and 4px by 8px padding; activity blocks a 6px radius and a 1px gap to their neighbours; sheets
24px padding and a 12px radius.

## Components

| Component | Where | Look |
|---|---|---|
| `Card` (`components/Card.tsx`) | a recording in the timesheet, the time column and the Week view | filled with `--block-fill`, text `--block-text`: the title (600), the subtitle (500), a foot with meta (the tag, with an icon) and the duration in mono on the right |
| `Block` (`components/Block.tsx`) | a summary block on the timeline, placed by time (activity lanes draw memories, see Timeline) | `--activity-fill`, 1px `--bg` border, `--radius-block`: the app (500, `--text-strong`), the title (`--text`), the duration trailing at the bottom right (mono, `--o2`), as on a card; one line when short |
| Buttons | `.btn` (secondary), `.btn-primary`, `.btn-danger`, `.icon-btn` | secondary: `--pop` with a `--field-line` border; primary and danger: the accent or `--style-red` with `--on-accent`; icon: a 2rem square |
| `SegmentedControl` (`components/SegmentedControl.tsx`) | Day mode, Settings choices | a `--track` with the pressed segment raised on `--raised` with `--shadow-small` |
| Popover | `Modal variant="popover"`: the date picker, the timeline settings | `--pop`, `--radius`, placed under its control, no scrim |
| Sheet | `Modal` (default `variant="sheet"`) | `--pop`, `--radius-lg`, 24px padding, over `--scrim` |
| Donut (`ReportChart` in `components/reports/`) | the Reports charts | an SVG ring of slices in accent colors on a `--band` track, percentages outside, the total in the center, a legend beside |

## Opacity levels

| Level | Token | Opacity | Used for |
|---|---|---|---|
| Primary | `--text` | 100% | the thing you came for |
| Secondary | `--o2` | 68% | labels, units, relative times, inactive items, meta (at least 4.5:1 on `--bg`) |
| Tertiary | `--o3` | 52% | counts, empty states, placeholders (at least 3:1 on `--bg`) |

## Rows

Every list shares one row: sidebar entries, palette results, settings lists. A 20px line with padding above
and below, a 1px `--line` hairline below, `--row-hover` on hover, `--sel` when selected. Sidebar entries have
2px more padding, an 8px inset and a 6px radius; the active one is `--sidebar-selected` with its label in
`--sidebar-strong` (500) and its icon in the accent.

Lists grouped under a heading (the clients on the Projects screen) use a section header band: `--band`
background, small semibold `--o2` text, 5px radius. Archived rows are dimmed (55% opacity) and keep a
Restore action. A client band and a project row start with a color dot (`AccentDot`, a project's in its shade);
a billable project carries a small `billable` badge after its name. The project sheet has a Billable switch
(a plain yes/no; there are no rates or amounts).
Colors are chosen with nine round swatches (`AccentSwatches` for a hue, `ShadeSwatches` for a shade), the
chosen one ringed. A tag is plain text, never a pill; a recording has at most one
([ADR 0019](adr/0019-one-tag-per-recording.md)).

## Inputs

Inputs look like inputs, never like rows.

- **Field:** `--field` background, 1px `--field-line` border, 6px radius, `--shadow-inset`, a leading icon,
  placeholder in `--o3`, optional key cap on the right (`InsetField` in `components/Field.tsx`). Plain
  `.input` controls (dialogs, Settings) share the frame without the icon.
- **Segmented control** (`SegmentedControl`, `.segmented`): a `--track` without a border; segments in `--o2`
  (500), hover primary text; the pressed segment is raised on `--raised` with `--shadow-small`, its label in
  `--text-strong`. Used for the Day mode switch, the report's Period and Settings choices.
- **Switch:** a track in `--field-line` with a `--knob` thumb; on, the track is the accent and the thumb
  `--on-accent`.
- **Key cap** (`KeyCap`): mono `--fs-caption`, `--o3`, 1px `--field-line` border, 4px radius. Key caps follow the
  user's bindings.

## Overlays

Palette, popovers, context menus, toasts and dialogs: `--pop` background, 1px `--field-line` border,
`--shadow` (a hairline ring and a soft drop). Menus, toasts and popovers take `--radius` (9px), sheets and
panels `--radius-lg` (12px). Sheets and the palette sit over `--scrim`; popovers (the date picker, the
timeline settings) sit over nothing. Overlays take
`Esc` first. Palette results use the row anatomy; the active one is `--sel` with a 2px accent bar on the left,
and command shortcuts show as key caps. Context menus use the UI font; the hovered or focused item is
`--accent` with `--on-accent` text, disabled items `--o3`, separators a 1px `--line`. The native WebKit
context menu is suppressed outside text fields (`lib/nativeMenu.ts`).

Pickers (project, tag) are a field with a list below it on `--pop`, using the palette's row anatomy and
small `--o3` section labels; the last option creates what was typed (`New project "x"…`). The tag picker
picks one tag; its first option is "No tag".

## Palette

⌘K is the fastest way anywhere. Empty, it lists Go to (the views with their keys, then Today, Yesterday,
This week, This month), Recent projects and Saved reports, and the footer names the prefixes. Typing
searches everything, ranked by match quality (whole name, then its start, then the start of a word, then
anywhere) and grouped in sections, the best section first:

- **Go to:** views, and dates: `today`, `yesterday`, `this week`, `last week`, `this month`, `last month`,
  weekday names (the latest such day, today included), month names (the latest such month), `12.10.`,
  `12.10.2026`, `2026-10-12`, `oct 12`, `12 october 2026` (without a year the latest such date). A day opens
  the Day view, a week the Week view, a month the Month view. The row shows the word and the date it means.
- **Settings:** `Settings: Capture` and every other page.
- **Projects:** ↵ opens Reports filtered to the project for the report's period; ⌘↵ starts a timer on it.
- **Clients:** ↵ opens Reports filtered to the client.
- **Saved reports:** ↵ opens one.
- **Commands:** the commands of the active scopes, with their keys.

Prefixes limit the list: `>` commands, `p:` projects, `c:` clients, `@` dates. ↑ ↓ move, ↵ opens, ⌘↵ runs
the row's second action where it has one (the footer names it), Esc closes. Section labels are small `--o3`
text; a row's date or detail follows its title in `--o2`.

## Timeline

The Day view reads like Timely's: a slim header, the timesheet on the left, then the timeline.

- **Header:** the date, the day's totals (`recorded 1:00 · tracked 6:12 · away 1:05`) only for a day other
  than today (the status bar has today's), the Timeline/List switch, Lanes (timeline only), the timeline
  settings gear and the ⋯ menu.
- **Timesheet** (left, about a quarter of the width, resizable with the splitter; kept as the `timesheet`
  pane size): "Timesheet", the day's recorded total large (`6h 05m`) and a + button (a new recording for the
  selection, else now; `recording.new`). Below, one `Card` per recording in the order of its first span on
  the day, filled in the project's shade: the note as title (the project without one), the project's dot and
  `Project | Client`, the tag (and `2 spans` when it has several on the day), the day's total at the bottom
  right (`1h 20m`). Longer recordings get taller cards (from about one line up to 300 minutes). A click or
  ↵ opens the editor in its place; hovering a card lights up its spans in the time column, and hovering a span
  lights up its card. The open editor takes its place among the cards (see [Recordings](#recordings)).
- **Timeline columns,** left to right: the hour axis (`--timeline-axis-w`, hour labels in `--fs-caption`
  `--o2`, tabular figures; hour hairlines in `--line`, half hours from 3 px per minute), the time column
  (`--recordings-col-w`, about 90 px: recordings and suggestions, where a drag selects time), the summary
  lane, then the activity lanes: Activity, an app's own lane, an external source's lane. Lane heads stick to
  the top with an icon (the app's for an app lane) and the name; the Activity head shows the day's tracked
  time. Lanes take shares of the width by kind (summary 1.2, activity 2, app 0.6, source 1) with minimum
  widths (`--summary-lane-min-w`, `--activity-lane-min-w`, `--app-lane-min-w`, `--lane-min-w`).
- **Summary lane** works in clock-aligned buckets of the Summary interval (default 5 min): each bucket shows
  the app (or window, without grouping by app) with the most time in it, neighbouring buckets with the same
  winner merge, empty buckets stay empty, so a summary block always starts and ends on the grid (:45, :50,
  never :49). Blocks (`Block`, `summary-block`) are filled with the app icon and label, the active time at
  the bottom right when tall enough.
- **Memories** (the activity, app and source lanes): each block is the app icon over a thin vertical bar as
  long as the activity really is, the label (500, `--text-strong`) and its duration (`34m`, `--o3`) beside it,
  on two lines when there is room, else on one; text may run past a short block, the bar keeps the real
  length; tiny ones show only the bar. Private blocks read "Private" in `--o2`. Overlapping blocks of one lane
  share its width side by side.
- **Focus** (J/K/H/L or a click) is a 2px accent outline; Space opens the details card for it.
- **Hover card** after 300 ms on `--pop` with `--shadow`: app, full title, URL or path (selectable; an
  http(s) URL opens in the browser), `09:12–09:40 · 28 min`, and the source when external.
- **Away spans and gaps** are darker bands across the axis, the time column and the lanes, behind the
  blocks: away in `--away-band` (darker than `--bg`), reading "Away · idle 25 min" in `--o2` when tall
  enough; gaps (2 min or more between the day's first and last activity) in a lighter mix of it, unlabeled.
- **Now** is a 1px accent line on today, updated every 30 s.
- **Empty day:** "Nothing tracked on this day" with the tracker's status line; without the Accessibility
  permission "Tracking needs permission" with Allow…. The timesheet stays beside it.
- **Zoom** is 1–8 px per minute (default 2.5), kept in `ui_state.json` as `timelineZoom`: a floating control
  at the timeline's bottom right (−, a slider, +) and ⌘= / ⌘-.
- **Lanes** (header): a popover with Summary and Activity switches and a switch per app of the day (most
  time first) that gives it a lane of its own (`ownLane`).
- **Compact** draws the day at 60% of the zoom, every block on one line.
- **Block gear:** a hovered block shows a gear at its top right; it opens the same menu as a right-click.

### Timeline settings

Settings → Timeline and the Day header's gear (both save at once; the Day view reloads):

| Setting | Values | Does |
|---|---|---|
| Summary lane | on (5 min) | a lane with the main app of every 5, 10, 15, 20, 30, 45 or 60 minutes, in buckets on the clock |
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

- **Recording by selection** (as in Timely; no key needed). Drag over empty time in the time column, the
  axis or the lanes, or click an activity block: the editor opens on release. ⇧-click grows the selection over
  another block first. ⌘-click adds or removes blocks without opening the editor; it opens when ⌘ is
  released, so several scattered blocks can be gathered. While the editor is open, a new drag or click moves a
  new recording to the new span (note, project and tag stay), and ⌥-drag or ⌥-click adds the span to the
  open recording instead (without an open editor, ⌥ on a block still drags it onto a project for a rule).
- **Snapping** (the Time snapping setting, 5 min by default) applies to every selection, drag and resize,
  on both ends, and Rust computes it: a selection over activity snaps each end to the nearest mark (blocks
  from 13:12 to 13:38 give 13:10–13:40); dragging an edge of a selection or a recording, or dragging in the
  time column, moves that edge in whole steps (13:40, 13:45, …); an end that would reach the other keeps one
  step. ⌥⇧↑ ⌥⇧↓ on a recording move its end the same way.
- **Selection** (the keyboard's way: V on the focused block, ⇧J and ⇧K add the next or previous one; ↵ opens
  the editor): `--sel` fill with accent hairlines and the snapped `09:00–10:30` in accent text; the status
  bar repeats it with the duration. A dashed accent outline in the time column shows where the recording goes.
- **Recording blocks** sit in the time column, one per span: a recording can hold several separate
  spans (14:25–15:00 and 16:05–20:15), each its own block, all lit up together with the recording's single
  timesheet card when one is hovered or focused; project, note and tag belong to the recording and show on
  the card. In the Day view a span shows only its start (top left) and end (bottom right); the Week view
  shows the full card. In the Day view a click on a span opens the editor; in the Week view a click focuses
  and a double-click edits. Each is a `Card` filled in the project's shade (`--block-fill`, light in light palettes, a muted dark tint
  in dark ones) with `--block-text`, at least 4.5:1 for every accent and shade: the project name (600), the
  note (500), then the tag with a tag icon and the duration in mono at the bottom right. A dashed top or
  bottom edge means the span continues on another day. The top and bottom 6px resize the span; the body
  moves it. Activity under recordings (half or more) takes `--activity-fill-dim` (summary) or a regular-weight
  `--o2` label (memories); hover lifts it.
- **Editor**, inline in the timesheet (Timely's): a card with a 2px accent ring at the new recording's place
  among the cards (in place of the card it edits), pushing the cards below down and scrolled into view; it is
  never a sheet over the timeline (the Week view, without a timesheet, shows it as a sheet). Top to bottom: the
  note as the title field (focused, "What did you work on?"), project (recent first, type to search, "New
  project…"), tag, the duration large as `0 h 20 m` (both numbers editable; a change moves the last span's
  end), "Activity n" (the activity blocks inside the spans; a click lists them), the spans as `FROM 10:00 TO
  10:20` rows (✕ removes one while there are several; "Add span" adds one after the last), then Save, Cancel
  and a trash icon for Delete. The draft's spans show in the time column as accent-filled blocks with their
  times. A new recording's project is proposed: the project of the latest recording (two weeks back) that
  covered the same main app, else the rule matching most of the activity. Field errors show under their
  field. A running recording's spans are shown, not edited.
- **Overlap**: asked inside the editor (outside it, a confirm sheet) listing the recordings in the way, each
  with its spans; "Replace overlapped part" trims or cuts their spans, and deletes a recording left without
  any; Back returns to the fields.
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
  project (the last used one), From and To (snapped), note, tag, and Record (↵) / Skip (Escape).
- A recording in the way reads `A recording already covers 12:10–12:30 (Project)` above the buttons; the
  form stays, so the times can change. The sheet stays open when another app takes focus.

## List view

- ⌘⇧L or Day mode → List shows the day as rows grouped like the timeline, longest first: app in `--o2`, the
  label, URL or path below, the duration right with a thin covered bar (the accent over `--line`) and
  `· 3 spans` when the activity came in parts. The timesheet with its cards stays on the left, as in the
  timeline.
- A strip of project chips (recent first, More… for the rest), each filled like a recording card, sits above
  the rows. Drag a row onto a chip
  to record it: the editor opens with the row's first start to last end and, for several spans, the warning
  `This activity has gaps (3 spans, 0:40 in between). Adjust the times if needed.`
- Keyboard: `/` search, ↑↓ rows, ↵ record (pick the project), Space show on the timeline.

## Rules and suggestions

- **Suggestions** sit in the time column under the recordings: a card outlined dashed in
  `--block-meta` on a half-strength `--block-fill` (full once hovered), a "+" top right, the project (500)
  and the rule's name, the duration bottom right. The cursor is `copy`. A click records it; ⌥-click records every suggestion of that rule on the
  day. Each is a button, so Tab reaches it, ↵ accepts it and ⌥↵ every suggestion of its rule. The tooltip names the rule, project, span and note.
- **Drop strip**: ⌥-dragging an activity block (while no recording editor is open) shows the recent projects on `--pop` over the top of the
  recordings column; the one under the pointer gets a dashed accent outline. Dropping opens the rule sheet,
  filled in; nothing is saved before Save. From the keyboard, the block menu's Make a rule… (⌃↵) opens the
  sheet with the block's conditions and no project.
- **Rules screen**: rows with a grip, the enable switch, the name, the project's dot and name and the
  conditions in `--o2`; a disabled rule is dimmed. The order is the priority; drag the grip, or ⌥↑ ⌥↓ with the focus in a row.
- **Rule sheet**: the form on the left, "Last 7 days" on the right with the matching time and the hits
  (day and time in mono `--o2`, the app, the title). Below 720px the preview moves under the form.

## Timer

- A running recording's open span grows to now in the Day view, with a pulsing accent dot at its top right
  (still with reduced motion); it has no grips. Starting the timer for a project, note and tag already
  recorded today continues that recording with a new span. The menu bar item's title reads `⏱ 1:23 Acme` while it runs, and the
  status bar shows `⏱ Acme 1:23` (a click stops it).
- The picker (⌘⇧T from any app, ⌘T in the window, Start Timer… in the tray) is a small floating panel on
  `--pop` under the tray icon: project, note, tag, Start (↵), Cancel (Escape), and the recent timers below.
  While a timer runs it shows the timer with Stop and "Start another…". It hides when it loses focus.

## Week and Month

- **Week**: the time axis and seven day columns (`--lane-min-w` like lanes); each head shows `Mon 20` and the
  day's recorded hours, today's in accent text; today's column has a faint tint. Recordings drag as in the
  Day view; a span dropped on another column moves there, with ⌥ it is copied as a new recording. A double-click on
  an empty spot records an hour there, in the editor as a sheet. The header totals read `recorded 32:10 · tracked 41:05`.
- **Month**: a calendar of whole weeks; each day a cell with its number (`--o3` outside the month, accent
  on today), the recorded hours in mono, and a 3 px strip along the bottom of the three projects with the
  most time, as wide as their shares. A click opens the day.

## Reports

The Reports screen follows Timely's day report, without money. The header has the period as title, then
the grouping of the copied text (By project, tag, day, week or month), Copy as text and Save report…; below,
one slim filter bar (Period presets as a segmented control, Custom revealing dates, clients, projects, tags, rounding, archived projects). The
sidebar's navigator steps the period. The content sits on `--pop` cards with `--radius-lg`:

1. **Totals:** `Total` with the hours large (`--text-strong`, 1.75 × `--fs-title`), then Recordings,
   Projects, Untagged (only when there is untagged time) and `Billable 6:24 h · 80 %` (of the rounded total;
   only when time is recorded), each a `--o2` label over its value. With rounding, the unrounded total stands
   below in `--o3`.
2. **Charts:** two donuts side by side, By client and By tag; a By select on each switches between client,
   project and tag. The ring has the slices in their colors (a client's accent, a project's shade, a tag's
   accent by its place in the tag list, graphite for No client and Untagged) on a `--band` track, the
   percentages outside the ring in `--o2` (from 3 %), the total and the count (`2 clients`) in the center,
   and a legend with a dot, the name and the hours.
3. **Table:** client rows on `--band` with their hours; project rows with a color dot, the name, the
   `billable` badge, the number of recordings, a **Copy notes** button and the hours; a click on a project
   unfolds its recordings (Expand all / Collapse all for every project). A recording row shows its day and
   all its spans of the day (`Wed, Oct 21 · 14:25–15:00, 16:05–20:15`), the note and the tag below. Copy
   notes copies the project's notes for the period, by first span, one per line (empty notes skipped,
   repeated notes kept, a recording's note once) and toasts `Copied 4 notes for Northwind / Support`; it is
   disabled when the project has no notes. Copy Report Notes (⌥⌘C) copies every note of the report the same
   way.

No recordings: the totals at zero and `No recordings.` Saved reports sit under Reports in the sidebar.

The copied text, also what `timetracker report` prints (the CLI adds `Billable: 3:30 of 4:45 (74 %)` below
the total; the copied text leaves it out):

```text
2026-10-19 – 2026-10-25 · rounded to 15 min

Northwind / Website relaunch  3:30 h
  - fix login (Mon, Wed)
  - review (Fri)

Internal  1:15 h

Total  4:45 h
```

Notes carry their weekdays when the range is longer than a day (dates beyond a week), except by day.
Grouped by tag, each recording counts once and recordings without a tag form `Untagged`, last. No
recordings: the header, then `No recordings.`

## Data folder

Settings → Data → Move… opens the macOS folder panel (it can create a folder), then a sheet: the database is
copied there and used from then on, tracking pauses while it copies, and the old files stay where they were
(both paths shown, mono, selectable). Move is the default button; while the copy runs it reads `Moving…`,
both buttons are disabled and Escape does nothing. Success is a toast naming the new and the old folder. A
folder TimeTracker refuses (inside the current folder, already holding data, not writable) is explained in red
under the Location row instead of a toast.

## Keys

Every action is a command (`APP_COMMANDS` in `src/commands/index.ts`) with a rebindable key (Settings →
Shortcuts, `keybindings.json`) or, without a default key, an item in the [menu bar](#menu-bar); every
command is in the palette too (⌘K, `>` lists the commands of the active scopes; see [Palette](#palette)). `?` lists the bound commands of the scopes active when it opens, most specific first.

Rules of the keys (`handleKeydown`):

- Bare keys never fire in text fields; there only ⌘ and ⌃ combinations and Escape do.
- While a dialog, the palette, the recording editor or a context menu is open, only Settings, `?`, the
  palette, zoom, Close window and Quit run; every other key belongs to the overlay. Escape closes the
  recording editor wherever the focus is.
- ↵ and Space on a focused button or link activate it. J, K, H, L, V, ⇧J, ⇧K, ] and [ give the keys back to
  the timeline: they take the focus off a Tabbed-to button.
- A command has one binding. P, N and O replaced ⌘←, ⌘→ and T for the dates; ] and [ replaced N and ⇧N for
  the recordings; T is free.
- `Tab` is never bound: it moves the keyboard focus. Dialogs keep it inside (`Modal`), in reading order:
  fields first, then Cancel and the default button; the first control, or the one a dialog marks, has the
  focus, and closing returns it to where it was.

Scopes are the registry's (`src/commands/scopes.ts`): `global`; `dated` under it, active in Day, Week, Month
and Reports; and one scope per view (`day`, `week`, `month`, `reports` under `dated`; `projects`, `rules`
under `global`), active while that view shows.

| Command | Label | Default key | Scope |
|---|---|---|---|
| `settings.toggle` | Settings… | ⌘, | global |
| `app.launchAtLogin` | Launch at login (a check item) | menu | global |
| `welcome.show` | Show welcome again | menu | global |
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
| `edit.undo` / `edit.redo` | Undo / Redo: the text field's typing, else the last recording change (not while an overlay is open) | ⌘Z / ⌘⇧Z | global |
| `logs.open` | Open log folder | menu | global |
| `data.reveal` | Show data folder (the database file in Finder) | menu | global |
| `data.exportBackup` | Export backup… | menu | global |
| `help.readme` | Open README | menu | global |
| `recording.new` | New recording… (the Day view's selection, else the half hour up to now on today) | menu | global |
| `app.quit` | Quit TimeTracker | ⌘Q | global |
| `nav.prev` / `nav.next` | Previous / Next day, week, month or report period | P / N | dated |
| `nav.today` | Go to today (this week, this month, the report's period that holds today) | O | dated |
| `nav.goToDate` | Go to date… (the date picker) | menu | dated |
| `day.nextBlock` / `day.prevBlock` | Next / Previous block in time | J / K | day |
| `day.prevLane` / `day.nextLane` | Block in the lane to the left / right | H / L | day |
| `day.details` | Block details | Space | day |
| `day.blockMenu` | Block actions… (app settings, copy, Make a rule…) | ⌃↵ | day |
| `day.selectBlock` | Select block (the focused one, as a click does) | V | day |
| `day.extendNext` / `day.extendPrev` | Extend selection to the next / previous block (as a ⇧-click does) | ⇧J / ⇧K | day |
| `day.toggleList` | Toggle timeline and list | ⌘⇧L | day |
| `activity.deleteDay` | Delete activity for this day… | menu | day |
| `activity.deleteRange` | Delete activity in a range… | menu | day |
| `day.settings` | Timeline settings | menu | day |
| `day.clearFocus` | Clear selection (and the block and recording focus) | Escape | day |
| `recording.next` / `recording.prev` | Next / Previous recording | ] / [ | day |
| `recording.open` | Record selection or edit recording (opens the editor) | ↵ | day |
| `recording.create` | Record selection | menu (shows ↵) | day |
| `recording.edit` | Edit recording | menu | day |
| `recording.delete` | Delete recording (Undo in the toast) | ⌫ | day |
| `recording.nudgeEarlier` / `recording.nudgeLater` | Move recording earlier / later by one snap step | ⌥↑ / ⌥↓ | day |
| `recording.extend` / `recording.shrink` | Extend / Shorten recording by one snap step | ⌥⇧↓ / ⌥⇧↑ | day |
| `recording.moveToPrevDay` / `recording.moveToNextDay` | Move recording to the previous / next day | ⌥← / ⌥→ | day |
| `recording.copyToPrevDay` / `recording.copyToNextDay` | Copy recording to the previous / next day | ⌥⇧← / ⌥⇧→ | day |
| `reports.copyText` | Copy report as text | ⌘⇧C | reports |
| `reports.copyNotes` | Copy report notes (every note of the report, one per line) | ⌥⌘C | reports |
| `reports.save` | Save report… | palette | reports |
| `reports.rename` / `reports.delete` | Rename / Delete saved report… (the one opened from the sidebar, until the filter changes) | palette | reports |
| `projects.new` | New project… | ⌘N | projects |
| `clients.new` | New client… | menu | projects |
| `rules.new` | New rule… | ⌘N | rules |
| `rules.moveUp` / `rules.moveDown` | Move rule up / down (the rule whose row has the focus) | ⌥↑ / ⌥↓ | rules |
| `suggestion.accept` / `suggestion.acceptRule` | Accept suggestion / all suggestions of its rule (the suggestion with the focus) | menu | day |

Keys of controls, not commands (they act on the focused control and are not rebindable):

| Where | Keys |
|---|---|
| List mode | `/` search, ↑↓ rows, ↵ record the row (pick the project), Space show it on the timeline; Tab to a project chip and ↵ records the focused row |
| Suggestion (Tab reaches it) | ↵ records it, ⌥↵ records every suggestion of its rule on the day |
| Recording editor, away prompt, timer picker | project picker: type, ↑↓, ↵ picks (the last option creates a project); From and To: type `HH:MM` or ↑↓ by one snap step; ↵ saves, records or starts; in the editor also ⌘↵, and ⌘⌫ deletes; Escape cancels or skips |
| Timeline, pointer | drag or click opens the editor; ⇧-click extends, ⌘-click gathers blocks (the editor opens when ⌘ is released), ⌥-drag or ⌥-click adds a span to the open editor's recording |
| Context menu (⌃↵ on a block) | ↑↓ items, ↵ runs, Escape closes |
| Pickers in the Reports filter | type, ↑↓, ↵ adds, ⌫ on an empty field removes the last |
| Week and Month | Tab to a day head (Week) or a day cell (Month), ↵ opens that day |
| Date picker (the sidebar's date heading) | ←→ by a day, ↑↓ by a week, PageUp / PageDown by a month, ↵ shows the day, Escape closes |
| Settings → Shortcuts | a key field records the next key; Escape stops, ⌫ unbinds |

Left to the pointer, each with a keyboard way to the same result: dragging a selection (V, ⇧J, ⇧K, or the
List mode), ⌥-adding a span to the open recording ("Add span" and its From / To fields), dragging or resizing a recording (⌥↑ ⌥↓ ⌥⇧↑ ⌥⇧↓), dragging a recording to another day in the
Week view (⌥← ⌥→, ⌥⇧ to copy, in the Day view), double-clicking the Week view (select in the Day view and ↵),
⌥-dragging a block onto a project (⌃↵ → Make a rule…), the rules' grip (⌥↑ ⌥↓), the hover card (Space), the
saved reports' right-click menu (`reports.rename`, `reports.delete`), and the Projects screen's right-click
menu (Edit and Archive are in the project sheet). Only the right-click "Copy id" of a project has no key; the
CLI and the MCP server list the ids.

### Record a day

From any view, with the documented default keys only:

1. ⌘1 shows the Day view, O today.
2. J focuses the first block of the day (K, H and L move on); Space shows its details.
3. V selects the focused block's span; ⇧J (or ⇧K) grows the selection over the next (or previous) block, as
   far as the work went. The status bar shows the span and its length; Escape clears it.
4. ↵ opens the editor in the timesheet for the selection, with the note focused: type it, then Tab to the
   project (often proposed already): type part of the name, ↑↓, ↵ picks it (or the last option,
   `New project "x"…`, opens the project sheet). Tab on to the tag and the duration.
5. Tab to the span's From and To: type `09:15` or press ↑↓ to move by one snap step; "Add span" adds another.
6. ↵ in a field or ⌘↵ saves. If it overlaps a recording, the editor lists them: ↵ on "Replace overlapped
   part" replaces it, Back returns to the fields; Escape cancels the editor.
7. The new recording has the focus: ⌥↑ ⌥↓ move it, ⌥⇧↓ ⌥⇧↑ change its end, ↵ edits it again, ⌫ deletes it,
   ⌘Z undoes the last change. ] and [ step through the day's recordings.
8. Back to step 2 for the next stretch (J moves on from the last focused block). Rule suggestions are
   buttons in the recordings column: Tab to one, ↵ records it.
9. P and N step to other days. ⌘2 shows the week (P N by week; Tab to a day head and ↵ opens it), ⌘3
   the month (by month; Tab to a day and ↵ opens it).
10. ⌘4 shows Reports (P N step its period, O goes back to the current one): Tab to the Period presets, on
    to the client, project and tag pickers and Rounding, then the charts' By selects and the table's project
    rows (↵ unfolds one) and Copy notes buttons; ⌘⇧C copies the report as text, ⌥⌘C all its notes, ⌘K
    `> save` saves it under a name.

## Menu bar

Every view and nearly every command has a menu item, with its key shown (`src-tauri/src/menu/`, synced as
in [backend.md](backend.md#native-menu)). An item whose command cannot run now is greyed out: outside its
view's scope, when its `when` is false (no selection, no focused recording, no running timer, no focused
suggestion), and while a dialog, the palette or the editor is open (except Settings, Keyboard Shortcuts,
Command Palette, zoom, Close Window, Undo, Redo and Quit). Rebinding a key in Settings → Shortcuts updates
the menu.

Keys with ⌘ or ⌃ are real menu accelerators. Single keys and ⌥ keys (P, N, O, J, K, ], [, ⌫, ↵, ⇧?, ⌥←) are
shown in the item's title, `Previous (P)`, and are never accelerators: an accelerator would take the key
from every text field. The page's key handler runs them, with the rules above.

| Menu | Items (command; key) |
|---|---|
| TimeTracker | About; Settings… (`settings.toggle`; ⌘,); Launch at Login (`app.launchAtLogin`, a check item); Show Welcome Again (`welcome.show`); Services; Hide, Hide Others, Show All; Quit TimeTracker (`app.quit`; ⌘Q) |
| File | New Recording… (`recording.new`); Start Timer… (`timer.start`; ⌘T), Stop Timer (`timer.stop`; ⌘⇧.); New Project… (`projects.new`; ⌘N), New Client… (`clients.new`), New Rule… (`rules.new`; ⌘N); Export Backup… (`data.exportBackup`); Close Window (`window.hide`; ⌘W) |
| Edit | Undo (`edit.undo`; ⌘Z), Redo (`edit.redo`; ⌘⇧Z); Cut, Copy, Paste, Select All (native); Delete Recording (`recording.delete`; ⌫); Copy Report as Text (`reports.copyText`; ⌘⇧C), Copy Report Notes (`reports.copyNotes`; ⌥⌘C) |
| View | Day … Rules (`view.day` … `view.rules`; ⌘1 … ⌘6, the active view checked); Show as List (`day.toggleList`; ⌘⇧L, checked in List mode); Toggle Sidebar (`sidebar.toggle`; ⌘⌃S); Zoom In, Zoom Out, Reset Zoom (`zoom.*`; ⌘= ⌘- ⌘0); Timeline Settings… (`day.settings`); Enter Full Screen (native) |
| Go | Previous (`nav.prev`; P), Next (`nav.next`; N), Today (`nav.today`; O), Go to Date… (`nav.goToDate`); Next Block (`day.nextBlock`; J), Previous Block (`day.prevBlock`; K); Next Recording (`recording.next`; ]), Previous Recording (`recording.prev`; [) |
| Day | Record Selection (`recording.create`; ↵, the key of `recording.open`), Edit Recording… (`recording.edit`); Move Recording to Previous / Next Day (⌥← / ⌥→), Copy Recording to Previous / Next Day (⌥⇧← / ⌥⇧→); Accept Suggestion, Accept All Suggestions of Rule (`suggestion.*`); Delete Activity for This Day…, Delete Activity in a Range… (`activity.*`) |
| Window | Minimize, Zoom, Bring All to Front (native) |
| Help | Keyboard Shortcuts (`shortcuts.show`; ⇧?), Command Palette… (`palette.toggle`; ⌘K); Open Log Folder (`logs.open`), Show Data Folder (`data.reveal`), README (`help.readme`) |

New Project and New Rule share ⌘N: only one of them is enabled at a time (Projects or Rules).

Commands left out of the menu, with keys and the palette only:

| Commands | Why |
|---|---|
| `day.prevLane` / `day.nextLane`, `day.details`, `day.blockMenu`, `day.selectBlock`, `day.extendNext` / `day.extendPrev`, `day.clearFocus` | step-by-step keyboard work on the focused block; the Go menu has the block steps J and K |
| `recording.open` | the Day menu has its two halves, Record Selection and Edit Recording… |
| `recording.nudgeEarlier` / `nudgeLater` / `extend` / `shrink` | one snap step per press, held to repeat; a menu click per 5 minutes helps no one |
| `reports.save`, `reports.rename`, `reports.delete` | saved reports live in the sidebar, whose right-click menu renames and deletes them |
| `rules.moveUp` / `rules.moveDown` | act on the focused rule row, next to its drag grip |
| `tracker.pause`, `tracker.resume` | the menu bar item (tray) has Pause Tracking with its durations and Resume |
| `timer.toggle` | File has Start Timer… and Stop Timer |

## Scheme audit

The three schemes in both modes are checked on the Day view, List view, Week, Reports and Settings
(`docs/screens/<scheme>-<mode>-<view>.png`, captured by `scripts/dev/screenshot-looks.sh <dir> [view]`, which
switches all six combinations through `scripts/dev/js.sh` and restores the settings; only the Default scheme's
light and dark Day view are committed). The six combinations give three distinct looks (ADR 0021). Fixes
live in `src/styles/palettes/` only.

Targets, computed by `src/styles/contrast.ts` (WCAG contrast; OKLCH as the CSS computes it):
- `--text` at least 7:1, `--o2` at least 4.5:1 and `--o3` at least 3:1 on `--bg`; `--line` at least 1.3:1
  against `--bg`.
- `--on-accent` at least 4.5:1 on every accent; each accent at least 3:1 on `--bg`.
- A filled recording, for every accent and all nine shades: `--block-text` at least 4.5:1 on `--block-fill`,
  its foot (`--block-meta`) too.
- Sidebar: `--sidebar-text` and `--sidebar-muted` at least 4.5:1 on `--sidebar-bg`; every accent of the sidebar's tone
  at least 3:1 on it.

Measured: light content block text 8.5:1 at worst (foot 4.6), dark content 8.3 (foot 5.4); `--o3` 3.2 (light)
and 4.7 (dark); the weakest on-accent pair 4.65 (gold, light). The List view's covered bar fills with the
accent.
