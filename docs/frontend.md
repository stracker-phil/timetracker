# Frontend

`src/` holds a React 19 + TypeScript app, bundled by Vite. State lives in one zustand store. The frontend
renders and forwards intent; [Rust owns the logic](architecture.md#design-rules).

## Boot

1. `index.html` loads `main.tsx`.
2. `main.tsx` calls `installModules()` (`src/modules.ts`) before the first render:
   `registerCoreSettingsPages()` (`components/settings/pages.tsx`), then `installCommands()` (core scopes,
   then the command registry).
3. `applyInitialLook()` (`lib/theme.ts`) sets the default look (mode System, scheme Default) for the OS
   appearance, so the first paint has colors before settings load.
4. `installNativeMenuGuard()` (`lib/nativeMenu.ts`): a right-click that no app menu handled shows nothing,
   so the WebView's Reload and Inspect Element never appear; text fields keep Cut/Copy/Paste.
5. `main.tsx` assigns the store to `window.timetracker` only when `import.meta.env.VITE_DEV_HOOKS` is `"1"`
   (the debug bundle of `bun run build:app`), then renders `<ErrorBoundary><App/></ErrorBoundary>`.
6. `App.tsx` imports the stylesheets (`styles/index.ts`: fonts, the palettes, `theme.css`, then the
   stylesheets by layer) and calls
   `useBackendEvents()` once:
   1. `loadUiState()` loads the window layout, then `restoreDay()` shows the last viewed day (within 30
      days). `loadSettings()` loads settings and keybindings; `installCommands()` runs again to apply the user's
      bindings, then `drainLaunches()` pulls the queued launch events (`take_launch_events`): `error` shows an error toast,
      `notice` an info toast (a migration that dropped tags), `showDay` calls `goToDay`.
   2. It subscribes to `launch` (drain again), `command` (menu and tray → `runMenuCommand`), `remote`
      (→ `answerRemote`, which always replies with `remoteReply`), `data_changed` (→ `dataChanged`) and
      `tracker_status` (→ `setTracker`); `loadTracker()` fetches the current status once.
   3. It adds a capture-phase `keydown` → `handleKeydown`, `error` / `unhandledrejection` → error toast, and
      `beforeunload` → `flushUiState`.
7. `applyAppearance(settings, systemDark)` runs whenever settings or the OS appearance change.

```
App
└─ .app grid
   ├─ Toolbar        drag strip: traffic-light inset, app title, Settings button
   ├─ Sidebar        DateNavigator (the shown period, ‹ › Today), then Day, Week, Month, Reports, Projects, Rules
   ├─ main           the active view's page (EmptyView for an unknown one)
   └─ StatusBar      left: sidebar toggle, TrackerStatus, the selection; right: running timer, billable ratio, today's totals
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
| `nav` | `view` (`day`, `week`, `month`, `reports`, `projects`, `rules`), `setView`, `day` (the shown `YYYY-MM-DD`, today at start), `goToDay(day)` (also switches to the Day view; `timetracker show` uses it), `goToday` (keeps the view), `showDate(day)` (keeps the view; the date picker), `step(±1)` (a day, a week or a month by the view; `addDays`, `addMonths` in `lib/day.ts`); in Reports the three move the report's period instead (`shiftReportRange`); each remembers the day as `uiState.lastDay`, and `restoreDay` shows it again at boot when it is at most 30 days old |
| `day` | `dayView` (the shown day's `DayView`; the previous one stays while the next day loads), `dayError`, `loadDay` (drops answers for a day no longer shown), `focusedBlock`/`focusBlock`, `detailsBlock`/`toggleDetails` (the card opened by keyboard), `blockMenuRequest`/`requestBlockMenu`, `setAppSetting(appKey, patch)` (stores the returned settings; the Day view reloads on the `data_changed` it causes) |
| `reports` | `reportFilter` (`null` follows `settings.reports` through `defaultFilter`), `setReportFilter(patch)`, `shiftReportRange(anchor, by)` (`api.reportRangeShift`: the period `by` steps of its unit away, from the period holding `anchor` when given, so `by = 0` is that period; failures toast), `savedReports`/`loadSavedReports` (boot), `saveReport(name)` (the shown filter), `renameSavedReport`, `deleteSavedReport`, `openSavedReport` (its filter, and the Reports view) |
| `rules` | `rules` (by position), `loadRules` (the Rules view, and every `data_changed` of `rules`), `saveRule(id, input)` (rejects; toasted unless the error has a `field`), `deleteRule`, `reorderRules(ids)`, `setRuleEnabled`, `acceptSuggestion(suggestion)` and `acceptRuleSuggestions(ruleId)` on the shown day, each with a toast offering Undo |
| `data` | `dataVersion` (every day changed), `dayVersions` (per day), `changeCount` (every event, for views over many days), `dataChanged(event)` (called on every `data_changed` event); `selectDayVersion(day)` changes when that day's data may have changed |
| `projects` | `projects` (`ProjectListItem[]`, archived included), `clients`, `tags` (`TagListItem[]`), `recent`, `projectsLoaded`; `loadProjects` (boot, and every `data_changed` of `projects` or `tags`); `saveProject(id, input)`, `archiveProject`, `saveClient`, `archiveClient`, `saveTag`, `deleteTag` reload after the write and reject on failure (toasted, unless the error has a `field` for the form). Helpers: `groupProjects(projects, clients, archived)`, `projectLabel` |
| `tracker` | `tracker` (`TrackerStatus`, or `null` while the tracker is off), `setTracker`, `loadTracker`, `pauseTracker(until)`, `resumeTracker`, `requestPermission` (asks macOS, then reloads the status), `permissionDialogHidden` |
| `recordings` | `selection` (`{ start, end, blockIds, moving? }`, unsnapped; `moving` is the end a drag moves), `selectionSnap` (as Rust snaps it), `setSelection`, `extendSelection(blockId)`, `toggleSelectionBlock(blockId)` (⌘-click), `focusedRecording`/`focusRecording`, `hoveredRecording`/`hoverRecording` (the recording under the pointer in the timesheet or the time column), `undoStatus`/`loadUndoStatus`, `createRecording(input)`, `updateRecording(id, patch)`, `updateSpan(id, spanId, span)`, `nudgeSpan(id, spanId, startSteps, endSteps)` (Rust applies the snap step), `deleteRecording(id)`, `moveRecording(id, day, spanId)`/`copyRecording(id, day, spanId)` (one span, else with `null` the whole recording), `undo`, `redo`. A write runs with `overlap: cancel`; on `overlap` it asks (inside the editor while one is open, else the overlap dialog) and runs again with `replace`. One write at a time; each ends with a toast offering Undo. A field error rejects for the editor, others are toasted |
| `editor` | `editor` (`create { start, end, projectId?, warning? }` or `edit { id, at?, spanId? }`), `draft` (`{ projectId, spans, totalMs, note, tagId }`; null until Rust has answered for its spans and an edited recording has loaded through `recordingsList`), `editing` (that recording), `editorOverlap` (the recordings a save would overlap, with `resolve`), `openEditor`, `closeEditor` (also clears the selection), `patchDraft` (project, note, tag), `setDraftSpan(i, span)` and `removeDraftSpan(i)` (shown at once, then as `recordingDraftSpans` rolls them), `addDraftSpan` (`recordingDraftAdd`), `setDraftTotal(ms)` (`recordingDraftTotal`); only the latest ask's answer is kept. `recordSelection(add?)` (snaps the selection through `api.recordingSnap`, then opens a new editor on it, moves an open new one to it, or with `add` adds it as a span of the open editor's recording), `answerOverlap(replace)` |
| `appIcons` | `appIcons` (a PNG data URL per bundle id, `null` once known to have none), `requestAppIcon(bundleId)`: asks Rust once per id for the app's run; the asks of one task go out as one `api.appIcons` call, and a failure leaves the initials |
| `viewState` | `uiState` (`UiState` from Rust), `setPaneSize`, `toggleSidebar`, `rememberDay`, `setTimelineZoom` (px per minute, `TIMELINE_ZOOM` 1–8, default 2.5), `finishOnboarding`, `showWelcome`; a debounced writer saves it, `flushUiState` writes at once |

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
`ACCENTS` (mirrors Rust's list; a Rust test compares them), `TEXT_SIZES` and `SETTINGS_LIMITS`
(mirror the clamps in `Settings::sanitized`, the source of truth), `LOG_LEVELS`, `BUNDLED_FONTS`,
`Keybindings` and `EMPTY_UI_STATE`.

## Commands

Every user action that has a shortcut or appears in the palette is a `Command` (`commands/registry.ts`):
`{ id, title, scope, defaultBinding?, when?, run, menu?, checked? }`. The core commands are `APP_COMMANDS`
(`commands/index.ts`); their default bindings are in [ux.md](ux.md#keys).

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
  every focus change (Accept Suggestion and the text-field Undo follow the keyboard focus); `syncMenu` sends
  only a changed state, so Rust greys out and checks the items.
- **Palette.** `CommandPalette` (the sheet, ↑ ↓ ↵ ⌘↵) with `components/palette/`: `parse.ts` (`parsePrefix`:
  `>` commands, `p:` projects, `c:` clients, `@` dates; `score`: 5 equal, 4 prefix, 3 a word start, 2 inside a
  word; `fuzzyScore` adds 1 for the characters in order, for project and client names), `usePaletteItems.ts`
  (one provider per kind: views, dates, settings pages, projects, clients, saved reports, commands; each section's
  matches best first, the sections by their best match, ties in that order) and `PaletteList.tsx` (section labels,
  rows, `PaletteFooter`). Dates come from Rust (`useDateJumps.ts`: `api.calendarJumps`, `calendar::jump`), so the page parses no
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
  `activity.deleteDay` and `activity.deleteRange` (no default keys; also in the Day header's ⋯ menu) open
  `DeleteActivityDialog` once the shown day's view has loaded. `suggestion.accept` and
  `suggestion.acceptRule` (Day menu, palette) record the suggestion whose button has the focus, or all of its
  rule's suggestions on the day, as ↵ and ⌥↵ on it do. `nav.goToDate` opens the date picker
  (`DatePickerPopover`).
- **Recordings.** `recording.next`/`prev` (] [) focus the shown day's recordings by start;
  `recording.open` (Enter) edits the focused recording or opens the editor on the selection (`recordSelection`);
  `recording.create` (Record Selection) and `recording.edit` (menu and palette) do one of the two;
  `recording.new` (File → New Recording…, from any view) opens the editor on the Day view's selection, else
  for the span `recording_default_span` proposes (the half hour up to now) on today's Day view (or this week's Week view);
  `recording.delete` (⌫);
  `recording.nudgeEarlier`/`nudgeLater` (⌥↑ ⌥↓) move the focused recording's first span of the day and
  `recording.extend`/`shrink` (⌥⇧↓ ⌥⇧↑) change its end, by one snap step (`nudgeSpan`: the page sends step counts, `recording_nudge` computes the times);
  `recording.moveToPrevDay`/`moveToNextDay` (⌥← ⌥→) and `recording.copyToPrevDay`/`copyToNextDay` (⌥⇧← ⌥⇧→)
  take the whole recording to the adjacent day at the same time. `edit.undo` (⌘Z) and `edit.redo` (⌘⇧Z) are global
  and replace the native Edit items: in a text field they undo its typing (`document.execCommand`), also in
  dialogs and the editor; elsewhere the last recording change, but not while an overlay is open. The
  palette's search field is left out, so the palette never lists them for it. While the editor is open, only
  `MODAL_ALLOWED` commands run. `zoom.in`/`zoom.out`/`zoom.reset` (⌘= ⌘- ⌘0)
  zoom the timeline (`TIMELINE_STEPS`) while the Day view shows and the text everywhere else.
- **Projects.** `projects.new` (⌘N) and `clients.new` (no default) open the sheets while the Projects view
  shows; `view.projects` (⌘5) shows it.
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

Shell components in `components/`: `Toolbar` (title and Settings; a drag strip), `Sidebar` (a drag strip
under the window buttons, `DateNavigator`, then the entries of `views.ts`), `StatusBar`, `EmptyView`, `Modal`
(`variant`: `sheet`, `panel`, `popover`, `bare`), `ContextMenu`, `Toasts`, `Field` (`InsetField`, `KeyCap`),
`Splitter`, `ErrorBoundary`, `CommandPalette` (`palette/`), `ShortcutsDialog`, `Dialogs`, `AppIcon`, and
`useAnchoredStyle` (a popover's fixed place under the element matching a selector, left- or right-aligned to it:
the date picker, the timeline settings, Lanes).

`AppIcon` (`app`, `bundleId`, `size` 16 or 20): the installed app's icon from the `appIcons` slice, rounded,
or the app's initial while there is none (no bundle id, an external source, an app not installed). 16 px in
timeline blocks and app lane heads, 20 px in the hover card, List view rows, rule conditions and Settings →
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

Dates are changed in one place, the sidebar:

- `DateNavigator` (top of the sidebar): the shown period as a heading, ‹ › and a Today button. In Day, Week
  and Month `useCalendarPeriod` asks Rust for the period (`api.calendarPeriod(unit, day)`: `from`, `to`,
  `holdsToday`) and the page only formats it: Day `Wed, Oct 21` (`formatDayLabel`), Week `Oct 19 – 25`
  (`formatRangeLabel`), Month `October 2026` (`formatMonthLabel`). Reports shows the preset's name or the
  custom range. The Today button reads Today, This week or This month by the unit and is disabled while the
  period holds today. The buttons call `step` and `goToday`, as P, N and O do. Projects and Rules render it
  hidden (`date-nav--idle`), so the view list keeps its place.
- `DatePickerPopover` (dialog `datePicker`, opened by the heading; anchored under `[data-date-heading]`): the
  month grid from Rust (`api.calendarPicker(month, selected)`: whole weeks from Monday, each day with
  `outside`, `isToday` and `selected`), today in accent text, the shown period (or a custom report range) on
  `--sel`. ‹ › and PageUp / PageDown turn the month, arrow keys move the focused day, ↵ or a click calls
  `showDate` and closes it.
- `StatusBar`: the sidebar toggle, `TrackerStatus` and the selection on the left; the running timer,
  `BillableRatio` (`api.billableRatio(range)` for the shown period: the report's range in Reports, the
  `useCalendarPeriod()` days in Day, Week and Month, today elsewhere; keyed on `changeCount` and the
  `useLiveReload` tick; a `role="meter"` hidden while `percent` is `null`) and
  `TodayTotals` (`api.dayTotals(today)`, reloaded on `selectDayVersion(today)` and the `useLiveReload` tick) on
  the right, so today's `recorded 1:00 · tracked 3:12` shows in every view.

Tracker components:

- `TrackerStatus` (status bar, left; the tracker state shows only there): a dot (accent while running, `--o3` while away or paused, `--style-red`
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
| `DayPage` | `DayHeader`, then the `Timesheet` pane (width from `usePaneSize("timesheet")`, about a quarter of the window by default, 220–560 px, a `Splitter` beside it) and `Timeline` or `ListView`; the empty state ("Nothing tracked on this day" with the tracker's status text on today, or "Tracking needs permission" with Allow…) takes the timeline's place. Loads on a new day, on `selectDayVersion(day)`, and while today on `useLiveReload` (every 30 s, and on tracker statuses at most every 10 s, since the tracker sends no `data_changed`; paused while `document.visibilityState` is hidden, which WKWebView sets when the window hides, with one reload when it shows again) |
| `DayHeader` | a slim bar: the date (`formatDayLabel`), totals `recorded 0:00 · tracked 6:12 · away 1:05` (`formatHm`) only for a day other than today (the status bar has today's), the Day mode switch, Lanes (timeline mode; dialog `lanes`, anchored under `[data-lanes-button]`), the gear that opens `TimelineSettingsPopover` (command `day.settings`, no default key), and the ⋯ menu: Delete activity for this day…, Delete activity in a range… |
| `DeleteActivityDialog` | dialog `deleteActivity`, a `Modal` sheet: the day (its `dayView.bounds`) or two `datetime-local` fields prefilled with them, the sentence "Deletes 412 activity spans and 3 away spans. Spans that cross the range are deleted entirely. Recordings are not affected." (counts from `api.activityCount`), errors inline, and Delete (`api.activityDelete`). Then a toast `Deleted activity for Tue, Oct 21` whose Undo (`api.activityDeleteUndo`) lasts until `undoUntil`; the Day view reloads from `data_changed` |
| `Timesheet` | the left pane: "Timesheet", the day's `totals.recordedMs` (`formatShortDuration`, `6h 05m`), + (`runCommand("recording.new")`), then a `TimesheetCard` per `view.timesheet` entry in Rust's order, or an empty line. The open `RecordingEditor` sits in place of the card it edits, or for a new recording before the first card that starts after its first span (`editorSlot`), in the same keyed list as the cards so it moves without mounting again |
| `TimesheetCard` | a `Card` (role button) in the project's shade: the note as title (the project without one), the `AccentDot` and `Project \| Client`, the tag and `n spans`, `durationMs`; its height grows with the duration (`--card-minutes`). Click or ↵ focuses the recording and opens the editor on it, hover `hoverRecording` |
| `ZoomControl` | −, a range input (`TIMELINE_ZOOM`, step 0.5) and +: `stepTimelineZoom` and `setTimelineZoom` |
| `LanesPopover` | dialog `lanes`: Summary (`summaryLane`) and Activity (`showAllActivity`) switches (the settings slice's `saveTimeline`), then a switch per `dayView.apps` entry for `ownLane` (`setAppSetting`) |
| `TimelineSettingsPopover` | dialog `timelineSettings`, a `Modal` popover under the header's gear: `TimelineControls` and "Hidden apps: n" from `dayView.hiddenApps`, each with Unhide |
| `Timeline` | `timeline-frame`: the scroll box with `LaneHeader` (sticky), `Spans` (away and gap bands across the whole body), `TimeAxis`, the time column (`RecordingsColumn`), the lanes and the current-time line on today; `ZoomControl` floats over its bottom right. Opens a day scrolled to 30 min before its first activity. Hover (one `mouseover` listener reading `data-block-id`) shows `HoverCard` after 300 ms; right-click, the gear on a hovered block, or ⌃↵ opens the block menu. With `compact` it draws the day at 60% of the zoom and adds `timeline--compact` |
| `blockMenu.ts` | the block menu's entries: for tracker blocks Group by app, Titles: Document name / App name and Own lane, checked by the effective value (an override equal to the timeline's setting is removed), Hide (a toast with Undo); then Copy title (not for private blocks), Copy URL |
| `Lane` | its blocks, absolutely placed: `top`/`height` from the times and the zoom, `left`/`width` from `col`/`cols`; only blocks within the visible part ± one screen (`useVisibleRange`, which moves in half-screen steps). The summary lane draws `summary` blocks, the others `memory` blocks |
| `ActivityBlock` | `look="summary"`: a `Block` with the `AppIcon` (16 px) and the label as title and `activeMs` (`formatShortDuration`) at the bottom right when tall; `look="memory"`: the icon over a bar of the block's real height, the label and `activeMs` beside it (two lines, one line when short, only the bar when tiny); a gear on hover; `private`, `dimmed` (covered) and `focused` classes |
| `HoverCard` | `AppIcon` and app, title, URL (http(s) opens through `api.openUrl`) or path, time and duration, the source when external; beside the block, else below or above it |
| `RecordingsColumn` | the time column: the day's `SuggestionBlock`s under its `RecordingBlock`s (`look="times"`, `hovered` from `hoveredRecording`), while a span is selected a dashed outline where its recording would go, and the open editor's draft spans (cut to the day) as filled accent blocks with their times (`drawnParts`, for drawing only), in place of the edited recording's own blocks; its blocks get `editOnClick`. A press on empty time is `useSelectionPress`'s |
| `SuggestionBlock` | a rule's suggestion: a button outlined dashed and half filled in the project's card colors, with the project, the rule and the duration; click sends `acceptSuggestion`, ⌥-click `acceptRuleSuggestions` (the `rules` slice; Rust's `data_changed` reloads the day) |
| `useRuleDrag` | ⌥-pressing an activity block (in `Timeline`, while no editor is open) shows the recent projects over the top of the recordings column; releasing over one opens the `rule` dialog with `draftFromBlock`: `<app> → <project>`, `app is <app>` and `domain is <host>` for a URL block, else `title contains <title>` |
| `RecordingBlock` | one span of a recording (`spanId`; a recording with several spans has several blocks, all focused and hovered together): a `Card` in the project's shade; `look="card"` (Week view) shows project, note, tag and duration, `look="times"` (Day view) only the start and end. Dashed top or bottom edge when clipped to the day. `look` only styles it; with `editOnClick` (the Day view) a click focuses the recording and opens the editor, without it a click focuses and a double-click edits. Hovering sets `hoverRecording`; dragging the body moves the span and an edge resizes it (`pointerDrag`, pointer capture), with the ghost from `useSnappedSpan` (the dragged edge as `moving`); release sends the unsnapped span and `moving` to `updateSpan`, and Rust snaps it |
| `RecordingEditor` | the editor as a form card in the timesheet (in the Week view inside a `Modal` sheet), on the `editor` slice's `draft`: an optional warning (the List view's gaps), the note as the title field (autofocus; one line, pasted line breaks become spaces), project (`ProjectPicker`), tag (`TagPicker`), the duration (`DurationField`, `0 h 20 m` from `draft.totalMs`; a change goes to `setDraftTotal`), the covered activity (`CoveredActivity`) and the spans (`EditorSpans`); Save (↵ or ⌘↵) and Delete (⌘⌫) come from `useEditorSave` (every span as Rust rolled it; a running recording keeps its spans and sends only the fields; field errors under their field), Cancel (Escape, wherever the focus is). Its form is named "New recording" or "Edit recording" (`id` lets the Week view's sheet point at it). A new recording takes the project of `useProposal` while it has none. Scrolls into view when it opens |
| `EditorSpans` | the draft's spans as From / To rows (`TimeField`: `HH:MM`, ↑↓ by the snap step), each with ✕ while there are several, and "Add span" (`addDraftSpan`: Rust places it after the last); a running recording's spans are shown, not edited |
| `EditorParts` | `DurationField`, `CoveredActivity` ("Activity n", opening the list of blocks with their start and time inside) and `OverlapNotice` (the recordings in the way, "Replace overlapped part" or Back, inside the editor) |
| `useProposal` | `api.recordingProposal` for the draft's spans once they settle (150 ms) |
| `useEditorSave` | the editor's `save`, `remove` and field `errors` |
| `TimeField` | `HH:MM` typed (each valid time counts at once) or ↑↓ by the snap step; the typed text shows while it belongs to the value, else the value |
| `OverlapDialog` | the recordings in the way, "Replace overlapped part" or Cancel, for writes made while no editor is open |
| `useSelectionPress` | the press on the lanes, the axis and the time column: a drag selects a span (auto-scrolling near the edges; `moving` is the end under the pointer) and opens the editor on release; a click on a block selects its span and opens the editor, ⇧-click extends the selection first, ⌘-click adds or removes the block and opens the editor when ⌘ is released (one listener per gathering, kept in a ref), a click on nothing clears the selection. With ⌥ while the editor is open, the span is added to its recording (`recordSelection(true)`) |
| `useSnappedSpan` | a span on Rust's grid (`api.recordingSnap` with the span's own `moving` end, at most once per frame; the raw span stands in until the answer) |

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

### List view

`components/day/list/`, shown by `DayPage` when `uiState.dayMode` is `list` (the "Day mode" segmented
buttons in `DayHeader`, or ⌘⇧L `day.toggleList`; `viewState.setDayMode` persists it), beside the same
`Timesheet` pane as the timeline.

- `ListView` loads `api.listView(day)` through `useApiResult`, keyed on `selectDayVersion(day)` and the live
  tick, and keeps Rust's order. The search field filters the loaded rows by app, label, title, URL and path
  (`/` focuses it, Escape clears it). ↑↓ move between rows; ↵ opens the recording editor for the row's span
  without a project; Space selects the row's span and blocks and switches to the timeline.
- `ListRow`: `AppIcon` and app, label, URL or path, the duration, `· n spans`, and a bar for the covered share. Clicking
  the title shows its details (title, URL, path, spans).
- `ProjectStrip`: active projects, recent first, 8 chips and the rest under More…. Dragging a row onto a chip
  (`pointerDrag`, `elementFromPoint` on `[data-project-id]`) or clicking a chip with a row focused opens the
  editor with that project; a row with several spans adds the gaps warning (`Editor.create { projectId?,
  warning? }`).
- Timeline-only commands (block focus, selection, recording keys) are off in List mode, so the rows keep
  their keys; zoom zooms the text.

### Away window

`windows/away/AwayPrompt.tsx`, the `away` window's page. It loads settings, projects and
`awayPromptCurrent` on mount and on focus, and refills from `away_prompt_changed` (the form is keyed by the
span, so a refresh of the same span keeps what was typed). Record sends `awayPromptAnswer`; an `overlap`
shows which recordings are in the way and keeps the form; Escape or Skip sends `awayPromptSkip`. It has no
toasts: unexpected errors are logged and shown inline. Settings → Tracker has the away prompt's switch and
its minimum length.

### Rules screen

`components/rules/`, the `rules` view (sidebar entry Rules, ⌘6). The `rules` slice holds `rules` (by
position) and their writes; a `data_changed` of kind `rules` reloads them.

- `RulesView` lists `RuleRow`s in order: a grip (a pointer drag by whole rows; on release `reorderRules`
  sends every id in the new order, shown at once and restored when the write fails), the enable switch
  (`setRuleEnabled`), the name, the project and `conditionSummary` (`app is "Slack" · title contains "x"`).
  A row click, New rule or ⌘N (`rules.new`) opens the `rule` dialog.
- `RuleSheet` (dialog `rule` with `id` or a `draft`): name, `ProjectPicker`, note, `TagPicker`, the
  `ConditionRow`s (field, comparison, value, remove; Add condition; an `app` condition whose value names a
  tracked app from `appsList()` shows its `AppIcon`, else the initial) and the enable switch. Save sends
  `saveRule`; a field error shows under its field, and `conditions.<i>.value` marks that row. Beside the
  form, `RulePreviewPanel` asks `rulePreview(input, 7)` through `useApiResult` (300 ms debounce, keeps the
  last result) once every condition has a value, and lists the hits with the total.

### Week and Month

`components/calendar/`: `CalendarHeader` (the title, totals). `WeekView` runs `weekView(day)` keyed
by the day and `changeCount`: day heads (label and total; a click opens the day) over a `TimeAxis` and seven
columns that place each day's `RecordingBlock`s from the column's bounds. A block keeps its vertical move
and resize; released over another column (`onDrop`, found with `elementFromPoint`) its span moves there, with ⌥
it is copied as a new recording (`moveRecording`/`copyRecording` with the block's `spanId`, in the `recordings`
slice, which ask about overlaps like every write). A double-click on an empty spot opens the editor for an hour from there,
as a `Modal` sheet around `RecordingEditor` (the Week view has no timesheet). `MonthView` runs
`monthView(day)`: a seven-column grid of `DayCell`s (day number, recorded `h:mm`, a 3 px strip of the top
three projects' shades by time); a click opens the day.

The editor's `edit` request may carry `at` (a span of the recording), so views other than the Day view open it.

### Reports screen

`components/reports/`: `ReportsView` (the `reports` view) runs `reportRun(filter)` keyed by the filter and
`changeCount`: a header with the range (`formatRangeLabel`), the grouping of the copied text, Copy as text
(`reports.copyText`, ⌘⇧C: `reportText` to the clipboard) and Save report… (`reports.save`: the `prompt`
dialog, then `saveReport`); `ReportFilters`, one slim row (Period presets as a segmented control, Custom revealing dates, `MultiSelect`
for clients, projects and tags, Rounding, Archived projects). Below, all rendered from `ReportResult` as Rust
built it: `ReportTotals` (the total large, recordings, projects, untagged time, `Billable 6:24 h · 80 %`),
two `ReportChart`s (SVG donuts of `result.charts`, by client and by tag, each with a By select for client,
project or tag, the total and count in the center, percentages on the ring and a legend; colors through
`accentColor` in `Accent.tsx`), and `ReportTable` (`result.clients`: client rows, project rows that unfold
their recordings, Expand all / Collapse all; a recording row shows its day, all its spans of the day
`14:25–15:00, 16:05–20:15`, note and tag). A project row's Copy notes puts `project.notes.text` in the
clipboard and toasts `Copied 4 notes for Acme / Portal` (`copyNotes` in `actions.ts`, which also
holds the commands' actions). The sidebar
lists saved reports under Reports: a click opens one, right-click renames (prompt) or deletes (confirm).

`PromptDialog` (`dialog: prompt { title, label, initial, confirmLabel, onSubmit }`) asks for one line.

### Projects screen

`components/projects/`: `ProjectsPage` (the `projects` view): a header (New project, New client, Show
archived), a band per client by name with its color dot (every client, "No projects" when empty; clicking
the name edits the client; a `+` creates a project for that client, except on archived clients),
client-less projects under "No client" last, then `TagsSection`. A project row: color dot and name (click
edits), a `billable` badge for a billable project, `last used 2 d ago` and `12:30 h / 30 d` in mono `--o2`; archived rows (with Show archived) are
dimmed with Restore. The row's context menu: Edit…, Archive or Restore, Copy id.

- `ProjectSheet` (`dialog: project`, an optional `clientId` preselects the client): name, client select
  with "New client…" (an inline name; Save creates the client first), the color (`AccentSwatches`, only
  without a client or with a new one) and the shade (`ShadeSwatches`; none pressed lets Rust pick), the Billable
  switch (`input.billable`; new projects start off), Archive or Restore for an existing project. `FormField` (`components/Field.tsx`) shows `AppError.field` errors
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
| `TagPicker` | `value: string \| null`, `onChange(tagId \| null)` | one tag or none ([ADR 0019](adr/0019-one-tag-per-recording.md)): the chosen tag in mono in the field, "No tag" as placeholder; the open list starts with "No tag", then every tag; typing filters; Enter picks the highlighted one or creates `New tag "<query>"` (`saveTag`) and picks it; ⌫ on an empty query removes the tag. The editor, `RuleSheet`, the timer picker and the away window use it |

### Settings pages

`components/settings/SettingsDialog.tsx` renders the pages registered in `registry/settingsPages.ts`;
`pages.tsx` registers the core ones (`registerCoreSettingsPages`), `controls.tsx` holds the shared rows:

| Page | Holds |
|---|---|
| Appearance | Mode (`SegmentedControl`: Light, Dark, System), Scheme (three tiles, each a miniature window in its light-mode look through `resolveLook`), accent row, UI and data sizes, fonts |
| Shortcuts | `ShortcutsPage`: every command with its binding; record, reset or unbind |
| Tracking → Capture | `CapturePage` (`CapturePage.tsx`): address and document switches, excluded apps (an "Add app…" picker listing `appsSeen(30)`), excluded websites (a field; Enter adds) |
| Tracking → Tracker | `TrackerPage`: away after (idle minutes), check every (poll seconds) |
| Tracking → Timeline | `TimelinePage` (`TimelinePage.tsx`): `TimelineControls` (summary lane and its interval slider, show all activity, compact, group by app, continuity, noise, titles, time snapping; each saved quietly), then Apps from `appsList()`: per app its `AppIcon` (by `appKey`), Group by app and Titles (Default or a value), Hidden, Own lane |
| System → General (order 0) | `LaunchAtLoginSwitch` (`LaunchAtLoginSwitch.tsx`, also on the welcome sheet): checked from `autostartStatus()` (the LaunchAgent, not the stored setting), changed through `setLaunchAtLogin` (`settings` slice: `api.autostartSet`, then the store's `settings.launchAtLogin` is updated so a later save keeps it); "Show welcome again" closes Settings and calls `showWelcome` |
| System → Data | the data folder (Reveal in Finder; Move…: `chooseFolder` → a `Modal` sheet that says the database is copied, tracking pauses meanwhile and the old files stay → `dataMove` with the Move button busy (`Moving…`, Escape ignored) → toast "Data moved to …", settings and the page's `DataInfo` reload; an `invalid_input` refusal shows under the row, other errors toast), counts, first activity, size on disk, Export backup… (save panel → `backupExport`, toast "Backup written"); an alert when the store could not be opened |
| System → Logging | log level, Open log folder |

A change saves through `saveSettings` (`api.updateSettings`) and applies at once.

## Styling

The CSS is plain and global, imported by `styles/index.ts` in this order: `styles/fonts.css`, the four palettes
`styles/palettes/{content,sidebar}-{light,dark}.css`, `styles/theme.css` (derived tokens, the sidebar's token
mapping, the block colors), then one stylesheet per layer, base to windows: `base.css` (reset, scrollbars,
text utilities), `controls.css` (buttons, fields, key caps, segmented control, switch, swatches), `shell.css`
(grid, toolbar, splitter, status bar), `sidebar.css`, `overlays.css` (dialogs, menus, toasts, palette, hover
card, shortcut overview), `forms.css`, `pickers.css` (pickers, date picker, app picker), `settings.css`,
`projects.css`, `day.css` (timeline, axis, lanes, away bands, selection, editor), `blocks.css` (everything
filled: activity blocks, cards, recordings, suggestions, project chips), `reports.css`, `calendar.css`
(Week, Month), `list.css`, `rules.css` and `windows.css` (timer picker, away prompt). The import order is the
cascade. Icons come from `lucide-react`. Token names and roles are in [ux.md](ux.md#appearance-and-tokens).

- **Appearance.** `applyAppearance` resolves `mode` and `scheme` with `resolveLook` (`lib/theme.ts`) and sets
  `data-content` and `data-sidebar` on `<html>`, `data-accent` from `accent`, the font sizes and families;
  Rust (`window.rs` `apply_window_mode`) sets the native window appearance from `mode`. The palettes match
  `[data-content="light|dark"]` and `[data-sidebar="light|dark"]`, so any element can preview a look.
- **Sidebar tokens.** The sidebar uses `--sidebar-*` only; inside `.sidebar`, `theme.css` maps `--text`, `--o2`,
  `--o3`, `--line`, `--row-hover` and `--sel` to the sidebar set, so shared controls (key caps, buttons)
  follow it. The accent hues come from the content palette of the sidebar's tone
  (`[data-sidebar="<tone>"] .sidebar` shares its rule), and `--accent` is re-derived there.
- **Block colors.** `.card`, `.suggestion` and `.project-chip` derive `--block-fill`, `--block-text` and
  `--block-meta` from `--rec` (inline, a `shadeColor`) and the palette's `--block-*` numbers.
  `styles/contrast.ts` mirrors that math (OKLCH to sRGB, WCAG contrast) for the scheme audit.
- **No color literals** (`#hex`, `rgb(`, `hsl(`) outside `styles/palettes/`; `styles/colors.test.ts` scans.
- **Sizing.** Chrome is in rem: `html { font-size: var(--ui-font-size) }`, so zoom scales it. Spacing uses
  `--space-*`, type the `--fs-*` scale.
- **macOS look.** The sidebar spans the window's height under the traffic lights; without it the toolbar
  leaves `--traffic-lights` for them. `data-tauri-drag-region` marks draggable areas (the toolbar, the
  sidebar's top strip); `user-select: none` applies globally, inputs and `.selectable` opt back in.

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
