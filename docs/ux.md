# Design language

The reference for how TimeTracker looks and behaves. Update this page with every change that builds or
changes an item. Decisions: [ADR 0016](adr/0016-color-schemes.md) (token sets, no literals),
[ADR 0021](adr/0021-appearance-modes-and-schemes.md) (schemes and palettes),
[ADR 0022](adr/0022-a-scheme-per-system-mode.md) (a scheme per macOS mode),
[ADR 0024](adr/0024-native-macos-look.md) (the native look),
[ADR 0018](adr/0018-one-line-notes-tags-copyable-reports.md) (notes, tags, reports). Implementation (files,
registries, CSS structure): [frontend.md](frontend.md#styling).

## Principles

- It looks like a calendar (Fantastical, Timely), not a code editor: proportional type, filled colored
  blocks, cards, generous spacing, a dark sidebar by default.
- Logging a day feels like arranging blocks, not filling a form. The Day view is the product; automation
  only suggests.
- Fully keyboard-usable, light and dark. Every action is a command with a rebindable shortcut and a palette
  entry.
- The system font (SF Pro) for all text, titles included, at the macOS text styles; numbers in tabular
  figures; mono (SF Mono) only for machine text such as paths, URLs, ids and commands.
- Native: it reads as a macOS app beside Calendar and System Settings (unified toolbar, inset groups,
  AppKit-sized controls; [ADR 0024](adr/0024-native-macos-look.md)), never as a web page.
- Hierarchy comes from weight, opacity and space: a strong title, then a normal subtitle, then meta in
  `--o2`. Recordings are filled cards in their project's color; activity is a soft grey surface.
- The accent marks interactive things only: keys, links, focus, selection. A client's color is one of the
  nine accent hues (`--a-<name>`); its projects are shades of it: the hue and chroma of the accent at one of
  nine lightness steps (`shadeColor` in `components/Accent.tsx`, OKLCH relative colors).
- Native and light: the tracker is never noticed; the window opens instantly.

## Surfaces

The window is a native macOS window ([ADR 0024](adr/0024-native-macos-look.md)): neutral greys, no tinted
page, no outlined cards.

| Token | Used for |
|---|---|
| `--sidebar-bg` | the sidebar, full height, under the window buttons; the window's sidebar material instead where the scheme's sidebar tone matches the macOS mode |
| `--bg` | content: the window background under the toolbar and the view |
| `--bg-quiet` | a pane beside the active one (the timesheet), weekend columns |
| `--chrome` | the status bar, the Settings page list |
| `--pop` | overlays: palette, popovers, context menus, dialogs, toasts |
| `--fill-quiet` | inset groups (System Settings style): Settings groups, the Projects and Rules lists, the Reports cards |

Separators are `--line` hairlines (`--sidebar-line` beside the sidebar). Bars inside content stay on `--bg`.
Grouped content is an **inset group**: `--fill-quiet`, `--radius` (10px), no border, rows divided by
`--line` hairlines inside the group's 12px padding, no hover fill on plain rows; its heading (13px semibold
`--text`, or a name with a dot) sits above it, outside the fill. In the Default scheme the dark sidebar is
opaque; in the Light scheme in light mode and the Dark scheme in dark mode the sidebar is transparent, so the
window's `sidebar` material (vibrancy) shows through, and it greys out while the window is inactive.

## Shell

One place for views and one place for dates: the sidebar.

