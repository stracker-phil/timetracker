# 0050. Settings → Shortcuts reassigns keys and resets only off the default

## Status

Accepted (2026-11-12), implemented. Inherited from the family catalog (keyboard-first; reference: Launcher ADRs
0032, 0045).

## Context

Recording a key another command held saved the clash and then showed a warning banner above the page. Reset
showed whenever `keybindings.json` held an override, even one equal to the default. The rows were one list,
each with its scope as the description.

## Decision

- **Reassign.** Recording a key another command holds where this one's key works asks in a sheet that names
  both; Reassign saves once, the other command losing its key; Cancel and Esc keep both. A command keeps one
  binding and a key one command per scope (`holderOf`, `reassign` in `commands/shortcutEdits.ts`).
- **Reset only off the default.** Reset shows only while the key differs from the default; recording the
  default drops the override (`bind`). Reset and Remove keep their slots while hidden, so rows never shift.
- **Rows by where the key works:** System-wide, In the main window, In Day, Week, Month and Reports, In each
  view, With a suggestion focused, In the Settings window. Each row has an anchor (`shortcut:<id>`), so the
  Settings search ([0049](0049-the-settings-search-finds-every-row.md)) finds every command.
- Rejected: the warning banner (the clash is already saved when it shows); Reset on every override; the scope
  as each row's description (it repeated on every row).

## Consequences

- Like every setting, a reassignment isn't on the Undo stack, which holds recording changes only
  ([0010](0010-recordings-never-overlap-ops-with-undo.md)); recording the old key again restores it.
- A clash typed by hand into `keybindings.json` is no longer pointed out; the more specific scope wins, as
  before.
