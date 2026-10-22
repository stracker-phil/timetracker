# 0024. The app looks native: system font, inset groups, a unified toolbar, AppKit control metrics

## Status

Accepted (2026-10-22), implemented. Supersedes in part [0021](0021-appearance-modes-and-schemes.md): the
warm-grey neutrals of the four palettes and the dark sidebar a shade darker than dark content. 0021's three
schemes, four palettes, `data-content` / `data-sidebar` tones and the nine accent hues stay.

## Context

After part 1 and the calendar look of 0021 the owner still found the app unprofessional: a warm-grey page
behind cards with a 1px border and a radius, rows repeating separators and hover fills, web-style buttons (an
orange filled primary beside outlined ones), badges, mono numbers, and a toolbar strip that only repeated the
window title. Calendar, Reminders and System Settings, the apps it sits beside, have none of that.

## Decision

- **Type:** the system font (SF Pro through `-apple-system`) for all text at the macOS text styles (13px body,
  11px secondary, 15px titles, 22px display numbers), tabular figures everywhere. The bundled Geist and
  JetBrains Mono are removed; mono (SF Mono through `ui-monospace`) is only for machine text such as paths,
  bundle ids, URLs and commands. The font settings still accept any installed family.
- **Surfaces:** neutral greys, no tint. Content is the window background (white, `#1e1e1e` dark). Grouped
  content is a System Settings inset group: a translucent quaternary fill (`--fill-quiet`), a 10px radius,
  no border, hairline separators between rows, no hover fill on plain rows. Overlays draw a hairline ring and
  a soft shadow instead of a border. The dark sidebar is macOS's dark sidebar, a shade lighter than dark
  content; where a scheme's sidebar tone matches the macOS mode, the window's sidebar material shows through
  (vibrancy).
- **Window chrome:** a unified toolbar. Every view renders its own `Toolbar`: the view's title, its detail and
  its actions in one 52px row beside the window buttons, which sit centered in it. The separate title strip
  and its Settings button are gone (Settings stays on ⌘, the app menu and the palette). The title bar stays
  `Overlay` with drag regions.
- **Controls:** AppKit metrics: 22px push buttons with a 6px radius and a bezel shadow; the accent fill only
  for a sheet's default button; borderless toolbar and inline buttons with a hover bezel; native popup
  buttons (WebKit's own chevrons inside a push button bezel); 22px fields with a hairline frame and the
  accent focus ring; a fill-track segmented control with a raised segment; a white-thumb switch. Default
  buttons sit last, Cancel before them, destructive actions at the leading end. Labels after a name
  (`billable`, `archived`) are plain secondary text, never chips.
- **Accent:** a System accent follows the macOS accent color (WebKit's `AccentColor`) with white text on it,
  and is the default. The nine hues stay for projects and as app accents.

## Consequences

- `docs/ux.md` holds the token and control spec; the palettes define translucent fills (`--fill-quiet`,
  `--fill`, `--fill-strong`), control tokens (`--control`, `--control-pressed`, `--control-shadow`) and
  `--on-system-accent`; `--row-hover`, `--track` and `--shadow-inset` are gone.
- A view that has a title renders `Toolbar`; the window can be dragged by any empty part of it.
- Settings files keep their accent; only new ones start on System. An unknown accent falls back to System.
- The vibrant sidebar depends on the macOS mode, so a scheme looks slightly different in each mode; the
  Default scheme's dark sidebar in light mode stays opaque.
