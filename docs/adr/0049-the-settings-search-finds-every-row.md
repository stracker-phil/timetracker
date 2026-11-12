# 0049. The Settings search finds every row

## Status

Accepted (2026-11-12), implemented. Inherited from the family catalog (settings-window; reference: Launcher ADR
0046). Supersedes the Search bullet of [0034](0034-settings-is-a-window-of-its-own.md).

## Context

The search narrowed the page list by page label, group and each section's title and keywords. The keywords
were the row labels copied in by hand, so they drifted from the rows and no test noticed; a hit named only one
section per page, the match was a plain substring, and the page opened at the group, not at the row.

## Decision

- **Rows, not sections.** Each page module exports its rows (`SettingsRow`: `id` = the row's `anchor`, `title`
  = its label, `keywords` for synonyms only, `under` = the anchor to reveal while the row is hidden). Every
  searchable `Row` (or a `Group` of rows built from data) carries its `anchor`.
- **Ranked like the palette.** Pages and rows are ranked by `settingsScore` (`lib/rank.ts`, the palette's
  scorer): the title as the palette scores it, then a keyword, then the title's letters in order.
- **Two-line hits.** Typing replaces the page list with one ranked list; a row hit shows its page and, below,
  the row's title. ↵ opens the first hit.
- **Scroll and wash.** A hit opens the page scrolled to the row and washes it in the accent once
  (`--motion-highlight`), again on every pick. `open_settings` and `app:settings:<id>` take a row id too.
- **A drift test** (`searchIndex.test.tsx`) renders every page and fails on an anchor without an entry, an
  entry without its row, or a title that isn't the row's label.
- Rejected: section keywords kept by hand (they drift); reading rows from the rendered pages at runtime (pages
  not shown aren't rendered); searching descriptions (keywords hold the synonyms); one hit per page (it hides
  the other rows).

## Consequences

- Group ids are no longer link targets; links name a page or a row. No link in the app used a group id.
- Rows whose label depended on the data got stable labels ("Exclude an app", "Exclude a website", "Add a
  source"), which also keeps them in place (stable controls).
