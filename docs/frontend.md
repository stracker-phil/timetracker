# Frontend

`src/` holds a React 19 + TypeScript app, bundled by Vite. State lives in one zustand store. The frontend
renders and forwards intent; [Rust owns the logic](architecture.md#design-rules).

## Boot

1. `index.html` loads `main.tsx`.
2. `main.tsx` calls `installModules()` (`src/modules.ts`) before the first render:
   `registerCoreSettingsPages()` (`components/settings/pages.tsx`), then `installCommands()` (core scopes,
   then the command registry).
3. `applyInitialScheme()` (`lib/theme.ts`) sets the default scheme for the OS appearance, so the first paint
   has colors before settings load.
4. `installNativeMenuGuard()` (`lib/nativeMenu.ts`): a right-click that no app menu handled shows nothing,
   so the WebView's Reload and Inspect Element never appear; text fields keep Cut/Copy/Paste.
5. `main.tsx` assigns the store to `window.timetracker` only when `import.meta.env.VITE_DEV_HOOKS` is `"1"`
   (the debug bundle of `bun run build:app`), then renders `<ErrorBoundary><App/></ErrorBoundary>`.
6. `App.tsx` imports the stylesheets (fonts, the scheme files, `theme.css`, `app.css`) and calls
   `useBackendEvents()` once:
   1. `loadUiState()` loads the window layout, then `restoreDay()` shows the last viewed day (within 30
      days). `loadSettings()` loads settings and keybindings; `installCommands()` runs again to apply the user's
      bindings, then `drainLaunches()` pulls the queued launch events (`take_launch_events`): `error` shows a toast, `showDay`
      calls `goToDay`.
   2. It subscribes to `launch` (drain again), `command` (menu and tray → `runMenuCommand`), `remote`
      (→ `answerRemote`, which always replies with `remoteReply`), `data_changed` (→ `dataChanged`) and
      `tracker_status` (→ `setTracker`); `loadTracker()` fetches the current status once.
   3. It adds a capture-phase `keydown` → `handleKeydown`, `error` / `unhandledrejection` → error toast, and
      `beforeunload` → `flushUiState`.
7. `applyAppearance(settings, systemDark)` runs whenever settings or the OS appearance change.

```
App
└─ .app grid
   ├─ Toolbar        traffic-light inset, app title, Day/Week/Month segmented control, Settings button
   ├─ Sidebar        Day, Week, Month, Reports, Projects; a tracker status line at the bottom
   ├─ main           DayPage for Day, ProjectsPage for Projects, else EmptyView: the active view's name
   └─ StatusBar      left: status text; right: zoom
Dialogs (settings, shortcuts, confirm) · PermissionDialog · CommandPalette · Toasts
```

## Store

`state/store.ts`: one zustand store assembled from slices (`registerSlice`); `resetStore(patch)` restores
every slice's initial state for tests. `state/requests.ts` has `fail(e, context)`: an error toast with the
error's message, returning the `AppError`.

| Slice | Holds |
|---|---|
| `settings` | `settings`, `keybindings`, `loadSettings`, `saveSettings`, `saveKeybindings` |
| `ui` | `toasts` (an optional `action`, e.g. Undo), `dialog` (`settings { page? }`, `shortcuts`, `confirm`, `project { id?, clientId? }`, `client { id? }`, `rule { id?, draft? }`), `paletteOpen`, `paletteSeed` |
| `nav` | `view` (`day`, `week`, `month`, `reports`, `projects`, `rules`), `setView`, `day` (the shown `YYYY-MM-DD`, today at start), `goToDay(day)` (also switches to the Day view; `timetracker show` uses it), `goToday` (keeps the view), `step(±1)` (a day, a week or a month by the view; `addDays`, `addMonths` in `lib/day.ts`); each remembers the day as `uiState.lastDay`, and `restoreDay` shows it again at boot when it is at most 30 days old |
| `day` | `dayView` (the shown day's `DayView`; the previous one stays while the next day loads), `dayError`, `loadDay` (drops answers for a day no longer shown), `focusedBlock`/`focusBlock`, `detailsBlock`/`toggleDetails` (the card opened by keyboard), `blockMenuRequest`/`requestBlockMenu`, `setAppSetting(appKey, patch)` (stores the returned settings; the Day view reloads on the `data_changed` it causes) |
| `reports` | `reportFilter` (`null` follows `settings.reports` through `defaultFilter`), `setReportFilter(patch)`, `savedReports`/`loadSavedReports` (boot), `saveReport(name)` (the shown filter), `renameSavedReport`, `deleteSavedReport`, `openSavedReport` (its filter, and the Reports view) |
| `rules` | `rules` (by position), `loadRules` (the Rules view, and every `data_changed` of `rules`), `saveRule(id, input)` (rejects; toasted unless the error has a `field`), `deleteRule`, `reorderRules(ids)`, `setRuleEnabled`, `acceptSuggestion(suggestion)` and `acceptRuleSuggestions(ruleId)` on the shown day, each with a toast offering Undo |
| `data` | `dataVersion` (every day changed), `dayVersions` (per day), `changeCount` (every event, for views over many days), `dataChanged(event)` (called on every `data_changed` event); `selectDayVersion(day)` changes when that day's data may have changed |
| `projects` | `projects` (`ProjectListItem[]`, archived included), `clients`, `tags` (`TagListItem[]`), `recent`, `projectsLoaded`; `loadProjects` (boot, and every `data_changed` of `projects` or `tags`); `saveProject(id, input)`, `archiveProject`, `saveClient`, `archiveClient`, `saveTag`, `deleteTag` reload after the write and reject on failure (toasted, unless the error has a `field` for the form). Helpers: `groupProjects(projects, clients, archived)`, `projectLabel` |
| `tracker` | `tracker` (`TrackerStatus`, or `null` while the tracker is off), `setTracker`, `loadTracker`, `pauseTracker(until)`, `resumeTracker`, `requestPermission` (asks macOS, then reloads the status), `permissionDialogHidden` |
| `recordings` | `selection` (`{ start, end, blockIds }`, unsnapped; Rust snaps on save), `selectionSnap` (as Rust will store it), `setSelection`, `extendSelection(blockId)`, `focusedRecording`/`focusRecording`, `editor` (`create { start, end }` or `edit { id }`)/`openEditor`/`closeEditor`, `undoStatus`/`loadUndoStatus`, `createRecording(input)`, `updateRecording(id, patch)`, `deleteRecording(id)`, `undo`, `redo`. A write runs with `overlap: cancel`; on `overlap` it opens the overlap dialog and runs again with `replace`. One write at a time; each ends with a toast offering Undo. A field error rejects for the editor, others are toasted |
| `viewState` | `uiState` (`UiState` from Rust), `setPaneSize`, `toggleSidebar`, `rememberDay`, `setTimelineZoom` (px per minute, `TIMELINE_ZOOM` 1–8, default 2.5); a debounced writer saves it, `flushUiState` writes at once |

Components select narrowly (`useStore((s) => s.view)`), never the whole store. `lib/useApiResult` loads a
value from `api` keyed on its inputs (and on `selectDayVersion(day)` or `dataVersion`), so a `data_changed` event
reloads every view showing that data.

## API boundary

Only `api.ts` files import `@tauri-apps/api` or `@tauri-apps/plugin-*`: the core's is `src/api/index.ts`, an
optional module's is `src/<module>/api.ts` ([ADR 0002](adr/0002-ipc-in-api-files.md)); `test/boundary.test.ts`
enforces it. `src/api/index.ts` exports `api`, a flat object of thin `invoke` wrappers for the commands in
[backend.md](backend.md#tauri-commands) plus the subscriptions `onLaunch`, `onCommand`, `onRemote`,
`onTrackerStatus` and `onDataChanged`, and `toAppError`, which normalizes rejections to `AppError`.

The IPC types are generated from Rust with ts-rs ([ADR 0004](adr/0004-generated-ipc-types.md)) into
`src/api/types.gen.ts`; never edit it. `src/api/types.ts` re-exports them plus the frontend-only constants:
`ACCENTS` and `SCHEMES` (mirror Rust's lists; a Rust test compares them), `TEXT_SIZES` and `SETTINGS_LIMITS`
(mirror the clamps in `Settings::sanitized`, the source of truth), `LOG_LEVELS`, `BUNDLED_FONTS`,
`Keybindings` and `EMPTY_UI_STATE`.

## Commands

Every user action that has a shortcut or appears in the palette is a `Command` (`commands/registry.ts`):
`{ id, title, scope, defaultBinding?, when?, run, menu? }`. The core commands are `APP_COMMANDS`
(`commands/index.ts`); their default bindings are in [ux.md](ux.md#keys).

- **Bindings** are strings like `Mod+Shift+K` (`Mod` is ⌘), normalized to `Mod, Ctrl, Alt, Shift` and shown
  as `⌘⇧K`. User overrides come from `keybindings.json`.
- **Scopes** (`commands/scopes.ts`) form a tree: `global`, and under it `day`, `week`, `month`, `reports`
  and `projects`, each active while `view` matches. The most specific active scope wins a key.
- **Keys.** `handleKeydown` runs in the capture phase. Inside text inputs only `Mod`/`Ctrl` combinations and
  Escape fire. While a dialog or the palette is open only `MODAL_ALLOWED` commands run. Auto-repeat is
  swallowed unless the command is in `REPEATABLE` (zoom).
- **Menu.** Native menu and tray items emit `command`; `runMenuCommand` runs the id under the same rules.
  A command shown in the menu has the same id and default as its `MENU_ITEMS` entry in `menu.rs`.
- **Palette.** `CommandPalette` lists the commands of the active scopes with their key caps, filtered by the
  typed text; once something is typed, active projects follow as `Project: Client / Name` with their color
  dot (fuzzy, `lib/fuzzy.ts`), and Enter opens the project's sheet. `>` limits the list to commands, `p:` to
  projects.
- **Day, Week and Month** (scope `calendar`, the parent of the three views' scopes). `nav.prev` (⌘←),
  `nav.next` (⌘→) step by the view's unit; `nav.today` (T).
- **Day view.** `day.nextBlock` (J),
  `day.prevBlock` (K), `day.prevLane` (H), `day.nextLane` (L), and with a focused block `day.details`
  (Space), `day.blockMenu` (⌃↵); `day.clearFocus` (Escape) clears the block focus, the selection and the
  focused recording.
- **Recordings.** `recording.open` (Enter) edits the focused recording or records the selection;
  `recording.create` and `recording.edit` (menu and palette) do one of the two; `recording.delete` (⌫);
  `recording.nudgeEarlier`/`nudgeLater` (⌥↑ ⌥↓) move the focused recording and `recording.extend`/`shrink`
  (⌥⇧↓ ⌥⇧↑) change its end, by one snap step. `edit.undo` (⌘Z) and `edit.redo` (⌘⇧Z) are global and leave
  text fields to their own undo. While the editor is open, only `MODAL_ALLOWED` commands run. `zoom.in`/`zoom.out`/`zoom.reset` (⌘= ⌘- ⌘0)
  zoom the timeline (`TIMELINE_STEPS`) while the Day view shows and the text everywhere else.
- **Projects.** `projects.new` (⌘N) and `clients.new` (no default) open the sheets while the Projects view
  shows; `view.projects` (⌘5) shows it.
- **Tracker.** `tracker.pause` (until resumed; while tracking) and `tracker.resume` (while paused), no default
  bindings.
- **Window.** `window.hide` calls `api.hideWindow`; `app.quit` is handled by Rust when it comes from the menu.

## Components

Shell components in `components/`: `Toolbar` (Day/Week/Month switch), `Sidebar` (the entries of `views.ts`), `StatusBar`, `EmptyView`, `Modal`, `ContextMenu`,
`Toasts`, `Field` (`InsetField`, `KeyCap`), `Splitter`, `ErrorBoundary`, `CommandPalette`,
`ShortcutsDialog`, `Dialogs`.

Tracker components:

- `TrackerStatus` (sidebar foot): a dot (accent while running, `--o3` while away or paused, `--style-red`
  without permission), the status text Rust computed (the tray shows the same), and Pause (a `ContextMenu`:
  15 minutes, 1 hour, until tomorrow, until resumed) while running or away, Resume while paused, and
  "Allow…" (shows the permission dialog again) without permission. "Tracking off" while `tracker` is `null`.
- `PermissionDialog`: a `Modal` while the state is `needsPermission`. It explains why Accessibility is needed
  and what is never captured; "Open System Settings" (`openPrivacySettings`) and "Check again"
  (`requestPermission`). It disappears once the status is no longer `needsPermission`. Escape hides it
  (`permissionDialogHidden` in the `tracker` slice) until "Allow…" or until the permission is lost again.

### Day view

`components/day/` renders `DayView` from Rust; it groups, packs and sums nothing.

| File | Renders |
|---|---|
| `DayPage` | `DayHeader` and `Timeline`; the empty state ("Nothing tracked on this day" with the tracker's status text on today, or "Tracking needs permission" with Allow…). Loads on a new day, on `selectDayVersion(day)`, and while today on `useLiveReload` (every 30 s, and on tracker statuses at most every 10 s, since the tracker sends no `data_changed`) |
| `DayHeader` | ‹ Today ›, the date (`formatDayLabel`), totals `recorded 0:00 · tracked 6:12 · away 1:05` (`formatHm`), and the gear that opens `TimelineSettingsPopover` (command `day.settings`, no default key) |
| `TimelineSettingsPopover` | dialog `timelineSettings`, a `Modal` panel under the header's gear: `TimelineControls` and "Hidden apps: n" from `dayView.hiddenApps`, each with Unhide |
| `Timeline` | the scroll box: `LaneHeader` (sticky), the recordings column (empty until recordings), `TimeAxis`, the lanes with `Spans` (away, gaps) behind them and the current-time line on today. Opens a day scrolled to 30 min before its first activity. Hover (one `mouseover` listener reading `data-block-id`) shows `HoverCard` after 300 ms; right-click, the gear on a hovered block, or ⌃↵ opens the block menu. With `compact` it draws the day at 60% of the zoom and adds `timeline--compact` |
| `blockMenu.ts` | the block menu's entries: for tracker blocks Group by app, Titles: Document name / App name and Own lane, checked by the effective value (an override equal to the timeline's setting is removed), Hide (a toast with Undo); then Copy title (not for private blocks), Copy URL |
| `Lane` | its blocks, absolutely placed: `top`/`height` from the times and the zoom, `left`/`width` from `col`/`cols`; only blocks within the visible part ± one screen (`useVisibleRange`, which moves in half-screen steps) |
| `ActivityBlock` | app and `label` (no label line when it is the app; one line when short or compact, no text when tiny), a gear on hover; `private`, `dimmed` (covered) and `focused` classes |
| `HoverCard` | app, title, URL (http(s) opens through `api.openUrl`) or path, time and duration, the source when external; beside the block, else below or above it |
| `RecordingsColumn` | the day's `SuggestionBlock`s under its `RecordingBlock`s and, while a span is selected, a dashed outline where its recording would go |
| `SuggestionBlock` | a rule's suggestion: a button, dashed in the project's shade, with the project, the rule and the duration; click sends `acceptSuggestion`, ⌥-click `acceptRuleSuggestions` (the `rules` slice; Rust's `data_changed` reloads the day) |
| `useRuleDrag` | ⌥-pressing an activity block (in `Timeline`) shows the recent projects over the top of the recordings column; releasing over one opens the `rule` dialog with `draftFromBlock`: `<app> → <project>`, `app is <app>` and `domain is <host>` for a URL block, else `title contains <title>` |
| `RecordingBlock` | the project's shade as a 3px bar and a 22% tint, project, note, tags and duration; dashed top or bottom edge when clipped to the day. Click focuses, double-click edits; dragging the body moves it and an edge resizes it (`pointerDrag`, pointer capture), with the ghost from `useSnappedSpan`; release sends `updateRecording` |
| `RecordingEditor` | project (`ProjectPicker`), From and To (`HH:MM`, ↑↓ step by the snap interval; an end before the start is on the next day), duration, note (one line; pasted line breaks become spaces), tags (`TagPicker`); Save (↵ or ⌘↵), Cancel, Delete (⌘⌫) when editing. It edits the whole recording, also when the day shows a part of it |
| `OverlapDialog` | the recordings in the way, "Replace overlapped part" or Cancel |
| `selectionPress` | the press on the lanes and the axis: a drag selects a span (auto-scrolling near the edges) and dropped on the recordings column opens the editor; a click selects a block's span, ⇧-click extends the selection, a click on nothing clears it |
| `useSnappedSpan` | a span as Rust will snap it (`api.recordingSnap`, at most once per frame; the raw span stands in until the answer) |

`lib/blockFocus.ts`: `stepBlock` (time order across lanes) and `stepLane` (the nearest block in the next
lane with blocks) for J/K/H/L. The status bar shows the selection as `09:00–10:30 · 1:30`.

### Timer

The `timer` slice holds `timer` (`TimerStatus` from `timer_status` events and `loadTimer` at boot),
`startTimer` (an `overlap` reads "A recording already covers now") and `stopTimer` (a toast with Undo).
Commands `timer.start` (⌘T: `timerPickerOpen`), `timer.stop` (⌘⇧. while one runs), `timer.toggle`. The status
bar shows the running timer. Settings → Shortcuts has a Global shortcut group above the in-app commands: the
recorder saves `timerShortcut` through `saveTimerShortcut`, which shows Rust's refusal under the row.

Secondary windows are pages of their own in `src/windows/<name>/` (`index.html`, `main.tsx`), each a
`build.rollupOptions.input` entry in `vite.config.ts`; they import `src/styles/index.ts` and use the same
store and `api`. `windows/timer/TimerPicker.tsx` reloads settings, projects, the timer and the recent timers
whenever the window gains focus (it is hidden, not closed, between uses), and reports its errors to the log.

### Rules screen

`components/rules/`, the `rules` view (sidebar entry Rules, ⌘6). The `rules` slice holds `rules` (by
position) and their writes; a `data_changed` of kind `rules` reloads them.

- `RulesView` lists `RuleRow`s in order: a grip (a pointer drag by whole rows; on release `reorderRules`
  sends every id in the new order, shown at once and restored when the write fails), the enable switch
  (`setRuleEnabled`), the name, the project and `conditionSummary` (`app is "Slack" · title contains "x"`).
  A row click, New rule or ⌘N (`rules.new`) opens the `rule` dialog.
- `RuleSheet` (dialog `rule` with `id` or a `draft`): name, `ProjectPicker`, note, `TagPicker`, the
  `ConditionRow`s (field, comparison, value, remove; Add condition) and the enable switch. Save sends
  `saveRule`; a field error shows under its field, and `conditions.<i>.value` marks that row. Beside the
  form, `RulePreviewPanel` asks `rulePreview(input, 7)` through `useApiResult` (300 ms debounce, keeps the
  last result) once every condition has a value, and lists the hits with the total.

### Week and Month

`components/calendar/`: `CalendarHeader` (‹ Today ›, the title, totals). `WeekView` runs `weekView(day)` keyed
by the day and `changeCount`: day heads (label and total; a click opens the day) over a `TimeAxis` and seven
columns that place each day's `RecordingBlock`s from the column's bounds. A block keeps its vertical move
and resize; released over another column (`onDrop`, found with `elementFromPoint`) it moves there, with ⌥ it
is copied (`moveRecording`/`copyRecording` in the `recordings` slice, which ask about overlaps like every
write). A double-click on an empty spot opens the editor for an hour from there. `MonthView` runs
`monthView(day)`: a seven-column grid of `DayCell`s (day number, recorded `h:mm`, a 3 px strip of the top
three projects' shades by time); a click opens the day.

The editor's `edit` request may carry `at` (a span of the recording), so views other than the Day view open it.

### Reports screen

`components/reports/`: `ReportsView` (the `reports` view) runs `reportRun(filter)` keyed by the filter and
`changeCount`: a header with the range, Copy as text (`reports.copyText`, ⌘⇧C: `reportText` to the
clipboard) and Save report… (`reports.save`: the `prompt` dialog, then `saveReport`); `ReportFilters`
(range presets and Custom dates, `MultiSelect` for clients, projects and tags, Group by, Rounding, Archived
projects); one `ReportGroupTable` per group (color dot, label, `h:mm h`, the notes, the recordings on
request); the total, and the tag overlap caveat. `actions.ts` holds the two commands' actions. The sidebar
lists saved reports under Reports: a click opens one, right-click renames (prompt) or deletes (confirm).

`PromptDialog` (`dialog: prompt { title, label, initial, confirmLabel, onSubmit }`) asks for one line.

### Projects screen

`components/projects/`: `ProjectsPage` (the `projects` view): a header (New project, New client, Show
archived), a band per client by name with its color dot (every client, "No projects" when empty; clicking
the name edits the client; a `+` creates a project for that client, except on archived clients),
client-less projects under "No client" last, then `TagsSection`. A project row: color dot and name (click
edits), `last used 2 d ago` and `12:30 h / 30 d` in mono `--o2`; archived rows (with Show archived) are
dimmed with Restore. The row's context menu: Edit…, Archive or Restore, Copy id.

- `ProjectSheet` (`dialog: project`, an optional `clientId` preselects the client): name, client select
  with "New client…" (an inline name; Save creates the client first), the color (`AccentSwatches`, only
  without a client or with a new one) and the shade (`ShadeSwatches`; none pressed lets Rust pick), Archive
  or Restore for an existing project. `FormField` (`components/Field.tsx`) shows `AppError.field` errors
  under their field.
- `ClientSheet` (`dialog: client`): name, color (none pressed on a new client lets Rust pick); Archive
  (also archives its projects) or Restore.
- `TagsSection`: one row per tag (name in mono, the number of recordings, delete; a confirm first when
  recordings carry it) and a New tag field (Enter).

### Pickers

`components/pickers/`, pure UI over the `projects` slice (they only call the api to create). Both open their
list (`PickerList`, a portal below the field, so a sheet's overflow never clips it) on focus; ↑ ↓ move,
Enter picks, Esc closes the list without closing the sheet around it; the list closes on blur.

| Picker | Props | Behavior |
|---|---|---|
| `ProjectPicker` | `value: string \| null`, `onChange(projectId)`, `autoFocus?` | shows the chosen project as color dot + `Client / Name`; empty query: "Recent" (up to 5) then all active projects by client; typed: fuzzy matches, best first; the last option is always `New project "<query>"…`, which opens `ProjectSheet` prefilled and picks the saved project. Archived projects never show |
| `TagPicker` | `value: string[]`, `onChange(tagIds)` | chosen tags as mono `a · b` in the field; typing filters the unchosen tags; Enter adds the highlighted one or creates `New tag "<query>"` (`saveTag`) and adds it; ⌫ on an empty query removes the last tag |

### Settings pages

`components/settings/SettingsDialog.tsx` renders the pages registered in `registry/settingsPages.ts`;
`pages.tsx` registers the core ones (`registerCoreSettingsPages`), `controls.tsx` holds the shared rows:

| Page | Holds |
|---|---|
| Appearance | theme, light and dark scheme swatches, accent row, UI and data sizes, fonts |
| Shortcuts | `ShortcutsPage`: every command with its binding; record, reset or unbind |
| Tracking → Capture | `CapturePage` (`CapturePage.tsx`): address and document switches, excluded apps (an "Add app…" picker listing `appsSeen(30)`), excluded websites (a field; Enter adds) |
| Tracking → Tracker | `TrackerPage`: away after (idle minutes), check every (poll seconds) |
| Tracking → Timeline | `TimelinePage` (`TimelinePage.tsx`): `TimelineControls` (summary lane and its interval slider, show all activity, compact, group by app, continuity, noise, titles, time snapping; each saved quietly), then Apps from `appsList()`: per app Group by app and Titles (Default or a value), Hidden, Own lane |
| System → General | Launch at login |
| System → Data | the data folder (Reveal in Finder), counts, first activity, size on disk, Export backup… (save panel → `backupExport`, toast "Backup written"); an alert when the store could not be opened |
| System → Logging | log level, Open log folder |

A change saves through `saveSettings` (`api.updateSettings`) and applies at once.

## Styling

The CSS is plain and global, imported in this order by `App.tsx`: `styles/fonts.css`, `styles/schemes/<id>.css`
(one file per scheme), `styles/theme.css` (derived tokens) and `styles/app.css` (sectioned with
`/* ---------- name ---------- */` comments: shell, fields, menus, dialogs, settings, palette, toasts). Icons
come from `lucide-react`. Token names and roles are in [ux.md](ux.md#color-schemes-and-tokens).

- **Color schemes.** `applyAppearance` sets `data-scheme` on `<html>` from `theme` and the
  `lightScheme` / `darkScheme` settings, and `data-accent` from `accent`; Rust (`window.rs`) sets the native
  window theme. Adding a scheme: a file in `styles/schemes/`, its import in `App.tsx`, its id in Rust's
  `SCHEMES` and in `SCHEMES` in `src/api/types.ts`.
- **No color literals** (`#hex`, `rgb(`, `hsl(`) outside `styles/schemes/`; `styles/colors.test.ts` scans.
- **Sizing.** Chrome is in rem: `html { font-size: var(--ui-font-size) }`, so zoom scales it.
- **macOS look.** The toolbar leaves `--traffic-lights` for the window buttons; `data-tauri-drag-region`
  marks draggable areas; `user-select: none` applies globally, inputs and `.selectable` opt back in.

## Tests

- **Setup.** Vitest with jsdom (`vite.config.ts`). `src/test-setup.ts` loads the jest-dom matchers, stubs
  `ResizeObserver`, `matchMedia` and the clipboard, and sets `navigator.platform` to Mac, so `Mod` is ⌘.
- **Mocking.** UI tests mock only the `src/api` module:

  ```ts
  vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));
  const mocked = vi.mocked(api, true);
  mocked.getSettings.mockResolvedValue(settings({ accent: "blue" }));
  ```

  `createApiMock()` returns every `api` member as a `vi.fn()` with harmless defaults; a new `api` member
  needs a default there.
- **Fixtures.** `src/test/settings.ts` (`settings(patch)`, the default `Settings`), `src/test/now.ts` (a fixed
  time).
- **Scans.** `test/boundary.test.ts` (Tauri imports only in `api` files), `styles/colors.test.ts` (no color
  literals outside schemes).
- **Style.** Query by role or text, drive the UI with `userEvent`, and assert what is visible or which API
  call was made. Reset with `resetStore()` in `beforeEach`.
