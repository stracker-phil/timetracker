# 0016. Color schemes are complete token sets; no color literal outside a scheme

## Status

Accepted (2026-10-21), implemented. Inherited from DataBee ADR 0024.

Superseded in part by [0021](0021-appearance-modes-and-schemes.md): the eight schemes and the light and dark scheme
settings.

## Context

The app runs in light and dark mode and shows project colors on blocks all day. A missing dark value shows
only when someone looks; project colors must read on every scheme.

## Decision

A color scheme is a complete set of tokens in `src/styles/schemes/<id>.css` under `[data-scheme="<id>"]`:
surfaces, text, lines, shadows, the nine accent hues `--a-<name>` and the five value-style colors. Settings
pick one scheme for light and one for dark; the defaults are DataBee's Ayu Light and Ayu Mirage, and the other
six schemes are copied as they are. A project's `color` is an accent name, so its block uses `--a-<color>`
and reads on every scheme. `theme.css` holds only derived tokens. No color literal appears outside a scheme
file; a Vitest scan enforces it. Free-form project colors (a hex picker) were rejected because they break on
scheme changes and invite status-like coloring.

## Consequences

- Nine project colors; clients group projects visually, not more hues.
- Adding a scheme is one file; every missing token fails the scan.
- Every new style needs a token first.
