# Frontend

`src/` holds a React 19 + TypeScript app, bundled by Vite. State lives in one zustand store. The frontend
renders and forwards intent; [Rust owns the logic](architecture.md#design-rules).

## Boot

1. `index.html` loads `main.tsx`.
2. `main.tsx` calls `installModules()` (`src/modules.ts`) before the first render:
   `registerCoreSettingsPages()` (`components/settings/pages.tsx`), then `installCommands()` (core scopes,
   then the command registry).
3. `applyInitialLook()` (`lib/theme.ts`) sets the default look (light mode Default, dark mode Dark) for the OS
   appearance, so the first paint has colors before settings load.
4. `installNativeMenuGuard()` (`lib/nativeMenu.ts`): a right-click that no app menu handled shows nothing,
   so the WebView's Reload and Inspect Element never appear; text fields keep Cut/Copy/Paste.
5. `installInputModality()` (`lib/inputModality.ts`): sets `<html data-input="keyboard">` on a key press
   outside a text field (or Tab in one) and clears it on a pointer press; the key focus styles (`.focused` on
   blocks, recordings, suggestions, timesheet and Week cards) apply only under it, so a click draws no outline.
6. `main.tsx` assigns the store to `window.timetracker` only when `import.meta.env.VITE_DEV_HOOKS` is `"1"`
   (the debug bundle of `bun run build:app`), then renders `<ErrorBoundary><App/></ErrorBoundary>`.
7. `App.tsx` imports the stylesheets (`styles/index.ts`: the palettes, `theme.css`, then the
   stylesheets by layer) and calls
   `useBackendEvents()` once:
   1. `loadUiState()` loads the window layout, then `restoreDay()` shows the last viewed day (`get_ui_state`
      leaves out one older than 30 days). `loadSettings()` loads settings and keybindings; `installCommands()` runs again to apply the user's
      bindings, then `drainLaunches()` pulls the queued launch events (`take_launch_events`): `error` shows an error toast,
      `notice` an info toast (a migration that dropped tags), `showDay` calls `goToDay`.
   2. It subscribes to `launch` (drain again), `command` (menu and tray → `runMenuCommand`), `remote`
      (→ `answerRemote`, which always replies with `remoteReply`), `data_changed` (→ `dataChanged`) and
      `tracker_status` (→ `setTracker`); `loadTracker()` fetches the current status once.
   3. It adds a capture-phase `keydown` → `handleKeydown`, `error` / `unhandledrejection` → error toast, and
      `beforeunload` → `flushUiState`.
8. `applyAppearance(settings, systemDark)` runs whenever settings or the OS appearance change.

```
App
└─ .app grid
   ├─ Sidebar        MiniCalendar, Hours, Reports, then Projects, Tags, Rules; SidebarStatus at the bottom
   ├─ main           the active view's page, its own Toolbar first (EmptyView for an unknown one)
   └─ StatusBar      left: sidebar toggle, TrackerStatus (sidebar hidden only), the selection; right: running timer, billable ratio, today's totals
Dialogs (settings, shortcuts, confirm) · PermissionDialog · Welcome · CommandPalette · Toasts
```

## Store

`state/store.ts`: one zustand store assembled from slices (`registerSlice`); `resetStore(patch)` restores
every slice's initial state for tests. `state/requests.ts` has `fail(e, context)`: an error toast with the
error's message, returning the `AppError`.

| Slice | Holds |
|---|---|
| `settings` | `settings`, `keybindings`, `loadSettings`, `saveSettings`, `saveTimeline(patch)` (timeline settings, saved quietly), `saveKeybindings`, `setLaunchAtLogin(enabled)` (resolves whether it is on now, `null` on a toasted failure) |
| `ui` | `toasts` (an optional `action`, e.g. Undo, with an optional `ttlMs`; an action toast stays 10 s by default), `dialog` (`settings { page? }`, `shortcuts`, `confirm`, `project { id?, clientId? }`, `client { id? }`, `rule { id?, draft? }`, `deleteActivity { day, bounds, range }`, `timelineSettings`, `datePicker`), `paletteOpen`, `paletteSeed` |
| `nav` | `view` (`day`, `week`, `month`, `reports`, `projects`, `tags`, `rules`), `setView(view, then?)` (another view first clears the selection and leaves the editor through `leaveEditor`; `then` runs once the view shows, never after Keep editing), `day` (the shown `YYYY-MM-DD`, today at start), `goToDay(day)` (also switches to the Day view; `timetracker show` uses it), `showHours` (Hours: `goToDay` today; the sidebar), `goToday` (keeps the view), `showDate(day)` (keeps the view; the date picker), `step(±1)` (a day, a week or a month by the view, through `stepThroughRust` in `lib/dayStepper.ts`: `calendarStep`, queued so quick presses add up, a failure toasts; the date picker and ⌥← ⌥→ use it too); in Reports the three move the report's period instead (`shiftReportRange`); each remembers the day as `uiState.lastDay`, and `restoreDay` shows it again at boot (Rust drops one older than 30 days) |
| `day` | `dayView` (the shown day's `DayView`; the previous one stays while the next day loads), `dayError`, `loadDay` (drops answers for a day no longer shown), `focusedBlock`/`focusBlock`, `detailsBlock`/`toggleDetails` (the card opened by keyboard), `blockMenuRequest`/`requestBlockMenu`, `setAppSetting(appKey, patch)` (stores the returned settings; the Day view reloads on the `data_changed` it causes) |
| `reports` | `reportFilter` (`null` follows `settings.reports` through `defaultFilter`), `setReportFilter(patch)`, `shiftReportRange(anchor, by)` (`api.reportRangeShift`: the period `by` steps of its unit away, from the period holding `anchor` when given, so `by = 0` is that period; failures toast), `savedReports`/`loadSavedReports` (boot), `saveReport(name)` (the shown filter), `renameSavedReport`, `deleteSavedReport`, `openSavedReport` (its filter, and the Reports view) |
| `rules` | `rules` (by position), `loadRules` (the Rules view, and every `data_changed` of `rules`), `saveRule(id, input)` (rejects; toasted unless the error has a `field`), `deleteRule`, `reorderRules(ids)`, `setRuleEnabled`, `rulesToday` (`RuleDay[]`, loaded after the rules by `loadRules`; empty when it fails) |
| `suggestions` | `shownSuggestions(state)`, `focusedSuggestion`/`focusSuggestion` (one thing holds the Day view's key focus: `focusCleared` in `state/focus.ts` gives what a suggestion, a recording/block/selection, or another day clears), and on the shown day `approveSuggestion(suggestion)`, `approveRuleSuggestions(ruleId)`, `approveSuggestions(ids)`, `dismissSuggestions(ids)` (each toasts with Undo and moves the focus from a settled focused suggestion to the next one left), `editSuggestion(suggestion)` (focuses it and opens the editor `create` with its span, project, note, tag and `suggestion { id, ruleName }`) |
| `data` | `dataVersion` (every day changed), `dayVersions` (per day), `changeCount` (every event, for views over many days), `dataChanged(event)` (called on every `data_changed` event); `selectDayVersion(day)` changes when that day's data may have changed |
| `projects` | `projects` (`ProjectListItem[]`, archived included), `clients`, `tags` (`TagListItem[]`), `recent`, `projectsLoaded`; `loadProjects` (boot, and every `data_changed` of `projects` or `tags`); `saveProject(id, input)`, `archiveProject`, `saveClient`, `archiveClient`, `saveTag`, `deleteTag`, `setTagEmoji(id, emoji)` reload after the write, settle once the lists have reloaded, and reject on failure (toasted, unless the error has a `field` for the form). Helpers: `groupProjects(projects, clients, archived)`, `projectLabel` |
| `tracker` | `tracker` (`TrackerStatus`, or `null` while the tracker is off), `setTracker`, `loadTracker`, `pauseTracker(until)`, `resumeTracker`, `requestPermission` (asks macOS, then reloads the status), `permissionDialogHidden` |
| `recordings` | `selection` (`{ start, end, blockIds }`: a drag's on `DayView.ticks`, a block selection's with the blocks' own times), `selectionSnap` (the selection on the grid, which the band, the outline and the status bar draw: a drag's as it is, a block selection's once `api.recordingSnap` answered), `setSelection` (draws it as it is), `extendSelection(blockId)` (⇧J, ⇧K; asks `api.recordingSnap` once per key), `focusedRecording`/`focusRecording`, `hoveredRecording`/`hoverRecording` (the recording under the pointer in the timesheet or the time column), `undoStatus`/`loadUndoStatus`, `createRecording(input, suggestionId?)` (with a suggestion: `api.suggestionApproveEdited`), `updateRecording(id, patch)`, `updateSpan(id, spanId, span, moving?)` (a drag's span on ticks; with `moving`, Rust keeps the other edge's time), `nudgeSpan(id, spanId, startSteps, endSteps)` (Rust applies the snap step), `deleteRecording(id)`, `moveRecording(id, day, spanId)`/`copyRecording(id, day, spanId)` (one span, else with `null` the whole recording), `undo`, `redo`. A write runs with `overlap: cancel`; on `overlap` it asks (inside the editor while one is open, else the overlap dialog) and runs again with `replace`. One write at a time; each ends with a toast offering Undo. A field error rejects for the editor, others are toasted |
| `editor` | `editor` (`create { start, end, projectId?, note?, tagId?, suggestion? }` or `edit { id, at?, spanId? }`), `draft` (`{ projectId, spans, totalMs, note, tagId }`; null until Rust has answered for its spans and an edited recording has loaded through `recordingsList`), `editing` (that recording), `editorOverlap` (the recordings a save would overlap, with `resolve`), `draftBlockIds` (the blocks clicked into the draft, marked selected on the timeline), `editorView` (the view `openEditor` ran in), `openEditor`, `closeEditor` (also clears the selection), `leaveEditor(proceed)` (no editor: `proceed` at once; a draft unchanged since it opened: closes it, then `proceed`; a changed one: the confirm dialog "Discard changes?" with Discard, which closes and proceeds, and Keep editing), `patchDraft` (project, note, tag), `setDraftSpan(i, span)` and `removeDraftSpan(i)` (shown at once, then as `recordingDraftSpans` rolls them), `draftDrag`/`setDraftDrag` (`{ index, span }` while a `DraftBlock` edge is dragged, on ticks: the block and `EditorSpans` show it; no IPC), `resizeDraftSpan(i, span, moving)` (on release: `recordingDraftResize` keeps the ticks and merges the spans the span then touches or overlaps, then clears `draftDrag`), `addDraftSpan` (`recordingDraftAdd`), `setDraftTotal(ms)` (`recordingDraftTotal`); only the latest ask's answer is kept. `recordSelection()` (snaps the selection through `api.recordingSnap`, a no-op for a drag's, stores it as `selectionSnap`, then opens a new editor on it, or adds it to the open editor's spans through `recordingDraftInclude`, which merges what touches), `toggleBlock(blockId)` (a click on a block: without an editor it opens one on the block's span; with one, `recordingDraftToggle` adds the block's span or takes it out, and taking out the last span closes the editor; not while the running timer's recording is edited), `answerOverlap(replace)` |
| `appIcons` | `appIcons` (a PNG data URL per bundle id, `null` once known to have none), `requestAppIcon(bundleId)`: asks Rust once per id for the app's run; the asks of one task go out as one `api.appIcons` call, and a failure leaves the initials |
| `viewState` | `uiState` (`UiState` from Rust), `setPaneSize`, `toggleSidebar`, `rememberDay`, `setTimelineZoom` (px per minute, `TIMELINE_ZOOM` 1–8, default 2.5), `finishOnboarding`, `showWelcome`; a debounced writer saves it, `flushUiState` writes at once |

Components select narrowly (`useStore((s) => s.view)`), never the whole store. `lib/useApiResult` loads a
value from `api` keyed on its inputs (and on `selectDayVersion(day)` or `dataVersion`), so a `data_changed` event
reloads every view showing that data.

Rust sends dates as text (`DayView.label`, `WeekView.label`, `ReportResult.title`, `ReportRow.dayLabel`,
`DateJump.label`, …) in the macOS region's date order; `lib/day.ts` keeps only `today` and the order-free
`formatMonthLabel`. Durations the page shows from milliseconds go through `formatDuration` (`lib/format.ts`,
`1h 05m`), which mirrors Rust's `format_duration`; clock times through `formatClock`
([ux.md](ux.md#formats-and-words)).

## API boundary

Only `api.ts` files import `@tauri-apps/api` or `@tauri-apps/plugin-*`: the core's is `src/api/index.ts`, an
optional module's is `src/<module>/api.ts` ([ADR 0002](adr/0002-ipc-in-api-files.md)); `test/boundary.test.ts`
enforces it. `src/api/index.ts` exports `api`, a flat object of thin `invoke` wrappers for the commands in
[backend.md](backend.md#tauri-commands) plus the subscriptions `onLaunch`, `onCommand`, `onRemote`,
`onTrackerStatus` and `onDataChanged`, and `toAppError`, which normalizes rejections to `AppError`.

The IPC types are generated from Rust with ts-rs ([ADR 0004](adr/0004-generated-ipc-types.md)) into
`src/api/types.gen.ts`; never edit it. `src/api/types.ts` re-exports them plus the frontend-only constants:
`ACCENTS` (mirrors Rust's list; a Rust test compares them), `TEXT_SIZES` and `SETTINGS_LIMITS`
(mirror the clamps in `Settings::sanitized`, the source of truth), `LOG_LEVELS`, `BUNDLED_FONTS`,
`Keybindings` and `EMPTY_UI_STATE`.

## Commands

Every user action that has a shortcut or appears in the palette is a `Command` (`commands/registry.ts`):
`{ id, title, scope, defaultBinding?, when?, run, menu?, checked?, menuOnly? }`; `menuOnly` marks a menu
item that is not meant to take a key (Launch at login, Show welcome again), which Settings → Shortcuts leaves
out. The core commands are `APP_COMMANDS` (`commands/index.ts`); their default bindings are in
[ux.md](ux.md#keys).

- **Bindings** are strings like `Mod+Shift+K` (`Mod` is ⌘), normalized to `Mod, Ctrl, Alt, Shift` and shown
  as `⌘⇧K`. User overrides come from `keybindings.json`.
- **Scopes** (`commands/scopes.ts`) form a tree: `global`; under it `dated` (with `day`, `week`, `month`
  and `reports`), `projects` and `rules`, each active while `view` matches. The most specific active scope wins a key.
- **Keys.** `handleKeydown` runs in the capture phase. Inside text inputs only `Mod`/`Ctrl` combinations and
  Escape fire. While a dialog, the palette, the editor or a context menu (`[role="menu"]`) has the keys only
  `MODAL_ALLOWED` commands run. Bare ↵ and Space on a focused button or link stay with it
  (`activatesControl`); the timeline's focus commands blur such a button first (`releaseControlFocus`), so
  the keys return to the timeline. Auto-repeat is swallowed unless the command is in `REPEATABLE` (zoom,
  nudging).
- **Menu.** Native menu and tray items emit `command`; `runMenuCommand` runs the id when `canRunFromMenu`
  says so (an active scope, `when` not false, not held back by an overlay). A command with `menu: true` has
  the same id and default as its `MENU_ITEMS` entry in `src-tauri/src/menu/items.rs`; `checked` gives a
  check item its mark. `installMenuSync` (`commands/menu.ts`, started in `App`) reports `menuState()`, the
  menu commands that can run now and the checked ones, through `api.menuSync` on every store change and
  every focus change (the text-field Undo follows the keyboard focus); `syncMenu` sends
  only a changed state, so Rust greys out and checks the items.
- **Palette.** `CommandPalette` (the sheet, ↑ ↓ ↵ ⌘↵) with `components/palette/`: `parse.ts` (`parsePrefix`:
  `>` commands, `p:` projects, `c:` clients, `@` dates; `score`: 5 equal, 4 prefix, 3 a word start, 2 inside a
  word; `fuzzyScore` adds 1 for the characters in order, for project and client names), `usePaletteItems.ts`
  (one provider per kind: views, dates, settings pages, projects, clients, saved reports, commands; each section's
  matches best first, the sections by their best match, ties in that order) and `PaletteList.tsx` (section labels,
  rows, `PaletteFooter`). Dates come from Rust (`useDateJumps.ts`: `api.calendarJumps`, `calendar::jump`, each with its `label`), so the page parses and labels no
  dates; a date opens the Day, Week or Month view at it (`setView`, `showDate`). A project opens Reports filtered
  to it for the report's period (`setReportFilter`), ⌘↵ starts a timer on it (`startTimer`); a client opens
  Reports filtered to it; a saved report `openSavedReport`; a settings page `openDialog({ type: "settings", page })`.
  Commands are those of the active scopes whose `when` holds, with their key caps; without `>` the `view.*`
  commands are left to the views.
- **Day, Week, Month and Reports** (scope `dated`, the parent of the four views' scopes). `nav.prev` (P),
  `nav.next` (N) step by the view's unit (in Reports the report's period); `nav.today` (O). The registry
  holds one binding per command, so ⌘← ⌘→ are gone.
- **Day view.** `day.nextBlock` (J),
  `day.prevBlock` (K), `day.prevLane` (H), `day.nextLane` (L), and with a focused block `day.details`
  (Space), `day.blockMenu` (⌃↵, which also offers Make a rule…), `day.selectBlock` (V, its span as the
  selection); `day.extendNext`/`extendPrev` (⇧J ⇧K) move the focus and grow the selection over the block;
  `day.clearFocus` (Escape) clears the block focus, the selection and the focused recording.
  `activity.deleteDay` and `activity.deleteRange` (no default keys; also in the Day toolbar's ⋯ menu) open
  `DeleteActivityDialog` once the shown day's view has loaded. `commands/suggestions.ts` holds the
  suggestion commands and registers the `suggestion` scope (beside `day`, active while `focusedSuggestion`
  is a suggestion of the shown day and nothing else has the focus): `suggestion.approve` (↵),
  `suggestion.edit` (E), `suggestion.dismiss` (⌫), `suggestion.approveRule`; and in `day`, while the day has
  suggestions, `suggestion.approveAll` and `suggestion.dismissAll` (the shown ones). `nav.goToDate` opens the date picker
  (`DatePickerPopover`).
- **Recordings.** `recording.next`/`prev` (] [, `stepEntry`) focus the shown day's timesheet entries in order, recordings and suggestions (`sheetEntries`, `lib/sheet.ts`);
  `recording.open` (Enter) edits the focused recording or opens the editor on the selection (`recordSelection`);
  `recording.create` (Record Selection) and `recording.edit` (menu and palette) do one of the two;
  `recording.new` (File → New Recording…, from any view) opens the editor on the Day view's selection, else
  for the span `recording_default_span` proposes (the half hour up to now) on today's Day view (or this week's Week view, this month's Month view; `openNewRecording` in the `editor` slice);
  `recording.delete` (⌫);
  `recording.nudgeEarlier`/`nudgeLater` (⌥↑ ⌥↓) move the focused recording's first span of the day and
  `recording.extend`/`shrink` (⌥⇧↓ ⌥⇧↑) change its end, by one snap step (`nudgeSpan`: the page sends step counts, `recording_nudge` computes the times);
  `recording.moveToPrevDay`/`moveToNextDay` (⌥← ⌥→) and `recording.copyToPrevDay`/`copyToNextDay` (⌥⇧← ⌥⇧→)
  take the whole recording to the adjacent day at the same time. `edit.undo` (⌘Z) and `edit.redo` (⌘⇧Z) are global
  and replace the native Edit items: in a text field they undo its typing (`document.execCommand`), also in
  dialogs and the editor; elsewhere the last recording change, but not while an overlay is open. The
  palette's search field is left out, so the palette never lists them for it. While the editor is open, only
  `MODAL_ALLOWED` commands run. `zoom.in`/`zoom.out`/`zoom.reset` (⌘= ⌘- ⌘0)
  zoom the timeline (`TIMELINE_STEPS`) while the Day view shows and elsewhere step the text size through
  `TEXT_SIZES` (Small, Medium, Large; ⌘0 is Medium; past either end nothing changes).
- **Projects.** `projects.new` (⌘N) and `clients.new` (no default) open the sheets while the Projects view
  shows; `view.projects` (⌘5) shows it.
- **Tags.** `tags.new` (⌘N while the Tags view shows) asks for a name (`prompt` dialog) and saves it with `saveTag`.
- **Hours.** The sidebar's Hours runs `showHours`; there is no `view.hours` command, menu item or palette entry,
  since `nav.today` (O) and the palette's `Today` date word do the same.
- **Reports.** `reports.copyText` (⌘⇧C), `reports.copyNotes` (⌥⌘C: every note of the report, `result.notes`),
  `reports.save`; `reports.rename`/`reports.delete` act on the saved
  report opened from the sidebar (`openedReport`, cleared when the filter changes).
- **Rules.** `rules.new` (⌘N); `rules.moveUp`/`moveDown` (⌥↑ ⌥↓) move the rule whose row has the focus
  (`moveRule` in the rules slice).
- **Tracker.** `tracker.pause` (until resumed; while tracking) and `tracker.resume` (while paused), no default
  bindings.
- **Window.** `window.hide` calls `api.hideWindow`; `app.quit` is handled by Rust when it comes from the menu.
- **App and help.** `app.launchAtLogin` switches the LaunchAgent (`setLaunchAtLogin`, checked by
  `settings.launchAtLogin`); `welcome.show` shows the welcome sheet again; `data.exportBackup` and
  `data.reveal` run the Settings → Data actions (`components/settings/actions.ts`); `help.readme` calls
  `api.openReadme`.

## Components

Shell components in `components/`: `Toolbar` (the unified toolbar every view renders first: `title`, an
optional `detail` and the actions as children; a drag region), `Sidebar` (a drag strip
under the window buttons, `MiniCalendar`, Hours, the pages of `views.ts` from Reports on, `SidebarStatus`), `StatusBar`, `EmptyView`, `Modal`
(`variant`: `sheet`, `panel`, `popover`, `bare`), `ContextMenu`, `Toasts`, `Field` (`InsetField`, `KeyCap`),
`Splitter`, `ErrorBoundary`, `CommandPalette` (`palette/`), `ShortcutsDialog`, `Dialogs`, `AppIcon`, and
`useAnchoredStyle` (a popover's fixed place under the element matching a selector, left- or right-aligned to it:
the date picker, View options).

`AppIcon` (`app`, `bundleId`, `size` 16 or 20): the installed app's icon from the `appIcons` slice, rounded,
or the app's initial while there is none (no bundle id, an external source, an app not installed). 16 px in
timeline blocks and app lane heads, 20 px in the hover card, rule conditions and Settings →
Timeline → Apps.

Design components ([ux.md](ux.md#components)):

- `Card` (`Card.tsx`): a recording filled in its project's color. Props `color` (a `shadeColor`), `title`,
  `subtitle`, `meta` (the foot's left side), `trailing` (its right side) and children (extras such as grips);
  other props go to the `div`.
- `Block` (`Block.tsx`): a block placed on the timeline by `top`, `height` and optional `left` and `width`,
  with `title`, `subtitle` and `meta`; without a `title` it shows no text (a tiny block). Children follow the
  text.
- `SegmentedControl` (`SegmentedControl.tsx`): mutually exclusive buttons (`aria-pressed`), labels
  capitalized; `settings/controls.tsx`'s `Segmented` puts one in a settings row.
- `AccentSwatches`, `ShadeSwatches`, `AccentDot` and `shadeColor` (`Accent.tsx`); a shade's lightness is
  `shadeLightness` (`lib/accent.ts`).

Dates are changed in one place, the sidebar's calendar (with P, N, O and Go to Date…):

- `views.ts` lists every view (`VIEWS`, in ⌘1 to ⌘7 order: Day, Week, Month, Reports, Projects, Tags,
  Rules). The sidebar lists the pages from Reports on, Projects, Tags and Rules after a separator; the
  calendar stands for Day, Week and Month. `Hours` calls `goToDay(today())`.
- `shownPeriod.ts`: `useCalendarPeriod` (the Day, Week or Month view's period from Rust,
  `api.calendarPeriod(unit, day)`), `useReportRange`, `useShown` (what the calendar highlights: a `Shown` of
  `period` with the view's unit and day, `report` with the range, or `none`) and `useShownDay` (the day whose
  month the calendar opens on).
- `MiniCalendar` (top of the sidebar): `api.calendarMiniMonth(shownDay, offset, shown)`, keyed on those,
  today and `changeCount`; Rust picks the month. Rust returns six Monday-first weeks (`MiniWeek { number, start, selected, days }`,
  `MiniDay { day, dayOfMonth, inMonth, isToday, hasRecordings, selected }`), the title (`monthName`, `year`)
  and `monthSelected` (the month name short or full by `settings.calendarTitle`); the page only draws them. A day calls `goToDay`, a week number `setView("week")` and
  `showDate(start)`, the title `setView("month")` and `showDate(monthStart)`. ‹ › change a local month `offset`, which
  starts over at 0 whenever the shown day changes. The title carries `data-date-heading`, so the date picker
  opens under it; the cells are `tabIndex={-1}` (the keys are P, N, O, ⌘1 to ⌘3 and the picker).
- `DatePickerPopover` (dialog `datePicker`, opened by Go to Date…; anchored under `[data-date-heading]`, the calendar's title, and centered while the sidebar is hidden): the
  month grid from Rust (`api.calendarPicker(month, selected)`: whole weeks from Monday, each day with
  `outside`, `isToday` and `selected`), today in accent text, the shown period (or a custom report range) on
  `--sel`. ‹ › and PageUp / PageDown turn the month, arrow keys move the focused day, ↵ or a click calls
  `showDate` and closes it.
- `StatusBar`: the sidebar toggle, `TrackerStatus` (only while `uiState.sidebarHidden`) and the selection on the left; the running timer
  and `ShownPeriodTotals` on the right: `api.periodTotals(range)` for the shown period (the report's range in
  Reports, the `useCalendarPeriod()` days in Day, Week and Month; nothing in Projects, Tags and Rules), keyed on
  `changeCount` and the `useLiveReload` tick, as `This week · recorded 1:30 · Billable 67 %` (`aria-label`
  "Shown period"; the label comes from Rust; the billable part is a `role="meter"`, left out while
  `billablePercent` is `null`).

Tracker components:

- `SidebarStatus` (bottom of the sidebar): `TrackerDot`, the status text, and a play / pause button (Pause
  pauses until resumed, a right-click offers the `PAUSES` durations; Play resumes; Allow… without
  permission), then the Settings gear (`settings.toggle`).
- `TrackerStatus` (status bar, left, while the sidebar is hidden): a dot (accent while running, `--o3` while away or paused, `--style-red`
  without permission), the status text Rust computed (the tray shows the same), and Pause (a `ContextMenu`:
  15 minutes, 1 hour, until tomorrow, until resumed) while running or away, Resume while paused, and
  "Allow…" (shows the permission dialog again) without permission. "Tracking off" while `tracker` is `null`.
- `PermissionDialog`: a `Modal` while the state is `needsPermission`. It explains why Accessibility is needed
  and what is never captured; "Open System Settings" (`openPrivacySettings`) and "Check again"
  (`requestPermission`). It disappears once the status is no longer `needsPermission`. Escape hides it
  (`permissionDialogHidden` in the `tracker` slice) until "Allow…" or until the permission is lost again.
  It stays hidden while the welcome sheet shows, which asks for the same permission.
- `onboarding/Welcome.tsx`: the first-run `Modal` sheet, shown while `uiState.onboardingDone` is false
  (`EMPTY_UI_STATE` says true, so it never flashes before the state loads). Three panels with Back and Next
  ("1 of 3"): what is recorded and never recorded, with the data folder (`dataInfo().dir`); Accessibility,
  with "Grant access" (`requestPermission`), a live status line from `tracker.permission` and "Skip for now"
  (Next is enabled once granted or skipped); `LaunchAtLoginSwitch` and the global timer shortcut
  (`settings.timerShortcut`). Done, and also Escape, call `finishOnboarding` (`viewState` slice: patches
  `onboardingDone` and calls `api.onboardingDone`). Each showing mounts afresh on the first panel.

### Day view

`components/day/` renders `DayView` from Rust; it groups, packs and sums nothing.

| File | Renders |
|---|---|
| `DayPage` | `DayHeader`, then the `Timesheet` pane (width from `usePaneSize("timesheet")`, about a quarter of the window by default, 220–560 px, a `Splitter` beside it) and `Timeline`; a day without activity and recordings keeps the timeline (hour axis, time column, lanes: drag and + still record) and passes `Timeline` its `empty` message ("Nothing tracked on this day" with the tracker's status text on today, or "Tracking needs permission" with Allow…), drawn over the lanes in `.lanes-empty`, which lets presses through. Loads on a new day, on `selectDayVersion(day)`, and while today on `useLiveReload` (every 30 s, and on tracker statuses at most every 10 s, since the tracker sends no `data_changed`; paused while `document.visibilityState` is hidden, which WKWebView sets when the window hides, with one reload when it shows again) |
| `DayHeader` | a slim bar: the date (`label`, `DayView.label` from Rust in the `dayTitle` format; the last loaded one stays while the next day loads), `RecordedMark` (a green `CircleCheck` "Recorded" once `recordedMs` > 0, else a grey dot "Nothing recorded"), totals `recorded 1h 00m · tracked 6h 12m` (`formatDuration`, parts that are zero left out, nothing when both are; `awayMs` is not shown), the View options button (dialog `viewOptions`, `[data-view-options]`; command `day.settings`, no default key), and the ⋯ menu: Delete activity for this day…, Delete activity in a range… |
| `DeleteActivityDialog` | dialog `deleteActivity`, a `Modal` sheet: the day (its `dayView.bounds`, titled with `dayView.label`) or two `datetime-local` fields prefilled with them, the sentence "Deletes 412 activities and 3 away times. Activity and away time that cross the range are deleted entirely. Recordings are not affected." (counts and the range's `rangeLabel` from `api.activityCount`), errors inline, and Delete (`api.activityDelete`). Then a toast `Deleted activity for Wednesday, October 21, 2026` (a range: `Wed, Oct 21 09:30 – Thu, Oct 22 10:00`) whose Undo (`api.activityDeleteUndo`) lasts until `undoUntil`; the Day view reloads from `data_changed` |
| `Timesheet` | the left pane: "Timesheet", the day's `totals.recordedMs` (`formatDuration`, `6h 05m`), + (`runCommand("recording.new")`; not rendered while an editor is open), then a `TimesheetCard` per `view.timesheet` entry in Rust's order, or an empty line. The open `RecordingEditor` sits in place of the card it edits, or for a new recording before the first card that starts after its first span (`editorSlot`), in the same keyed list as the cards so it moves without mounting again |
| `Timesheet` (suggestions) | `sheetEntries(view)` (`lib/sheet.ts`) puts each suggestion before its `nextCard`; the editor replaces the card or suggestion it edits; over the cards `ReviewLine`: "n suggestions · Approve all · Dismiss all" |
| `TimesheetCard` | a `Card` (role button) in the project's shade: the note as title (the project without one), the `AccentDot` and `Project \| Client`, the tag (`TagLabel`: its emoji, else the tag icon, and its name) and `n times`, `durationMs`; its height grows with the duration (`--card-minutes`). Click or ↵ focuses the recording and opens the editor on it, hover `hoverRecording` |
| `ZoomControl` | −, a range input (`TIMELINE_ZOOM`, step 0.5) and +: `stepTimelineZoom` and `setTimelineZoom` |
| `ViewOptionsPopover` | dialog `viewOptions`, a `Modal` popover anchored under `[data-view-options]`: Lanes (Summary `summaryLane`, Activity `showAllActivity`), Activity (Compact, Titles; `TITLES` from `timelineActions.ts`), App lanes (an `ownLane` switch per `dayView.apps` entry, `setAppSetting`) and Hidden apps (`dayView.hiddenApps` with Unhide; the count in the heading only above 0). Join short gaps, Hide activity shorter than, Snapping and the summary interval are only in Settings → Timeline |
| `Timeline` | `timeline-frame`: the scroll box (both ways: lanes keep their minimum widths, `laneCols` from `Lane.tsx` sets `--lane-cols` on a lane and its head; the axis and time column stick left) with `LaneHeader` (sticky; a pinned app lane's head is its icon, named by `aria-label`), `TimeAxis`, the time column (`RecordingsColumn`), the lanes and the current-time line on today; `ZoomControl` floats over its bottom right. Over the lanes, a `selection` band with no visible text per open editor's span (`drawnParts`, cut to the day) and one for the snapped selection while it is not one of them (labelled `Selection 09:00–10:30`); the blocks of `draftBlockIds` (with an editor open) or of the selection's `blockIds` are passed to each `Lane` as `selectedBlocks`. Opens a day scrolled to 30 min before its first activity. Hover (one `mouseover` listener reading `data-block-id`) shows `HoverCard` after 300 ms; right-click, the gear on a hovered block, or ⌃↵ opens the block menu. With `compact` it draws the day at 60% of the zoom and adds `timeline--compact` |
| `blockMenu.ts` | the block menu's entries: for tracker blocks Own lane (checked in an app lane or with the override) and Hide (a toast with Undo); then Copy title (not for private blocks), Copy URL |
| `Lane` | its blocks, absolutely placed: `top`/`height` from the times and the zoom, `left`/`width` from `col`/`cols`; only blocks within the visible part ± one screen (`useVisibleRange`, which moves in half-screen steps). The summary lane draws `summary` blocks, the others `memory` blocks |
| `ActivityBlock` | `look="summary"`: a `Block` with the `AppIcon` (16 px) and the label as title and `activeMs` (`formatDuration`) at the bottom right when tall; `look="memory"`: the icon over a bar of the block's real height, the label beside it and `activeMs` under it (the label alone when short, only the bar when tiny), a gear on hover in the row's flow; `look="pinned"` (app lanes): the icon and `activeMs` only; `private`, `dimmed` (covered), `focused` and `selected` (its span is in the selection or the draft) classes |
| `HoverCard` | `AppIcon` and app, title, URL (http(s) opens through `api.openUrl`) or path, time and duration, the source when external; beside the block, else below or above it |
| `RecordingsColumn` | the time column: the day's `SuggestionBlock`s (the edited suggestion gives way to the draft) under its `RecordingBlock`s (`hovered` from `hoveredRecording`), while a span is selected a dashed outline where its recording would go, and a `DraftBlock` per open editor's span, in place of the edited recording's own blocks. A press on empty time is `useSelectionPress`'s |
| `SuggestionBlock` | a rule's suggestion in the time column: hatched in the project's fill, outlined dashed, with the project, the rule and the duration; `focused` adds the accent ring; a click `editSuggestion` |
| `SuggestionCard` | a suggestion in the timesheet: a ghost `Card` (role group, Tab stop; focusing it `focusSuggestion`), note or project as title, project and client, "Suggested by <rule>" and the tag, the duration; ✓ `approveSuggestion`, ✕ `dismissSuggestions([id])`, a click `editSuggestion` |
| `useRuleDrag` | ⌥-pressing an activity block (in `Timeline`, while no editor is open) shows the recent projects over the top of the recordings column; releasing over one opens the `rule` dialog with `draftFromBlock`: `<app> → <project>`, `app is <app>` and `domain is <host>` for a URL block, else `title contains <title>` |
| `DraftBlock` | one of the open editor's spans in the time column: a filled accent block cut to the day (`drawnParts`) with its start and end, presses on its body falling through to the column; a grip on each edge inside the day (a short accent bar) drags that end from tick to tick (`tickPointer`, `spanWithEdge`; the other end keeps its time) into `draftDrag`, and release sends that span to `resizeDraftSpan`, which keeps it drawn until Rust's answer |
| `RecordingBlock` | one span of a recording in the Day view's time column (`spanId`; a recording with several spans has several blocks, all focused and hovered together): a `Card` in the project's shade with only its start and end. Dashed top or bottom edge when clipped to the day. A click focuses the recording and opens the editor. Hovering sets `hoverRecording`; dragging the body moves the span by whole ticks (`spanMoved`, from the tick pressed on) and an edge resizes it (`spanWithEdge`; the other edge keeps its time), the pointer read through `tickPointer` (`pointerDrag`, pointer capture). The block and its times follow the ticks with no IPC; release sends the span, if it changed, and the dragged edge as `moving` to `updateSpan`, and the block keeps the dragged span until Rust's new times arrive (or the write fails) ; the tag's emoji (`tagEmoji`) at the lower left once the block shows both times |
| `RecordingEditor` | the editor as a form card in the timesheet (in the Week and Month views inside `EditorSheet`), on the `editor` slice's `draft`: the note as the title field (autofocus; one line, pasted line breaks become spaces), then full-width fields without labels (`EditorField`; the ones without a picker frame as `EditorLine`s behind an icon): project (`ProjectPicker` with `showRecent`), tag (`TagPicker`, only once a project is picked or the draft has a tag), the duration (`LoggedTime`, `0 h 20 m` from `draft.totalMs`; a change goes to `setDraftTotal`), the spans (`EditorSpans`, one `[start] – [end]` line each, a clock before the first) in `editor-section`s split by hairlines (Logged time is its own tinted band), then the buttons Save (↵ or ⌘↵, the default button) and Cancel (Escape, wherever the focus is) at the leading end and Delete (⌘⌫) at the trailing end; Save and Delete come from `useEditorSave` (every span as Rust rolled it; a running recording keeps its spans and sends only the fields; field errors under their field). Opened on a suggestion it shows "Suggested by <rule>. Approve records it.", the submit reads Approve and saves through `createRecording(input, suggestionId)`. Its form is named "New recording", "Edit suggestion" or "Edit recording" (`id` lets `EditorSheet` point at it). Beside the note, Autofill (`useAutofill`: `api.recordingAutofill` once the note settles for 250 ms) shows while Rust's project or tag differ from the draft's and patches both in. A new recording starts without a project; `useProposal`'s project is only offered (`suggested`, first among the recent rows). Scrolls into view when it opens |
| `EditorSpans` | the draft's spans as `[start] – [end]` lines (a clock icon before the first) (`TimeField`: `HH:MM`, ↑↓ by the snap step), each with ✕ while there are several, and "Add time" (`addDraftSpan`: Rust places it after the last; the rows are labelled `Time 1 from`, and ✕ `Remove time 09:00–10:30`); a running recording's spans are shown, not edited |
| `EditorParts` | `LoggedTime` ("Logged time": `DurationField` and the 15m / 30m / 1h 00m buttons, group "Set duration", that set the length through `onChange`; none while read-only), `DurationField`, `EditorField` (a full-width control and its error), `EditorLine` (an `EditorField` behind an icon) and `OverlapNotice` (the recordings in the way, "Replace overlapped part" or Back, inside the editor) |
| `useProposal` | the project of `api.recordingProposal` for the draft's spans once they settle (150 ms) |
| `useEditorSave` | the editor's `save`, `remove` and field `errors` |
| `TimeField` | `HH:MM` typed, read by Rust on the value's day (`calendarParseTime`; each valid time counts once it answers) or ↑↓ by the snap step; the editor's save and the away prompt's Record first await `typedTimesSettled` (every pending read and the `onChange` work it started, such as `setDraftSpan`), so ↵ right after typing saves the typed time; the typed text shows while it belongs to the value, else the value |
| `OverlapDialog` | the recordings in the way, "Replace overlapped part" or Cancel, for writes made while no editor is open |
| `tickPointer.ts` | the event wrapper every timeline drag reads the pointer through ([ADR 0026](adr/0026-timeline-pointer-input-snaps-to-ticks.md)): `TickGrid` (`DayView.ticks` and the timeline's `y`), `tickPointer(grid, el)` (a pointer's `clientY` to `{ index, above }`, the tick drawn nearest it, measured from `.timeline-body`), `tickNearPx`, `tickNear(grid, ms)` (the tick drawn nearest a time the day holds). Pixel geometry only: no arithmetic on times |
| `spanDrag.ts` | spans from ticks: `selectedSpan(ticks, origin, at)` (origin tick to the pointer's, one tick toward the pointer when they are the same), `spanWithEdge(ticks, span, edge, index)` (that edge on the tick, at least one tick past the other edge, which keeps its time), `spanMoved(grid, span, by)` (both edges `by` ticks on, kept inside the grid), `sameSpan`; `Grip` |
| `useSelectionPress` | the press on the lanes, the axis and the time column: a drag selects `selectedSpan` from the tick pressed on (fixed for the drag) to the pointer's tick (auto-scrolling near the edges) and opens the editor on release; a click on a block toggles its span (`toggleBlock`), a click on nothing clears the selection. No modifier changes a click |

`lib/blockFocus.ts`: `stepBlock` (time order across lanes) and `stepLane` (the nearest block in the next
lane with blocks) for J/K/H/L. The status bar shows the selection as `09:00–10:30 · 1:30`.

### Timer

The `timer` slice holds `timer` (`TimerStatus` from `timer_status` events and `loadTimer` at boot),
`startTimer` (an `overlap` reads "A recording already covers now"), `updateTimer` (the running timer's
project, note and tag) and `stopTimer` (a toast with Undo).
Commands `timer.start` (⌘T: `timerPickerOpen`), `timer.stop` (⌘⇧. while one runs), `timer.toggle`. The status
bar shows the running timer. Settings → Shortcuts has a Global shortcut group above the in-app commands: the
recorder saves `timerShortcut` through `saveTimerShortcut`, which shows Rust's refusal under the row. Every
bound shortcut, global or in-app, has an X ("Remove shortcut for …") that saves it as `null`.

Secondary windows are pages of their own in `src/windows/<name>/` (`index.html`, `main.tsx`), each a
`build.rollupOptions.input` entry in `vite.config.ts`; they import `src/styles/index.ts` and use the same
store and `api`. `windows/timer/TimerPicker.tsx` reloads settings, projects, the timer and the recent timers
whenever the window gains focus (it is hidden, not closed, between uses), and reports its errors to the log.
Its `TimerForm` ("Start Timer") and the away window use the editor's field order with `FormField` labels:
Note (`NoteInput`, autofocus, `NOTE_PLACEHOLDER` "What did you work on?"), Project, Tag. While a timer runs the
form is filled with it ("Timer running", elapsed time, the note selected on focus) and Save calls `updateTimer`;
it is keyed by the recording, its `updatedAt` and the window's openings, so the minute ticks never reset it.

### Away window

`windows/away/AwayPrompt.tsx`, the `away` window's page. It loads settings, projects and
`awayPromptCurrent` on mount and on focus, and refills from `away_prompt_changed` (the form is keyed by the
span, so a refresh of the same span keeps what was typed). The header shows `awayMs` and `endedAt`; From and
To go through `recordingDraftSpans`, which rolls an end at or before the start to the next day and gives the
length. Record sends the drafted spans with `awayPromptAnswer`; an `overlap`
shows which recordings are in the way and keeps the form; Escape or Skip sends `awayPromptSkip`. It has no
toasts: unexpected errors are logged and shown inline. Settings → Tracker has the away prompt's switch and
its minimum length.

### Rules screen

`components/rules/`, the `rules` view (sidebar entry Rules, ⌘7). The `rules` slice holds `rules` (by
position) and their writes; a `data_changed` of kind `rules` reloads them.

- `RulesView` lists `RuleRow`s in order: a grip (a pointer drag by whole rows; on release `reorderRules`
  sends every id in the new order, shown at once and restored when the write fails), the enable switch
  (`setRuleEnabled`), the name, the project and `conditionSummary` (`app is "Chrome" or is "Safari" · title contains "x"`).
  A row click, New rule or ⌘N (`rules.new`) opens the `rule` dialog. Without rules a centered empty state
  ("No rules yet", the ⌥-drag tip, a New rule button) replaces the intro and the list.
- `RuleSheet` (dialog `rule` with `id` or a `draft`): name, `ProjectPicker`, note, `TagPicker`, the
  conditions as `ConditionGroups` ("Conditions": one section per field from `groupConditions`, its +
  adding a value; each value a `ConditionRow` with comparison, value and remove, "or" before all but the first;
  quiet buttons below add a field not used yet; an `app` value naming a tracked app from `appsList()` shows its
  `AppIcon`, else the initial) and the enable switch. Save sends
  `saveRule`; a field error shows under its field, and `conditions.<i>.value` marks that row. Beside the
  form, `RulePreviewPanel` asks `rulePreview(input, 7)` through `useApiResult` (300 ms debounce, keeps the
  last result) once every condition has a value, and lists the hits with the total.

### Week and Month

`components/calendar/`: each view's `Toolbar` (the title, totals), `EditorSheet` (the editor as a `Modal` sheet
around `RecordingEditor`, only while `editorView` is the shown view: the Week and Month have no timesheet) and `entry.ts` (`editEntry`: the editor on an
entry's recording, found by its `start`/`end` as `at`; `entryName`). `WeekView` runs `weekView(day)` keyed by
the day and `changeCount`: day heads (`DayHead`: `Mon 19`, today as "Today" in accent, and the day's recorded
`h:mm`, left out at zero; a click opens the day) over seven columns in one scroll box, each the day's
`WeekCard`s in order and a "New recording" button (`openNewRecording(day)`), shown while the column is hovered
or the button has the keyboard focus. `WeekCard` is a `Card` (project, the note when Rust's `showNote` says it fits,
tag, the spans count when several, the day's `h:mm`) whose height is `heightUnits` times `--week-unit`:
a click focuses it (`focusRecording`), a double-click or ↵ `editEntry`, ⌫ `deleteRecording`. Dragged
(`pointerDrag`) it dims, the column under the pointer (`elementFromPoint`, `data-day`) gets an accent ring, and
released over another column it moves there, with ⌥ it is copied (`moveRecording`/`copyRecording` with the
card's day as `from`, in the `recordings` slice, which ask about overlaps like every write); a running one
does not drag. `MonthView` runs `monthView(day)`: a grid of week rows (`role="row"`, each as tall as its
fullest cell) of `DayCell`s: the weekday and date (a button that opens the day; today's number in an accent
circle), a + shown on hover or focus (`openNewRecording(day)`), the recorded `h:mm`, then an `EntryLine` per
recording (a dot in the project's shade, the note or the project, the `h:mm`; a click `editEntry`).
Weekend columns and cells (`isWeekend`) sit on `--bg-quiet`.

The editor's `edit` request may carry `at` (a span of the recording), so views other than the Day view open it.

### Reports screen

`components/reports/`: `ReportsView` (the `reports` view) runs `reportRun(filter)` keyed by the filter and
`changeCount`: a `Toolbar` with `result.title` (Rust, in the Day title's format), `CopyTextButton` (a split
button: Copy as text, `reports.copyText`, ⌘⇧C: `reportText` to the clipboard; its chevron, "Group copied text
by", opens a `useContextMenu` menu of checkable By project, tag, day, week, month that sets
`reportFilter.groupBy`) and Save report… (`reports.save`: the `prompt` dialog, then `saveReport`);
`ReportFilters`, one slim row (Period presets as a segmented control, Custom revealing dates, `MultiSelect`
for clients, projects and tags, Rounding, Include archived). Below, all rendered from `ReportResult` as Rust
built it: `ReportTotals` (the total large, recordings, projects, untagged time, `Billable 6:24 h · 80 %`),
two `ReportChart`s (SVG donuts of `result.charts`, by client and by tag, each with a By select for Client,
Project or Tag, the total and count in the center, percentages on the ring and a legend; colors through
`accentColor` in `Accent.tsx`), and `ReportTable` (`result.clients`: client rows, project rows that unfold
their recordings, Expand all / Collapse all; a recording row shows its day, all its spans of the day
`14:25–15:00, 16:05–20:15`, note and tag). A project row's Copy notes puts `project.notes.text` in the
clipboard and toasts `Copied 4 notes for Acme / Portal` (`copyNotes` in `actions.ts`, which also
holds the commands' actions). The sidebar
lists saved reports under Reports: a click opens one, right-click renames (prompt) or deletes (confirm).

`PromptDialog` (`dialog: prompt { title, label, initial, confirmLabel, onSubmit }`) asks for one line.

### Projects screen

`components/projects/`: `ProjectsPage` (the `projects` view; it reloads the projects on `changeCount`, so the
hours follow every recording): a `Toolbar` (Show archived, New client, New project), then an inset group per
client by name (`ClientHead` above it: its color dot, the name, which edits the client, the number of projects, the labels
This week and This month, and a `+` that creates a project for that client, except on archived clients;
"No projects" when empty), client-less projects under "No client" last. A
project row: color dot and name (click edits), `billable` (a `.secondary-label`) for a billable project, `last used 2 d
ago`, then `weekMs` and `monthMs` as `h:mm` in two right-aligned columns (`--o3` at zero); archived rows
(with Show archived) are dimmed with Restore. The row's context menu: Edit…, Archive or Restore, Copy id.

- `ProjectSheet` (`dialog: project`, an optional `clientId` preselects the client): name, client select
  with "New client…" (an inline name; Save creates the client first), the color (`AccentSwatches`, only
  without a client or with a new one) and the shade (`ShadeSwatches`; none pressed lets Rust pick), the Billable
  switch (`input.billable`; new projects start off), Archive or Restore for an existing project. `FormField` (`components/Field.tsx`) shows `AppError.field` errors
  under their field.
- `ClientSheet` (`dialog: client`): name, color (none pressed on a new client lets Rust pick); Archive
  (also archives its projects) or Restore.

### Tags screen

`components/tags/TagsPage.tsx`, the `tags` view (⌘6): a `Toolbar` (`Tags`, its description and New tag, which
runs `tags.new` (⌘N): a name prompt, then `saveTag`), then one row per tag from the `projects` slice's `tags`
(`TagRow`: `TagEmojiField` (the emoji in place: ↵ or blur saves through `setTagEmoji`, empty removes it, Escape restores; a field error shows under it), name, the number of recordings, Rename, Delete, the two shown on hover or focus within the row; a
confirm first when recordings carry it), or `No tags yet.` `TagRow` renames in place (`useTagRename`): a double click on the name, ↵ on the focused row or Rename (`Rename tag
<name>`) shows the `Tag name` field, focused and selected; ↵ or blur calls `saveTag(tag.id, name)`, Escape and
an empty or unchanged name cancel. A failure keeps the field open, with a field error (a taken name) shown
beside it; the row takes the focus back after ↵ or Escape.

### Pickers

`components/pickers/`, pure UI over the `projects` slice (they only call the api to create). Both are a
`PickerField` (`PickerField.tsx`): a combobox that shows the value (or a muted placeholder) and an optional muted `detail`
after it (the project's client), and opens the list on focus (Tab or a click). Typing (as the query), ↓ ↑, Space, a click on the focused field or the chevron
("Show projects", "Show tags") open it again (`PickerList`, a portal below the field, so a sheet's overflow
never clips it) on the current value, which carries a check (`aria-checked`); ↑ ↓ move, Enter picks, Esc
closes the list without closing the sheet around it and keeps the value; the list closes on blur. With the
list closed, Enter and Esc go to the form (save, cancel).

| Picker | Props | Behavior |
|---|---|---|
| `ProjectPicker` | `value: string \| null`, `onChange(projectId)`, `autoFocus?`, `showRecent?`, `suggested?` | while empty and with `showRecent`, up to 5 projects show below the field as rows to click ("Recent projects", named `Client / Name`, out of the tab order): `suggested` first, marked "Suggested", then the recent active ones; shows the chosen project as color dot + name with the client muted after it, else "Select project" with a hollow dot; empty query: every active project once, by client (no Recent section); typed: fuzzy matches, best first; the last option is always `New project "<query>"…`, which opens `ProjectSheet` prefilled and picks the saved project. Archived projects never show |
| `TagPicker` | `value: string \| null`, `onChange(tagId \| null)` | one tag or none ([ADR 0019](adr/0019-one-tag-per-recording.md)): the chosen tag in the field behind its emoji (else the tag icon), each option with its emoji, "No tag" as placeholder; the open list starts with "No tag", then every tag; typing filters; Enter picks the highlighted one or creates `New tag "<query>"` (`saveTag`) and picks it; ⌫ with the list closed or on an empty query removes the tag. The editor, `RuleSheet`, the timer picker and the away window use it |

### Settings pages

`components/settings/SettingsDialog.tsx` renders the pages registered in `registry/settingsPages.ts`;
`pages.tsx` registers the core ones (`registerCoreSettingsPages`), `controls.tsx` holds the shared rows:

| Page | Holds |
|---|---|
| App → General (order 0) | `LaunchAtLoginSwitch` (`LaunchAtLoginSwitch.tsx`, also on the welcome sheet): checked from `autostartStatus()` (the LaunchAgent, not the stored setting), changed through `setLaunchAtLogin` (`settings` slice: `api.autostartSet`, then the store's `settings.launchAtLogin` is updated so a later save keeps it); "Show welcome again" closes Settings and calls `showWelcome` |
| App → Appearance | "Light mode scheme" and "Dark mode scheme", stacked rows with three tiles each (`SchemePicker`, a miniature window per scheme through `schemeLook`, pressed for that mode's `lightScheme` or `darkScheme`), accent row (`AccentSwatches withSystem`: System first, then the nine hues), Dates (`calendarTitle`: short, full; `dayTitle`: long, short; segmented controls), Text size (`textSize`: small, medium, large; a segmented control over `TEXT_SIZES`) |
| App → Shortcuts | `ShortcutsPage`: every command with its binding except `menuOnly` ones; record, reset or unbind |
| Tracking → Capture | `CapturePage` (`CapturePage.tsx`): address and document switches, excluded apps (an "Add app…" picker listing `appsSeen(30)`), excluded websites (a field; Enter adds) |
| Tracking → Tracker | `TrackerPage`: away after (idle minutes), the away prompt switch and, `nested` under it, "For longer than" (a minutes popup as wide as the number field) |
| Tracking → Timeline | `TimelinePage` (`TimelinePage.tsx`): `TimelineControls` (summary lane and its interval slider, show all activity, compact, group by app, Join short gaps (`continuity`), Hide activity shorter than (`noise`), both labelled in minutes through `SegmentedControl`'s `labels`, titles, time snapping; each saved quietly), then Apps from `appsList()`: per app its `AppIcon` (by `appKey`), its name and tracked time (never the bundle id), Hidden and Own lane |
| System → Data | the data folder (a stacked Location row: Reveal in Finder and Move…, the path under them; Move…: `chooseFolder` → a `Modal` sheet that says the database is copied, tracking pauses meanwhile and the old files stay → `dataMove` with the Move button busy (`Moving…`, Escape ignored) → toast "Data moved to …", settings and the page's `DataInfo` reload; an `invalid_input` refusal shows under the row, other errors toast), counts, first activity, size on disk, Export backup… (save panel → `backupExport`, toast "Backup written"); an alert when the store could not be opened |

A change saves through `saveSettings` (`api.updateSettings`) and applies at once.

## Styling

The CSS is plain and global, imported by `styles/index.ts` in this order: the four palettes
`styles/palettes/{content,sidebar}-{light,dark}.css`, `styles/theme.css` (derived tokens, the sidebar's token
mapping, the block colors), then one stylesheet per layer, base to windows: `base.css` (reset, scrollbars,
text utilities), `controls.css` (the AppKit-sized controls of [ux.md](ux.md#controls): buttons, fields, popup
buttons, key caps, segmented control, switch, swatches, secondary labels), `shell.css` (grid, the unified
toolbar, splitter, status bar), `sidebar.css`, `overlays.css` (dialogs, menus, toasts, palette, hover
card, shortcut overview), `forms.css`, `pickers.css` (pickers, date picker, app picker), `settings.css`,
`projects.css`, `day.css` (timeline, axis, lanes, selection, editor), `blocks.css` (everything
filled: activity blocks, cards, recordings, suggestions, project chips), `reports.css`, `calendar.css`
(Week, Month), `rules.css` and `windows.css` (timer picker, away prompt). The import order is the
cascade. Icons come from `lucide-react`. Token names and roles are in [ux.md](ux.md#appearance-and-tokens).

- **Appearance.** `applyAppearance` picks `lightScheme` or `darkScheme` by `prefers-color-scheme` with
  `resolveLook` (`lib/theme.ts`; `useSystemDark` re-renders on a macOS switch) and sets `data-content` and
  `data-sidebar` on `<html>`, `data-accent` from `accent` (`system` maps `--accent` to WebKit's `AccentColor`
and `--on-accent` to `--on-system-accent`; `applyInitialLook` sets it before settings load), and
`--ui-font-size` and `--data-font-size` from the `textSize` step's entry in `TEXT_SIZES`. The native window
  theme is never set, so it follows macOS ([ADR 0022](adr/0022-a-scheme-per-system-mode.md)). The palettes match
  `[data-content="light|dark"]` and `[data-sidebar="light|dark"]`, so any element can preview a look.
- **Sidebar tokens.** The sidebar uses `--sidebar-*` only; inside `.sidebar`, `theme.css` maps `--text`, `--o2`,
  `--o3`, `--line`, `--fill` and `--sel` to the sidebar set, so shared controls (key caps, buttons)
  follow it. The accent hues come from the content palette of the sidebar's tone
  (`[data-sidebar="<tone>"] .sidebar` shares its rule), and `--accent` is re-derived there.
- **Block colors.** `.card`, `.suggestion` and `.project-chip` derive `--block-fill`, `--block-text` and
  `--block-meta` from `--rec` (inline, a `shadeColor`) and the palette's `--block-*` numbers.
  `styles/contrast.ts` mirrors that math (OKLCH to sRGB, WCAG contrast) for the scheme audit.
- **No color literals** (`#hex`, `rgb(`, `hsl(`) outside `styles/palettes/`; `styles/colors.test.ts` scans.
- **Sizing.** Chrome is in rem: `html { font-size: var(--ui-font-size) }`, so zoom scales it. Spacing uses
  `--space-*`, type the `--fs-*` scale.
- **macOS look** ([ADR 0024](adr/0024-native-macos-look.md)). The sidebar spans the window's height under the
  traffic lights; each view's `Toolbar` is the 52px row beside them (`trafficLightPosition` 20/28 in
  `tauri.conf.json` centers them in it); without the sidebar the toolbar leaves `--traffic-lights` for them.
  `data-tauri-drag-region` marks draggable areas (the toolbar, its title and spacer, the sidebar's top
  strip); `user-select: none` applies globally, inputs and `.selectable` opt back in. The body is
  transparent; `sidebar.css` makes the sidebar transparent where its tone matches `prefers-color-scheme`, so
  the window's `sidebar` effect (vibrancy) shows through.
- **Groups.** Grouped content is an inset group (`--fill-quiet`, `--radius`, no border; one shared rule in `base.css`): `.settings-card`,
  `.project-rows` and `.tag-rows`, `.rule-rows`, `.report-totals`, `.report-chart`, `.report-table`.

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
  literals outside palettes).
- **Style.** Query by role or text, drive the UI with `userEvent`, and assert what is visible or which API
  call was made. Reset with `resetStore()` in `beforeEach`.
