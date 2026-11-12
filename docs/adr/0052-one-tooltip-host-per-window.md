# 0052. One tooltip host per window

## Status

Accepted (2026-11-12), implemented. Inherited from the family catalog (tooltip-host; reference: Daybook ADR
0039, Taskboard `components/TooltipHost.tsx`). Owner's decision (2026-11-12).

## Context

Controls carried native `title` tooltips. WebKit shows them late, in the system's small style that ignores the
schemes, they can hang over a sheet or the palette after it opened, and text cut off with an ellipsis showed
nothing at all.

## Decision

- **`TooltipHost`**, once per window (main, Settings, timer picker, away prompt), takes over every `title`: it
  moves it to `data-tooltip` on first hover (an element named only by its title keeps it as `aria-label`) and
  shows one styled tooltip after 450ms, at once within 600ms of the last one, and on keyboard focus. Text cut
  off with an ellipsis shows in full.
- Below the control, above it without room, at most 420px wide and 8px inside the window; a press, a key, the
  wheel, leaving or the window losing focus hides it.
- Colors are tokens of both content palettes (`--tooltip-bg`, `--tooltip-text`, `--tooltip-border`); the rise
  takes `--motion-fast`.
- Components keep writing `title`; a tooltip that names a key ends with the current binding
  (`useBindingLabel`), never a spelled key. The recording editor's ⌘⌫ is the editor's own fixed key, not a
  command, so its title keeps it.
- Rejected: native `title` (slow, unstyled, over overlays); a `<Tooltip>` wrapper per control (every control
  changes, and a forgotten one falls back to the native tooltip).

## Consequences

- Tests that look for a `title` before any hover still find it; after a hover it is `data-tooltip`.
