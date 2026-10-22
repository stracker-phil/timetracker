# 0022. Appearance follows macOS, with one scheme per mode

## Status

Accepted (2026-10-22), implemented. Supersedes in part [0021](0021-appearance-modes-and-schemes.md): the Mode
setting, the single `scheme` and the rule that dark mode makes the whole window dark. 0021's three schemes,
four palettes and `data-content` / `data-sidebar` tones stay.

## Context

0021 gave Appearance a Mode (Light, Dark, System) and one Scheme that decided only the light-mode look; in
dark mode the window was dark whatever the scheme. The owner wants the app to always follow macOS and to
pick a look for each macOS mode, for example all light even while macOS is dark. With one scheme, dark mode
with the Light scheme still rendered dark.

## Decision

- The app always follows the macOS appearance. There is no Mode setting and no app override; the native
  window theme is never set, so it follows the system too.
- Two settings: `lightScheme` (the look while macOS is light; default `default`) and `darkScheme` (while
  macOS is dark; default `dark`). Each is one of Default (dark sidebar, light content), Light (all light) or
  Dark (all dark).
- The look is the scheme of the current macOS mode, nothing else: dark mode with `darkScheme` Light is all
  light. `lib/theme.ts` maps a scheme to a content and a sidebar tone (`schemeLook`) and picks the scheme by
  `prefers-color-scheme` (`resolveLook`).
- Settings → Appearance shows a Light | Dark segmented control, starting on the current macOS mode, that
  switches which mode's scheme tiles are shown below it. The accent stays one setting for both modes.
- The settings file migrates by itself: a 0021 file's `scheme` becomes `lightScheme` (`darkScheme` takes its
  default) and `mode` is ignored; part 1's `lightScheme` and `darkScheme` values (editor scheme names) are
  unknown and load as the defaults. The old keys are dropped on the next save.

## Consequences

- Each scheme is one look in either mode, so the scheme audit captures three looks per mode, and a mode can
  only be captured by switching macOS itself (`scripts/dev/screenshot-looks.sh` uses the current one).
- Native controls (menus, pickers) follow macOS, so they can be dark beside a light page when `darkScheme`
  is Light or Default; the page's own controls follow the content palette's `color-scheme`.
- A 0021 Mode choice (for example a forced Dark) is lost once; the window follows macOS again.