- **Sidebar** (`Sidebar`): full height on its own palette. On top a drag strip under the window buttons, then:
  - **Month calendar** (`MiniCalendar`), the Day, Week and Month navigation: the month and year as the title
    (`--fs-heading`, bold, the year in `--accent-text`; `Nov 2026`, or `November 2026` with Settings →
    Appearance → Calendar title on Full), ‹ › to page months, a week-number column (`Wk`, ISO
    numbers, a hairline beside it) and weekday initials from Monday. Six weeks always, so the sidebar never
    changes height; days outside the month are dimmed, today sits in an accent circle with `--on-accent`
    text, and a small dot under a day means it has recordings. It shows the month of the shown date (the
    report's first day for a custom report, else today); ‹ › page it until the shown date changes. The shown range is highlighted: the day on `--sidebar-selected` (Day view), the whole week row as a
    rounded `--sidebar-selected` band with its number in the accent (Week view), or the title (Month view);
    Reports highlight their period the same way when it is one day, week or month. A click on a day shows it
    in the Day view, on a week number that week in the Week view, on the title that month in the Month view.
    The cells are for the pointer; the keys are ⌘1 to ⌘3, P, N, O and Go to Date… (`nav.goToDate`), whose
    picker opens under the title.
  - **Hours** (clock): today in the Day view, ready to record today; selected while the Day view shows today.
  - **Reports** (⌘4) with the saved reports under it.
  - A separator, then the configuration pages: **Projects** (⌘5), **Tags** (⌘6), **Rules** (⌘7).
  - **At the bottom** (`SidebarStatus`): the tracker's status in a rounded `--sidebar-hover` field with a
    `--sidebar-line` ring (the dot as in the status bar, the tray's text, `Tracking · Code`) and a play / pause
    button: Pause pauses until resumed (a right-click offers 15 minutes, 1 hour, until tomorrow), Play
    resumes, Allow… without permission. The Settings gear sits beside it.
- **Unified toolbar** (`Toolbar`): every view starts with one 52px row beside the window buttons (which sit
  centered in it, 20px from the left): the view's title (`--fs-title`, bold, `--text-strong`), its detail
  (`--fs-meta`, `--o2`, e.g. `recorded 1:00 · tracked 4:12`), then its actions at the trailing end, with a
  hairline below. Empty parts drag the window. There is no app title and no Settings button (⌘, the app
  menu and the palette open Settings). Without the sidebar the row leaves `--traffic-lights` for the window
  buttons. Titles, totals and view switches live here, never date controls; the Day toolbar shows the
  shown day's totals only when it is not today.
- **Status bar** (plain `--fs-meta` text in `--o2` on `--chrome`, beside the sidebar): on the left the sidebar
  toggle, the tracker's state only while the sidebar is hidden (a dot: accent while running, `--o3` while away or paused, `--style-red`
  without permission; the tray's text; Pause, Resume or Allow…) and the selection; on the right the running
  timer, the billable ratio and today's totals, `recorded 1:00 · tracked 3:12`, in every view. The billable
  ratio is a small bar (a neutral track, filled in the accent; no red or green judgment) with
  `Billable 80 %`, and `Billable 6:24 / 8:00` in its tooltip, for the shown period (day, week,
  month, report period; today on Projects, Tags and Rules). It hides while nothing is recorded in that period. The
  text zoom has no readout: ⌘= ⌘- ⌘0.

## Appearance and tokens

The app always follows the macOS appearance; there is no app mode. Settings → Appearance picks a **scheme
per macOS mode**: a Light | Dark segmented control ("Pick a scheme for each macOS mode."), starting on the
current mode, switches which mode's three scheme tiles show below it, and a tile sets that mode's scheme
(`lightScheme`, `darkScheme`). The look is the scheme of the current mode, so dark mode with the Light scheme
is all light ([ADR 0022](adr/0022-a-scheme-per-system-mode.md)). The accent is one setting for both modes.

| Scheme | Sidebar | Content | Default for |
|---|---|---|---|
| Default | dark (opaque) | light | light mode |
| Light | light (vibrant in light mode) | light | |
| Dark | dark (a shade lighter than the content; vibrant in dark mode) | dark | dark mode |

`resolveLook` (`lib/theme.ts`) picks the current mode's scheme, `schemeLook` turns it into a content tone and
a sidebar tone, and `applyAppearance` sets `data-content` and `data-sidebar` on `<html>` (Settings' scheme
tiles set them on their own element). Four palettes in `src/styles/palettes/` answer them, neutral macOS
greys: `content-light`, `content-dark`, `sidebar-light`, `sidebar-dark`. Palettes of a kind define the same
tokens; no color literal appears outside them (`src/styles/colors.test.ts`).

**Content palette** tokens:

| Token | Light | Dark | Role |
|---|---|---|---|
| `--bg` | `#ffffff` | `#1e1e1e` | content surface, the window background |
| `--bg-quiet` | `#f7f7f7` | `#222222` | a pane beside the active one, weekends |
| `--chrome` | `#f5f5f5` | `#252525` | status bar, Settings page list |
| `--pop` | `#ffffff` | `#2c2c2c` | overlays |
| `--text`, `--text-strong` | `#262626`, black | `#e6e6e6`, white | primary text (`--o2`, `--o3` derive from it); titles and pressed segments |
| `--fill-quiet`, `--fill`, `--fill-strong` | black at 3.5, 5.5 and 10% | white at 4.5, 8 and 14% | translucent fills over any surface: inset groups; hover bezels, the segmented track, search fields, key caps; pressed bezels, the switch track |
| `--line` | black at 12% | white at 10% | hairlines |
| `--control`, `--control-pressed` | white, `#ebebeb` | `#4a4a4a`, `#5c5c5c` | push button and popup button bezel; pressed |
| `--control-shadow` | a hairline ring and a 1px drop | a top highlight, a dark ring | the push button bezel's edge |
| `--field`, `--field-line` | white, black at 16% | white at 5%, white at 16% | text field fill and its 1px inset frame |
| `--raised`, `--knob` | white, white | `#5a5a5a`, white | the pressed segment; the switch thumb |
| `--shadow`, `--shadow-small` | | | overlays (a hairline ring and a soft drop; no border); the raised segment and the switch thumb |
| `--scrim`, `--shade` | black 14% | black 35% | behind sheets and panels; black for `color-mix` darkening |
| `--a-<name>`, `--accent-text-max-l` | deep hues, `0.56` | light hues, `1` | the nine accent hues (`ACCENTS`): app accent and project colors; the highest OKLCH lightness of `--accent-text`. They also apply inside a sidebar of the same tone |
| `--on-accent` | white | `#1a1a1a` | text on any palette accent's fill, at least 4.5:1 on every hue; like the hues, it follows the sidebar's tone inside the sidebar |
| `--on-system-accent` | white | white | text on the System accent, as macOS draws it |
| `--style-green` | | | the success tint; red, yellow, blue and purple are accent hues (`theme.css`) |
| `--activity-fill`, `-hover`, `-dim` | `#f0f0f0` … | `#2c2c2c` … | activity blocks; hovered; covered by a recording |
| `--block-fill-l`, `-spread`, `-c` | `0.91`, `0.1`, `0.07` | `0.36`, `0.08`, `0.06` | a filled recording's OKLCH lightness, lightness spread per shade, chroma cap |
| `--block-text-l`, `-c` | `0.32`, `0.1` | `0.94`, `0.035` | its text's lightness and chroma cap |

**Sidebar palette** tokens (everything inside the sidebar uses only these; `theme.css` maps the generic
tokens of shared controls to them inside `.sidebar`, and the accent hues come from the content palette of the
sidebar's tone):

| Token | Dark | Light | Role |
|---|---|---|---|
| `--sidebar-bg` | `#262626` | `#ebebeb` | the surface where it is opaque |
| `--sidebar-text`, `--sidebar-strong` | `#e6e6e6`, white | `#262626`, black | entries and days; the selected entry and the calendar's title |
| `--sidebar-muted` | `#9e9e9e` | `#636363` | icons, key caps, saved reports, week numbers, weekday initials, dimmed days, recording dots |
| `--sidebar-hover`, `--sidebar-selected` | white at 5 and 12% | black at 4 and 9% | hovered and selected entry, the calendar's highlighted day, week band and title, the tracker field (translucent, so they work on the material) |
| `--sidebar-line` | white at 10% | black at 10% | its edge, the separator, the week-number column, the tracker field's ring |

Derived tokens in `src/styles/theme.css`, the same for every palette:

| Token | Value | Role |
|---|---|---|
| `--accent` | `AccentColor` for `system`, else `var(--a-<choice>)`, via `[data-accent]`; re-derived inside the sidebar | interactive things only |
| `--style-red`, `-yellow`, `-blue`, `-purple` | `--a-red`, `--a-gold`, `--a-blue`, `--a-purple` | status tints (an error background is a `color-mix()` of `--style-red`) |
| `--o2`, `--o3` | `--text` at 68% and 52% | secondary and tertiary text |
| `--sel` | `--accent` at 20% (`--sidebar-selected` in the sidebar) | selected row |
| `--accent-text` | `--accent` with its lightness capped at `--accent-text-max-l` | the accent as text |
| `--focus-ring` | a 3px ring of the accent at 45% | focus on fields, buttons, segments |
| `--block-fill`, `--block-text`, `--block-meta` | from the project's shade `--rec` on `.card`, `.suggestion`, `.project-chip` | a filled recording: the fill, its text, its foot (text 75% over the fill) |
| `--ui-font-size`, `--data-font-size` | 13px, 12px | set from Settings by `applyAppearance` (`lib/theme.ts`) |
| `--fs-display`, `--fs-title`, `--fs-body`, `--fs-meta`, `--fs-caption` | 22, 15, 13, 11 and 10px in rem | the type scale |
| `--space-1` … `--space-6`, `--space-8`, `--margin` | 4 to 24px and 32px in rem; 20px | spacing; content margins |
| `--control-h`, `--toolbar-control-h` | 22px, 28px in rem | push buttons, fields, popup buttons, menu items; toolbar buttons |
| `--font-ui`, `--font-mono` | the system font (SF Pro), SF Mono | defaults; Settings accepts any installed family |
| `--radius-sm`, `--radius`, `--radius-lg` | 6, 10, 12px | controls; menus, popovers and inset groups; sheets and panels |
| `--radius-block`, `--radius-card` | 6, 8px | activity blocks; recording cards |
| `--toolbar-height`, `--traffic-lights` | 52px, 84px | the unified toolbar (centered on the window buttons); its left inset without the sidebar (px: they don't scale) |

**Accent:** Settings → Appearance "Accent" picks the app accent: System first (a swatch of the accent hues
around a circle; it follows the macOS accent color through WebKit's `AccentColor`, with white text on it),
then the nine hues. System is the default (`SYSTEM_ACCENT`); an unknown value falls back to it.

**Fonts:** none are bundled. The UI uses the system font (`-apple-system`, SF Pro) and machine text SF Mono
(`ui-monospace`). Settings → Appearance "UI font" and "Mono font" take any installed family name; empty means
the system one.

## Type

| Token | Size | Used for |
|---|---|---|
| `--fs-display` | 22px | large numbers: the timesheet total, the report total, the editor's duration, the running timer |
| `--fs-heading` | 17px | the sidebar calendar's month and year |
| `--fs-title` | 15px | toolbar titles (the Day view's `Wednesday, November 11, 2026`, or `Wed, Nov 11` with Settings → Appearance → Day title on Short), Settings page titles, the palette's field |
| `--fs-body` | 13px (the UI size) | everything else, block and card titles, sheet titles (bold) and group headings (semibold) included |
| `--fs-meta` | 11px | secondary text: meta lines, labels, descriptions, counts, the status bar, toolbar details |
| `--fs-caption` | 10px | hour labels, key caps |

Times and durations use tabular figures (the body sets `tabular-nums`); `--data-font-size` (12px) sizes
data values such as activity titles. Both sizes are settings: Appearance "UI size" (13px, 10–24) and "Data
size" (12px, 9–28); ⌘= and ⌘- zoom both, except in the Day view, where they zoom the timeline. Chrome sizes
are in rem of the UI size. Weights 400, 500, 600 and 700 (titles). Never mono in the UI: mono is for paths,
bundle ids, URLs and commands only. Durations read `1 h 05 min` (`formatDuration`), clock times `09:30`
(`formatClock`).

## Spacing and radius

Spacing sits on an 8px grid with 4px half steps (`--space-1` 4px to `--space-8` 32px, in rem so it zooms);
content keeps 20px margins (`--margin`). Rows are at least 28px (inset group rows 36px). Recording cards have
an 8px radius and 4px by 8px padding; activity blocks a 6px radius and a 1px gap to their neighbours; inset
groups and popovers a 10px radius; sheets 20px padding and a 12px radius.

## Controls

AppKit sizes and looks ([ADR 0024](adr/0024-native-macos-look.md)), in `styles/controls.css`:

| Kind | Class | Look |
|---|---|---|
| Push button | `.btn` | 22px high, 10px side padding, 6px radius, `--control` fill with `--control-shadow`, regular weight; pressed `--control-pressed`; no hover change |
| Default button | `.btn-primary` | the push button filled with the accent and `--on-accent` text (500). Only a sheet's or window's default action, last in its row |
| Destructive | `.btn-danger` | a push button with `--style-red` text (500) |
| Borderless | `.btn-quiet`, `.icon-btn` | no bezel until hovered (`--fill`), pressed `--fill-strong`; icons in `--o2`. `.icon-btn` is a 22px square |
| Toolbar button | `.btn` or `.icon-btn` inside `.toolbar` | borderless, 28px high, icon and label, hover bezel |
| Popup button | `select.input` | WebKit's native chevrons inside the push button bezel |
| Text field | `.input` | 22px, `--field` with a 1px inset `--field-line` frame, 6px radius; focus adds `--focus-ring` |
| Search field | `InsetField` (`.inset-field`) | a `--fill` capsule with a leading icon and an optional key cap; on focus the field surface, frame and ring |
| Segmented control | `SegmentedControl` (`.segmented`) | a 22px `--fill` track with 2px padding; segments in `--text`; the pressed one raised on `--raised` with `--shadow-small`, its label `--text-strong` |
| Switch | `.switch` | 32 by 18px, a `--fill-strong` track, a white `--knob` thumb with `--shadow-small`; on, the track is the accent |
| Key cap | `KeyCap` (`.keycap`) | the UI font at `--fs-caption`, `--o3` on a `--fill` rounded rect |
| Secondary label | `.secondary-label` | plain `--fs-meta` `--o2` text after a name (`billable`, `archived`); never a chip or badge |
| Swatches | `AccentSwatches`, `ShadeSwatches` | 16px circles with a hairline; the chosen one ringed; `withSystem` adds the System swatch first |

Buttons in a row end with the default button: Cancel, then Save; a destructive or secondary action sits at the
leading end (the recording editor's Delete).

## Components

| Component | Where | Look |
|---|---|---|
| `Toolbar` (`components/Toolbar.tsx`) | the first row of every view | see Shell; props `title`, `detail` and the actions as children |
| `Card` (`components/Card.tsx`) | a recording in the timesheet, the time column and the Week view's columns | filled with `--block-fill`, text `--block-text`, no shadow: the title (600), the subtitle (500), a foot with meta (the tag, with an icon) and the duration (tabular) on the right |
| `Block` (`components/Block.tsx`) | a summary block on the timeline, placed by time (activity lanes draw memories, see Timeline) | `--activity-fill`, 1px `--bg` border, `--radius-block`: the app (500, `--text-strong`), the title (`--text`), the duration trailing at the bottom right (`--o2`), as on a card; one line when short |
| Popover | `Modal variant="popover"`: the date picker, the timeline settings | `--pop`, `--radius`, `--shadow`, placed under its control, no scrim |
| Sheet | `Modal` (default `variant="sheet"`) | `--pop`, `--radius-lg`, `--shadow`, 20px padding, the title 13px bold, over `--scrim`; buttons at least 70px wide |
| Donut (`ReportChart` in `components/reports/`) | the Reports charts | an SVG ring of slices in accent colors on a `--fill` track, percentages outside, the total in the center, a legend beside |

## Opacity levels

| Level | Token | Opacity | Used for |
|---|---|---|---|
| Primary | `--text` | 100% | the thing you came for |
| Secondary | `--o2` | 68% | labels, units, relative times, inactive items, meta (at least 4.5:1 on `--bg`) |
| Tertiary | `--o3` | 52% | counts, empty states, placeholders (at least 3:1 on `--bg`) |

## Rows

Every list shares one row: sidebar entries, palette results, settings lists. A 20px line with padding above
and below (at least 28px; 36px in inset groups), hairlines between rows, no hover fill on plain rows, `--sel`
when selected. Sidebar entries have 2px more padding, an 8px inset and a 6px radius; the active one is
`--sidebar-selected` with its label in `--sidebar-strong` (500) and its icon in the accent.

Lists grouped under a heading are inset groups (see Surfaces): the Projects screen has one per client and
one for the tags, the heading above each the client's color dot and name (600) with `3 projects` in `--o3`
and the column labels. Archived rows are dimmed (55% opacity) and keep a Restore action. A project row starts
with a color dot (`AccentDot`, in its shade); a billable project carries `billable` as a secondary label
after its name. The project sheet has a Billable switch (a plain yes/no; there are no rates or amounts).
Colors are chosen with round swatches (`AccentSwatches` for a hue, `ShadeSwatches` for a shade), the chosen
one ringed. A tag is plain text, never a pill; a recording has at most one
([ADR 0019](adr/0019-one-tag-per-recording.md)).

## Inputs

Inputs look like AppKit's (see Controls), never like rows. A text field is a 22px frame on `--field` with a
1px inset `--field-line` and the accent focus ring; a search field (`InsetField`, `components/Field.tsx`) a
`--fill` capsule with a leading icon, the placeholder in `--o3` and an optional key cap that follows the
user's bindings. A select is a popup button. The segmented control serves the Day mode switch, the report's
Period and Settings choices.

## Overlays

Palette, popovers, context menus, toasts and dialogs: `--pop` with `--shadow` (a hairline ring and a soft
drop; no border). Menus, toasts and popovers take `--radius` (10px), sheets and panels `--radius-lg` (12px).
Sheets and the palette sit over `--scrim`; popovers (the date picker, the timeline settings) sit over
nothing. Overlays take `Esc` first. Palette results use the row anatomy with a 6px radius; the active one is
filled with the accent and `--on-accent` text, as in Spotlight, and command shortcuts show as key caps.
Context menus use the UI font, 22px items with a 5px radius; the hovered or focused item is `--accent` with
`--on-accent` text, disabled items `--o3`, separators an inset 1px `--line`. The native WebKit context menu is
suppressed outside text fields (`lib/nativeMenu.ts`).

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

The Day view reads like Timely's: the toolbar, the timesheet on the left, then the timeline.

- **Toolbar:** the date, the day's totals (`recorded 1:00 · tracked 6:12 · away 1:05`) only for a day other
  than today (the status bar has today's), the Timeline/List switch, Lanes (timeline only), the timeline
  settings gear and the ⋯ menu, all borderless toolbar buttons.
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
- **Away time and gaps** draw nothing: the lanes show only the captured activity, and the body keeps one
  background. Away time counts in the day's totals (`away 1:05`) and can be recorded from the away prompt.
- **Now** is a 1px accent line on today, updated every 30 s.
- **Empty day** (no activity and no recordings; away time alone is empty): "Nothing tracked on this day"
  with the tracker's status line; without the Accessibility permission "Tracking needs permission" with
  Allow…. The timesheet stays beside it.
- **Zoom** is 1–8 px per minute (default 2.5), kept in `ui_state.json` as `timelineZoom`: a floating control
  at the timeline's bottom right (−, a slider, +) and ⌘= / ⌘-.
- **Lanes** (toolbar): a popover with Summary and Activity switches and a switch per app of the day (most
  time first) that gives it a lane of its own (`ownLane`).
- **Compact** draws the day at 60% of the zoom, every block on one line.
- **Block gear:** a hovered block shows a gear at its top right; it opens the same menu as a right-click.

### Timeline settings

Settings → Timeline and the Day toolbar's gear (both save at once; the Day view reloads):

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
timeline), Hidden (gone from every lane; the Lanes popover lists hidden apps with Unhide), Own lane.

## Recordings

- **Recording by selection** (as in Timely; no key needed). Drag over empty time in the time column, the
  axis or the lanes, or click an activity block: the editor opens on release. Every click on an activity or
  summary block toggles that block's span in the editor's spans, with no modifier: a click on a block whose
  span is not in them adds it as another span, a click on one whose span they cover takes that time out
  again (a span that sticks out on both sides becomes two). Two clicked blocks give two spans with the time
  between them unrecorded; spans that touch or overlap once snapped merge into one. Taking out the last span
  closes the editor without saving, as Escape does. A drag while the editor is open adds its span the same way
  (merged with the spans it touches). Both work alike on a new recording and on the recording being edited
  (not on the running timer's, whose spans Rust ends). ⌥ on a block without an open editor drags it onto a
  project for a rule.
- **Snapping** (the Time snapping setting, 5 min by default) applies to every selection, drag and resize,
  on both ends, and Rust computes it: a selection over activity snaps each end to the nearest mark (blocks
  from 13:12 to 13:38 give 13:10–13:40); dragging an edge of a selection or a recording, or dragging in the
  time column, moves that edge in whole steps (13:40, 13:45, …); an end that would reach the other keeps one
  step. ⌥⇧↑ ⌥⇧↓ on a recording move its end the same way.
- **Selection** (the keyboard's way: V on the focused block, ⇧J and ⇧K add the next or previous one; ↵ opens
  the editor, or adds the span to an open one): a `--sel` band with accent hairlines over the lanes, with no
  time in it (the time column shows the times); the status bar shows the snapped span with the duration. A
  dashed accent outline in the time column shows where the recording goes. While the editor is open, each of
  its spans has its own band over the lanes and its draft block in the time column with its start and end, and
  every clicked block whose span is in them is marked selected (an accent edge on a summary block, an accent
  bar and title on an activity block) until a second click takes it out.
- **Recording blocks** sit in the time column, one per span: a recording can hold several separate
  spans (14:25–15:00 and 16:05–20:15), each its own block, all lit up together with the recording's single
  timesheet card when one is hovered or focused; project, note and tag belong to the recording and show on
  the card. A span shows only its start (top left) and end (bottom right), and a click on it opens the
  editor (the Week lists whole recordings as cards, see [Week and Month](#week-and-month)). Each is a `Card` filled in the project's shade (`--block-fill`, light in light palettes, a muted dark tint
  in dark ones) with `--block-text`, at least 4.5:1 for every accent and shade: the project name (600), the
  note (500), then the tag with a tag icon and the duration (tabular) at the bottom right. A dashed top or
  bottom edge means the span continues on another day. The top and bottom 6px resize the span; the body
  moves it. Activity under recordings (half or more) takes `--activity-fill-dim` (summary) or a regular-weight
  `--o2` label (memories); hover lifts it.
- **Editor**, inline in the timesheet (Timely's): a card with a 2px accent ring at the new recording's place
  among the cards (in place of the card it edits), pushing the cards below down and scrolled into view; it is
  never a sheet over the timeline (the Week and Month views, without a timesheet, show it as a sheet). Top to bottom: the
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
  `--pop` with `--radius-lg` opens on top of everything: `You were away 42 min` in `--fs-title`,
  `12:03–12:45 · idle` (asleep, locked), then project (the last used one), From and To side by side with the
  length beside them (snapped), note, tag, and Record (↵) / Skip (Escape). A failure reads in red on a light
  red tint above the buttons.
- A recording in the way reads `A recording already covers 12:10–12:30 (Project)` above the buttons; the
  form stays, so the times can change. The sheet stays open when another app takes focus.

## List view

- ⌘⇧L or Day mode → List shows the day as rows grouped like the timeline, longest first: the app's icon and
  name in `--o2`, the label (500), the URL or path (mono, `--o3`), the duration right (tabular) with a thin
  covered bar (the accent over `--fill-strong`) and `· 3 spans` when the activity came in parts. Rows are
  separated by hairlines, without a hover fill; the focused row is `--sel`. The search is an inset field with its `/` key cap. The timesheet with its cards stays on the left, as in the
  timeline.
- A strip of project chips (recent first, More… for the rest), each filled like a recording card, sits above
  the rows. Drag a row onto a chip
  to record it: the editor opens with the row's first start to last end and, for several spans, the warning
  `This activity has gaps (3 spans, 0:40 in between). Adjust the times if needed.`
- Keyboard: `/` search, ↑↓ rows, ↵ record (pick the project), Space show on the timeline.

## Rules and suggestions

- **Suggestions to review**: a rule's match is a suggestion, a largely prefilled recording (the rule's project,
  note and tag) that becomes a recording only once approved. Over the timesheet's cards a line in `--sel`
  reads "3 suggestions · Approve all · Dismiss all" while the day has any (both act on the shown ones).
- **Ghost cards** in the timesheet, in time order among the recordings' cards (Rust places each before the
  first card starting after it, `Suggestion.nextCard`): a card outlined dashed in `--block-meta` on a faint
  `--block-fill` (22% over `--bg-quiet`), the note as title (else the project), project and client, "Suggested
  by <rule>" with a sparkle, the tag and the duration; ✓ (Approve) and ✕ (Dismiss) round buttons top right.
  A click opens the inline editor prefilled with it ("Suggested by <rule>. Approve records it.", the primary
  button reads Approve): saving records it as edited and dismisses the rest of its time, one undo step.
- **Hatched blocks** in the time column: diagonal stripes of the project's fill, dashed outline, project, rule
  and duration; a click opens the editor on it like its card. The suggestion being edited gives way to the
  editor's draft spans.
- **Focus and keys**: ] and [ step through the timesheet, recordings and suggestions alike; Tab reaches a ghost
  card. A focused suggestion (accent ring on card and block) takes ↵ approve, E edit, ⌫ dismiss; after
  approving or dismissing, the focus moves to the next suggestion. Approve and dismiss each toast with Undo.
- **Dismissed stays dismissed**: a dismissed suggestion's time is never suggested again by that rule on that
  day, also after the rule is edited or a recording over it is deleted; only Undo brings it back.
- **Drop strip**: ⌥-dragging an activity block (while no recording editor is open) shows the recent projects on `--pop` over the top of the
  recordings column; the one under the pointer gets a dashed accent outline. Dropping opens the rule sheet,
  filled in; nothing is saved before Save. From the keyboard, the block menu's Make a rule… (⌃↵) opens the
  sheet with the block's conditions and no project.
- **Rules screen**: the toolbar with New rule, the intro, then one inset group of rules (at most 52rem
  wide), a row per rule divided by hairlines: a grip, the enable switch, the name (600) with today's matches
  on the right ("1h 20m today · 2 to review" in the accent while suggestions wait, "Nothing today", "Off" when
  disabled), then the project as a dot in its shade with its name and the conditions in `--o2`; a disabled
  rule's text is dimmed. A dragged row lifts onto `--pop` with `--shadow`. The order is the priority; drag the grip, or ⌥↑ ⌥↓ with the focus in a row.
- **Rule sheet**: the form on the left, "Last 7 days" on the right with the matching time and the hits
  (day and time in `--o2`, the app, the title). Below 720px the preview moves under the form.

## Timer

- A running recording's open span grows to now in the Day view, with a pulsing accent dot at its top right
  (still with reduced motion); it has no grips. Starting the timer for a project, note and tag already
  recorded today continues that recording with a new span. The menu bar item's title reads `⏱ 1:23 Acme` while it runs, and the
  status bar shows `⏱ Acme 1:23` (a click stops it).
- The picker (⌘⇧T from any app, ⌘T in the window, Start Timer… in the tray) is a small floating panel on
  `--pop` with `--radius-lg` under the tray icon: "Start timer", project, note, tag, Start (↵), Cancel
  (Escape), and below a hairline the "Recent" timers as rows with their project's dot. While a timer runs it
  shows the project and the elapsed time large, the note, Stop and "Start another…". It hides when it loses
  focus.

## Week and Month

- **Week** (Timely's Week view): seven columns, Monday at the left, with no time axis and no empty hours.
  Each head reads `Mon 19` (semibold; today as "Today" in accent text) with the day's recorded hours
  (`--o2`, tabular) on the right; a click opens the day. Below, the day's recordings as filled `Card`s in
  start order, a small gap apart: project, note, the tag with its icon, and the day's duration at the bottom
  right. A card grows with its duration, from one line of title over the foot to a cap at six hours (Rust's
  height units), so a 6 h recording is clearly taller than a 10 min one. A recording with several spans that
  day is one card with its total and "n spans"; one past midnight shows in both days with each day's share.
  A "New recording" button (+) follows the last card (in an empty column, a dashed slot at the top); it opens the editor as a
  sheet on the default span on that day. A click on a card focuses it (accent ring), a double-click or ↵
  edits it, ⌫ deletes it. Dragged to another column (the column under the pointer gets an accent ring) the
  recording's time on that day moves there at the same clock times; with ⌥ it is copied. Times are edited in
  the editor or the Day view; the Week has no resize. Today's column has a faint accent tint, Saturday and
  Sunday a `--bg-quiet` one; the columns scroll together.
- **Month**: whole weeks, Monday in the top-left cell, cells divided by `--line` hairlines, weekends on
  `--bg-quiet`. The weeks fill the view's height, sharing it evenly; a week whose fullest day needs more grows
  taller than the others and the grid scrolls. Each cell reads `Mon 19` at the top (today's number in an accent circle, days outside the month
  dimmed; a click opens the day), a + on hover (the editor for that day, as in the Week) and the recorded
  hours on the right, then one line per recording in start order, all the same height: a dot in the
  project's shade, the note or the project (ellipsized) and the duration on the right; a click edits it. No
  cap on the lines.
- Both toolbars read the period's label and its totals, `recorded 32:10 · tracked 41:05` (Month: recorded).
  The Month grid runs edge to edge under the toolbar, without an outer frame, as in Calendar.

## Projects

- The toolbar holds Show archived, New client and New project (toolbar buttons). Below, one inset group per
  client (by name; client-less projects under "No client" last) under its heading: the client's dot, its
  name (a click edits it), `3 projects` in `--o3`, the column labels This week and This month, and + for a
  new project of that client. Each project row: its dot, the name (a click edits it), `billable` and
  `last used 2 d ago` as secondary labels, then the hours this week and this month as `3:20` (tabular,
  `--o3` at zero; Rust computes both for the current week and month). Right-click: Edit…, Archive or
  Restore, Copy id.

## Tags

- A page of its own (⌘6): the toolbar reads `Tags` and `One per recording, for kinds of work across projects.`
  Below, one inset group with a row per tag (its name, the number of recordings, ✕ to delete; a confirm first
  when recordings carry it), then the New tag field (↵ adds); ⌘N (`tags.new`) asks for a name in a sheet.

## Settings

- A panel in the System Settings layout: the page list on `--chrome` (28px rows; the current page filled
  with the accent and `--on-accent` text, as a focused macOS sidebar) and the page on `--bg`: the title in
  `--fs-title` bold, the intro in `--o2`, then groups. A group heading is 13px semibold `--text` over its
  inset group; each row (at least 36px) has the label and its description (`--fs-meta`, `--o2`) on the left
  and the control on the right, hairlines between rows. Counts and durations use tabular
  figures; paths, bundle ids and commands are mono and selectable. A failure reads in red on a light red
  tint.
- Appearance → **Dates**: Calendar title (Short `Nov 2026`, the default, or Full `November 2026`) and Day
  title (Long `Wednesday, November 11, 2026`, the default, or Short `Wed, Nov 11`, with the year only when it
  is not this year's), as segmented controls. Rust formats both; a change reloads the Day view and the
  calendar. Other labels (the palette, Week and Month toolbars, reports) keep their own formats.

## Reports

The Reports screen follows Timely's day report, without money. The toolbar has the period as title, then
the grouping of the copied text (By project, tag, day, week or month), Copy as text and Save report…; below,
one slim filter bar (Period presets as a segmented control, Custom revealing dates, clients, projects, tags, rounding, archived projects). The
P and N step the period, O goes to the one holding today, and the sidebar's calendar highlights it when it is one day, week or month. The content sits in inset groups:

1. **Totals:** `Total` with the hours large (`--text-strong`, `--fs-display` bold), then Recordings,
   Projects, Untagged (only when there is untagged time) and `Billable 6:24 h · 80 %` (of the rounded total;
   only when time is recorded), each a `--o2` label over its value. With rounding, the unrounded total stands
   below in `--o3`.
2. **Charts:** two donuts side by side, By client and By tag; a By select on each switches between client,
   project and tag. The ring has the slices in their colors (a client's accent, a project's shade, a tag's
   accent by its place in the tag list, graphite for No client and Untagged) on a `--fill` track, the
   percentages outside the ring in `--o2` (from 3 %), the total and the count (`2 clients`) in the center,
   and a legend with a dot, the name and the hours.
3. **Table:** rows divided by hairlines: client rows (600) with their hours; project rows with a color dot,
   the name, `billable` as a secondary label, the number of recordings, a borderless **Copy notes** button
   and the hours; a click on a project
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
and Reports; and one scope per view (`day`, `week`, `month`, `reports` under `dated`; `projects`, `tags`, `rules`
under `global`), active while that view shows. `suggestion` (under `dated`, beside `day`) is active while the
Day view has a focused suggestion and no selection or focused recording, so its ↵ and ⌫ never clash with the
recording commands'.

| Command | Label | Default key | Scope |
|---|---|---|---|
| `settings.toggle` | Settings… | ⌘, | global |
| `app.launchAtLogin` | Launch at login (a check item) | menu | global |
| `welcome.show` | Show welcome again | menu | global |
| `shortcuts.show` | Keyboard shortcuts | ? | global |
| `palette.toggle` | Command palette | ⌘K | global |
| `sidebar.toggle` | Toggle sidebar | ⌘⌃S | global |
| `view.day` … `view.rules` | Show Day, Week, Month (each keeps the shown date), Reports, Projects, Tags, Rules | ⌘1 … ⌘7 | global |
| `view.hours` | Show Hours: today's Day view, as the sidebar's Hours | menu | global |
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
| `recording.new` | New recording… (the Day view's selection, else the half hour up to now on today; the Week and Month stay) | menu | global |
| `app.quit` | Quit TimeTracker | ⌘Q | global |
| `nav.prev` / `nav.next` | Previous / Next day, week, month or report period | P / N | dated |
| `nav.today` | Go to today (this week, this month, the report's period that holds today) | O | dated |
| `nav.goToDate` | Go to date… (the date picker) | menu | dated |
| `day.nextBlock` / `day.prevBlock` | Next / Previous block in time | J / K | day |
| `day.prevLane` / `day.nextLane` | Block in the lane to the left / right | H / L | day |
| `day.details` | Block details | Space | day |
| `day.blockMenu` | Block actions… (app settings, copy, Make a rule…) | ⌃↵ | day |
| `day.selectBlock` | Select block (the focused one's span) | V | day |
| `day.extendNext` / `day.extendPrev` | Extend selection to the next / previous block | ⇧J / ⇧K | day |
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
| `tags.new` | New tag… (a name prompt) | ⌘N | tags |
| `rules.new` | New rule… | ⌘N | rules |
| `rules.moveUp` / `rules.moveDown` | Move rule up / down (the rule whose row has the focus) | ⌥↑ / ⌥↓ | rules |
| `suggestion.approve` / `suggestion.edit` / `suggestion.dismiss` | Approve / Edit / Dismiss the focused suggestion | ↵ / E / ⌫ | suggestion |
| `suggestion.approveRule` | Approve all suggestions of the focused one's rule | menu | suggestion |
| `suggestion.approveAll` / `suggestion.dismissAll` | Approve / Dismiss all the shown day's suggestions | menu | day |

Keys of controls, not commands (they act on the focused control and are not rebindable):

| Where | Keys |
|---|---|
| List mode | `/` search, ↑↓ rows, ↵ record the row (pick the project), Space show it on the timeline; Tab to a project chip and ↵ records the focused row |
| Ghost card (Tab reaches it) | focuses the suggestion: ↵ approve, E edit, ⌫ dismiss (the `suggestion` scope); ✓ and ✕ inside it |
| Recording editor, away prompt, timer picker | project picker: type, ↑↓, ↵ picks (the last option creates a project); From and To: type `HH:MM` or ↑↓ by one snap step; ↵ saves, records or starts; in the editor also ⌘↵, and ⌘⌫ deletes; Escape cancels or skips |
| Timeline, pointer | drag or click opens the editor; with the editor open, a click on a block adds its span or takes it out again, and a drag adds a span |
| Context menu (⌃↵ on a block) | ↑↓ items, ↵ runs, Escape closes |
| Pickers in the Reports filter | type, ↑↓, ↵ adds, ⌫ on an empty field removes the last |
| Week and Month | Tab to a day head (Week) or a day's date (Month), ↵ opens that day; Tab to a card (Week) or a line (Month), ↵ opens the editor; on a Week card ⌫ deletes it; Tab to + and ↵ opens the editor for that day |
| Date picker (Go to Date…, under the calendar's title) | ←→ by a day, ↑↓ by a week, PageUp / PageDown by a month, ↵ shows the day, Escape closes |
| Settings → Shortcuts | a key field records the next key; Escape stops, ⌫ unbinds |

Left to the pointer, each with a keyboard way to the same result: dragging a selection (V, ⇧J, ⇧K, or the
List mode), clicking or dragging a span into the open editor ("Add span" and its From / To fields), dragging or resizing a recording (⌥↑ ⌥↓ ⌥⇧↑ ⌥⇧↓), dragging a card to another day in the
Week view (⌥← ⌥→, ⌥⇧ to copy, in the Day view),
⌥-dragging a block onto a project (⌃↵ → Make a rule…), the rules' grip (⌥↑ ⌥↓), the hover card (Space), the
saved reports' right-click menu (`reports.rename`, `reports.delete`), and the Projects screen's right-click
menu (Edit and Archive are in the project sheet). Only the right-click "Copy id" of a project has no key; the
CLI and the MCP server list the ids.

### Record a day

From any view, with the documented default keys only (the pointer ways in brackets):

1. ⌘1 shows the Day view, O today (or Hours in the sidebar, `view.hours`; the sidebar calendar's days for
   any other day).
2. J focuses the first block of the day (K, H and L move on); Space shows its details.
3. V selects the focused block's span; ⇧J (or ⇧K) grows the selection over the next (or previous) block, as
   far as the work went. The status bar shows the snapped span and its length; Escape clears it. (Drag over
   the time column or the lanes, or click a block: the editor opens on release.)
4. ↵ opens the inline editor in the timesheet, at the recording's place among the cards, with the note
   focused: type it, then Tab to the project (often proposed already): type part of the name, ↑↓, ↵ picks it
   (or the last option, `New project "x"…`, opens the project sheet). Tab on to the tag and the duration.
5. Tab to a span's From and To: type `09:15` (or `915`) or press ↑↓ to move by one snap step; "Add span"
   adds another. (With the editor open, a click on a block adds its span, a second click takes it out again;
   a drag adds a span.)
6. ↵ in a field or ⌘↵ saves. If it overlaps a recording, the editor lists them: ↵ on "Replace overlapped
   part" replaces it, Back returns to the fields; Escape cancels the editor.
7. The new recording has the focus: ⌥↑ ⌥↓ move it, ⌥⇧↓ ⌥⇧↑ change its end, ⌥← ⌥→ move it to another day,
   ↵ edits it again, ⌫ deletes it, ⌘Z undoes the last change. ] and [ step through the timesheet.
8. Rule suggestions are ghost cards in the timesheet, in time order among the recordings: ] or [ to one,
   ↵ approves, E opens it in the editor, ⌫ dismisses; the Day menu (or ⌘K) approves or dismisses all of them.
9. Back to step 2 for the next stretch (J moves on from the last focused block). P and N step to other days.
10. ⌘2 shows the week (P N by week; Tab to a day head and ↵ opens it, Tab to a card and ↵ edits it), ⌘3 the
    month (by month; Tab to a day and ↵ opens it).
11. ⌘4 shows Reports (P N step its period, O goes back to the current one): Tab to the Period presets, on
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
| File | New Recording… (`recording.new`); Start Timer… (`timer.start`; ⌘T), Stop Timer (`timer.stop`; ⌘⇧.); New Project… (`projects.new`; ⌘N), New Client… (`clients.new`), New Tag… (`tags.new`; ⌘N), New Rule… (`rules.new`; ⌘N); Export Backup… (`data.exportBackup`); Close Window (`window.hide`; ⌘W) |
| Edit | Undo (`edit.undo`; ⌘Z), Redo (`edit.redo`; ⌘⇧Z); Cut, Copy, Paste, Select All (native); Delete Recording (`recording.delete`; ⌫); Copy Report as Text (`reports.copyText`; ⌘⇧C), Copy Report Notes (`reports.copyNotes`; ⌥⌘C) |
| View | Hours (`view.hours`); Day, Week, Month, Reports, Projects, Tags, Rules (`view.day` … `view.rules`; ⌘1 … ⌘7, the active view checked); Show as List (`day.toggleList`; ⌘⇧L, checked in List mode); Toggle Sidebar (`sidebar.toggle`; ⌘⌃S); Zoom In, Zoom Out, Reset Zoom (`zoom.*`; ⌘= ⌘- ⌘0); Timeline Settings… (`day.settings`); Enter Full Screen (native) |
| Go | Previous (`nav.prev`; P), Next (`nav.next`; N), Today (`nav.today`; O), Go to Date… (`nav.goToDate`); Next Block (`day.nextBlock`; J), Previous Block (`day.prevBlock`; K); Next Recording (`recording.next`; ], recordings and suggestions), Previous Recording (`recording.prev`; [) |
| Day | Record Selection (`recording.create`; ↵, the key of `recording.open`), Edit Recording… (`recording.edit`); Move Recording to Previous / Next Day (⌥← / ⌥→), Copy Recording to Previous / Next Day (⌥⇧← / ⌥⇧→); Approve Suggestion (↵), Edit Suggestion… (E), Dismiss Suggestion (⌫), Approve All Suggestions of Rule, Approve All Suggestions, Dismiss All Suggestions (`suggestion.*`); Delete Activity for This Day…, Delete Activity in a Range… (`activity.*`) |
| Window | Minimize, Zoom, Bring All to Front (native) |
| Help | Keyboard Shortcuts (`shortcuts.show`; ⇧?), Command Palette… (`palette.toggle`; ⌘K); Open Log Folder (`logs.open`), Show Data Folder (`data.reveal`), README (`help.readme`) |

New Project, New Tag and New Rule share ⌘N: only one of them is enabled at a time (Projects, Tags or Rules). Record Selection
and Approve Suggestion share ↵, Delete Recording and Dismiss Suggestion share ⌫: a focused suggestion clears the
selection and the recording focus, so only one of each pair is enabled.

Commands left out of the menu, with keys and the palette only:

| Commands | Why |
|---|---|
| `day.prevLane` / `day.nextLane`, `day.details`, `day.blockMenu`, `day.selectBlock`, `day.extendNext` / `day.extendPrev`, `day.clearFocus` | step-by-step keyboard work on the focused block; the Go menu has the block steps J and K |
| `recording.open` | the Day menu has its two halves, Record Selection and Edit Recording… |
| `recording.nudgeEarlier` / `nudgeLater` / `extend` / `shrink` | one snap step per press, held to repeat; a menu click per 5 minutes helps no one |
| `reports.save`, `reports.rename`, `reports.delete` | saved reports live in the sidebar, whose right-click menu renames and deletes them |
| `rules.moveUp` / `rules.moveDown` | act on the focused rule row, next to its drag grip |
| `tracker.pause`, `tracker.resume` | the sidebar's play / pause button and the menu bar item (tray) have Pause Tracking with its durations and Resume |
| `timer.toggle` | File has Start Timer… and Stop Timer |

## Scheme audit

The three schemes in both modes are checked on Day, List, Week, Month, Reports, Projects, Tags, Rules and Settings
(`docs/screens/<scheme>-<mode>-<view>.png`, captured by `scripts/dev/screenshot-looks.sh <dir> [view]`, which
sets the current macOS mode's scheme to each of the three through `scripts/dev/js.sh` and restores the
settings; the app follows macOS, so switch the system appearance and run it again for the other mode). Only
the README's are committed: `default-light-day.png`, `dark-light-day.png` (the Dark scheme with macOS in
light mode) and `default-light-reports.png`. Each scheme is
one look in either mode; the mode changes the native window chrome and, where the sidebar's tone matches it,
lets the sidebar material show through. Fixes live in `src/styles/palettes/` only.

Targets, computed by `src/styles/contrast.ts` (WCAG contrast; OKLCH as the CSS computes it):
- `--text` at least 7:1, `--o2` at least 4.5:1 and `--o3` at least 3:1 on `--bg`; `--line` (over `--bg`) at
  least 1.3:1 against `--bg`.
- `--on-accent` at least 4.5:1 on every palette accent (the System accent takes white, as macOS does); each
  accent at least 3:1 on `--bg`.
- A filled recording, for every accent and all nine shades: `--block-text` at least 4.5:1 on `--block-fill`,
  its foot (`--block-meta`) too.
- Sidebar: `--sidebar-text` and `--sidebar-muted` at least 4.5:1 on `--sidebar-bg`; every accent of the sidebar's tone
  at least 3:1 on it.

Measured: light content block text 8.5:1 at worst (foot 4.6), dark content 8.3 (foot 5.4); `--o3` 3.2 (light)
and 4.6 (dark); the weakest on-accent pair 4.65 (gold, light). The List view's covered bar fills with the
accent.
