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
   1. `loadSettings()` loads settings and keybindings; `installCommands()` runs again to apply the user's
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
   ├─ main           ActivityDebug for Day (until the Day view), else EmptyView: the active view's name
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
| `ui` | `toasts`, `dialog` (`settings { page? }`, `shortcuts`, `confirm`, `project { id? }`, `client { id? }`), `paletteOpen`, `paletteSeed`, `view` (`day`, `week`, `month`, `reports`, `projects`), `setView`, `day` (the Day view's `YYYY-MM-DD`, today at start), `goToDay(day)` (also switches to the Day view; `timetracker show` uses it) |
| `data` | `dataVersion` (every day changed), `dayVersions` (per day), `dataChanged(event)` (called on every `data_changed` event); `selectDayVersion(day)` changes when that day's data may have changed |
| `projects` | `projects` (`ProjectListItem[]`, archived included), `clients`, `tags` (`TagListItem[]`), `recent`, `projectsLoaded`; `loadProjects` (boot, and every `data_changed` of `projects` or `tags`); `saveProject(id, input)`, `archiveProject`, `saveClient`, `archiveClient`, `saveTag`, `deleteTag` reload after the write and reject on failure (toasted, unless the error has a `field` for the form). Helpers: `groupProjects(projects, clients, archived)`, `projectLabel` |
| `tracker` | `tracker` (`TrackerStatus`, or `null` while the tracker is off), `setTracker`, `loadTracker`, `pauseTracker(until)`, `resumeTracker`, `requestPermission` (asks macOS, then reloads the status), `permissionDialogHidden` |
| `viewState` | `uiState` (`UiState` from Rust), `setPaneSize`, `toggleSidebar`; a debounced writer saves it, `flushUiState` writes at once |

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
- `ActivityDebug`: temporary, until step 06's Day view. A date field with previous/next day buttons and
  two plain tables from `activityList(day)` (with the URL or path; private rows dimmed) and `awayList(day)`, reloaded on `data_changed` and on tracker
  statuses at most every 10 s (a status inside the 10 s reloads when they end). Its previous/next day buttons
  step the date in the page; the Day view moves that to Rust.

### Projects screen

`components/projects/`: `ProjectsPage` (the `projects` view): a header (New project, New client, Show
archived), a band per client by name (every client, "No projects" when empty; clicking the name edits the
client; a `+` creates a project for that client, except on archived clients), client-less projects under "No client" last, then `TagsSection`. A project row: color dot and name
(click edits), `last used 2 d ago` and `12:30 h / 30 d` in mono `--o2`; archived rows (with Show archived) are dimmed with Restore. The row's context menu: Edit…, Archive or Restore,
Copy id.

- `ProjectSheet` (`dialog: project`, with an optional `clientId` that preselects the client): name, client select with "New client…" (an inline name; Save creates
  the client first), `AccentSwatches`, Archive
  or Restore for an existing project. `FormField` (`components/Field.tsx`) shows `AppError.field` errors under their field.
- `ClientSheet` (`dialog: client`): name; Archive (also archives its projects) or Restore.
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
