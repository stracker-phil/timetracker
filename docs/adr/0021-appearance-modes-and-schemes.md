# 0021. Appearance is a mode and one of three schemes, built from four palettes

## Status

Accepted (2026-10-22), implemented. Supersedes in part [0016](0016-color-schemes.md): the eight editor
schemes and the light and dark scheme settings. 0016's rule that palettes are complete token sets and that
no color literal appears outside them stays.

## Context

Part 1 copied DataBee's eight code-editor schemes (Ayu, Monokai, Nord, Solarized, …) with one setting for
the light scheme and one for the dark. They made the app look like a code editor. The owner works in
Fantastical and Timely every day: calm calendar apps with a dark sidebar by default and an Appearance setting
that offers a mode and a few designed looks, not a gallery of themes.

## Decision

Settings → Appearance has two controls:

- **Mode**: Light, Dark or System (follows macOS; the default). Setting `mode`.
- **Scheme**: Default (dark sidebar, light content; the default), Light (all light) or Dark (all dark).
  Setting `scheme`.

In dark mode the whole window is dark whatever the scheme, the sidebar a shade darker than the content; the
scheme decides only the light-mode look. The Dark scheme in light mode therefore equals dark mode, and the six
combinations give three distinct looks: dark sidebar with light content, all light, all dark.

The colors come from four palettes designed for this app with warm-grey neutrals, one per surface and tone:
light content, dark content, light sidebar, dark sidebar (`src/styles/palettes/`). `lib/theme.ts` resolves the
mode and scheme to a content tone and a sidebar tone and sets `data-content` and `data-sidebar` on `<html>`.
The sidebar has its own token set (`--sidebar-*`) and its own accent hues, so it can be dark beside light
content. The nine accent hues stay, tuned per palette so each reads as a dot, label and fill on its surface.

The settings file migrates by itself: `mode` and `scheme` replace `theme`, `lightScheme` and `darkScheme`,
which are no longer read, so a part 1 file loads as mode System and scheme Default.

## Consequences

- No scheme gallery: adding a look means a new scheme value and, at most, another palette file.
- Every palette of a kind defines the same tokens; the color scan of 0016 covers `palettes/`.
- Contrast is checked per palette (text, muted text, accent fills, filled recording blocks for every accent
  and shade) instead of per editor scheme.
- A user's old theme choice (for example dark) is reset to System once.
