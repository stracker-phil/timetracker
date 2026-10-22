import { api, TEXT_SIZES, type Settings } from "../api";
import { exportBackup, revealDataFolder, showWelcomeAgain } from "../components/settings/actions";
import { copyReport, copyReportNotes, promptDeleteSavedReport, promptRenameSavedReport, promptSaveReport } from "../components/reports/actions";
import { VIEWS } from "../components/views";
import { stepBlock, stepLane } from "../lib/blockFocus";
import { stepThroughRust } from "../lib/dayStepper";
import { sheetEntries } from "../lib/sheet";
import { fail } from "../state/requests";
import { TIMELINE_ZOOM, useStore, type View } from "../state/store";
import { allCommands, bindingFor, bindingLabel, getCommand, isRecordingTarget, isTextInput, registerCommands, resolveKeydown, setOverrides, type Command } from "./registry";
import { activeScopes, registerScope } from "./scopes";
import { registerSuggestionScope, SUGGESTION_COMMANDS } from "./suggestions";

/** The views with a period, which P, N and O step. */
const DATED_VIEWS: View[] = ["day", "week", "month", "reports"];

export * from "./registry";
export * from "./scopes";

/** Registers the core scopes; safe to call again (tests and re-installs do). A view's scope is active while it shows. */
export function registerCoreScopes() {
  registerScope({ id: "global", parent: null, label: "Everywhere", isActive: () => true });
  registerScope({ id: "dated", parent: "global", label: "Day, Week, Month and Reports", isActive: () => DATED_VIEWS.includes(useStore.getState().view) });
  for (const { id, label } of VIEWS) {
    const parent = DATED_VIEWS.includes(id) ? "dated" : "global";
    registerScope({ id, parent, label, isActive: () => useStore.getState().view === id });
  }
  registerSuggestionScope();
}

function saveTextSizes(sizes: (current: Settings) => Pick<Settings, "uiFontSize" | "dataFontSize">) {
  const s = useStore.getState();
  if (s.settings) void s.saveSettings({ ...s.settings, ...sizes(s.settings) }, { quiet: true });
}

/** Rust's `Settings::sanitized` clamps the sizes, so zooming past a bound is a no-op. */
const zoomText = (step: number) => saveTextSizes((c) => ({ uiFontSize: c.uiFontSize + step, dataFontSize: c.dataFontSize + step }));
const resetText = () => saveTextSizes(() => ({ uiFontSize: TEXT_SIZES.ui.default, dataFontSize: TEXT_SIZES.data.default }));

const onDayView = () => useStore.getState().view === "day";
// The Day view zooms its timeline; every other view zooms the text.
const zoomIn = () => (onDayView() ? useStore.getState().stepTimelineZoom(1) : zoomText(1));
const zoomOut = () => (onDayView() ? useStore.getState().stepTimelineZoom(-1) : zoomText(-1));
const resetZoom = () => (onDayView() ? useStore.getState().setTimelineZoom(TIMELINE_ZOOM.default) : resetText());

const timerRuns = () => !!useStore.getState().timer?.running;

/** The shown day's view, once loaded: its bounds come from Rust. */
const shownDayView = () => {
  const s = useStore.getState();
  return s.dayView?.day === s.day ? s.dayView : null;
};

function openDeleteActivity(range: boolean) {
  const view = shownDayView();
  if (view) useStore.getState().openDialog({ type: "deleteActivity", day: view.day, bounds: view.bounds, range });
}

/** Takes the focus off a Tabbed-to button, which would keep ↵ and Space (`activatesControl`). */
function releaseControlFocus() {
  const el = document.activeElement;
  if (el instanceof HTMLElement && el !== document.body && !isTextInput(el)) el.blur();
}

/** Moves the block focus within the shown day's view. */
function moveFocus(step: typeof stepBlock, by: 1 | -1) {
  const s = useStore.getState();
  if (s.dayView?.day !== s.day) return;
  releaseControlFocus();
  s.focusBlock(step(s.dayView, s.focusedBlock, by));
}

/** Selects the focused block's span. */
function selectFocusedBlock() {
  const s = useStore.getState();
  if (s.focusedBlock === null) return;
  releaseControlFocus();
  s.focusRecording(null);
  s.setSelection(null);
  s.extendSelection(s.focusedBlock);
}

/** Moves the block focus in time and grows the selection over it. */
function extendToBlock(by: 1 | -1) {
  const s = useStore.getState();
  if (s.dayView?.day !== s.day) return;
  const next = stepBlock(s.dayView, s.focusedBlock, by);
  if (next === null) return;
  releaseControlFocus();
  s.focusRecording(null);
  if (!s.selection && s.focusedBlock !== null) s.extendSelection(s.focusedBlock);
  s.extendSelection(next);
  s.focusBlock(next);
}

/** Focuses the next (1) or previous (-1) timesheet entry of the shown day, a recording or a suggestion; from none, the first or last. */
function stepEntry(by: 1 | -1) {
  const s = useStore.getState();
  if (s.dayView?.day !== s.day) return;
  const entries = sheetEntries(s.dayView);
  if (entries.length === 0) return;
  const current = s.focusedRecording ?? s.focusedSuggestion;
  const at = current === null ? -1 : entries.findIndex((e) => e.id === current);
  const next = at === -1 ? (by === 1 ? entries[0] : entries[entries.length - 1]) : entries[Math.max(0, Math.min(entries.length - 1, at + by))];
  releaseControlFocus();
  s.focusBlock(null);
  s.setSelection(null);
  if (next.kind === "card") s.focusRecording(next.id);
  else s.focusSuggestion(next.id);
}

const hasFocusedBlock = () => useStore.getState().focusedBlock !== null;
const hasSelection = () => useStore.getState().selection !== null;
const hasFocusedRecording = () => useStore.getState().focusedRecording !== null;
const hasFocusedSuggestion = () => useStore.getState().focusedSuggestion !== null;

/** Opens the editor on the Day view's selection, else on Rust's default span (the half hour up to now) on today's Day, Week or Month view. */
function newRecording() {
  const s = useStore.getState();
  if (s.view === "day" && hasSelection()) return s.recordSelection();
  if (s.view !== "week" && s.view !== "month") s.setView("day");
  s.goToday();
  return s.openNewRecording();
}

/** Edits the focused recording, else opens the editor for the selection. */
function openRecording() {
  const s = useStore.getState();
  if (s.focusedRecording) return s.openEditor({ mode: "edit", id: s.focusedRecording });
  void s.recordSelection();
}

function clearDayFocus() {
  const s = useStore.getState();
  s.focusBlock(null);
  s.setSelection(null);
  s.focusRecording(null);
  s.focusSuggestion(null);
}

/** Moves the start and end of the focused recording's first span of the day by whole snap steps; Rust knows the step. */
function shiftRecording(startSteps: number, endSteps: number) {
  const s = useStore.getState();
  const block = s.dayView?.recordings.find((r) => r.id === s.focusedRecording);
  if (!block || block.clippedStart || block.clippedEnd || block.running) return;
  void s.nudgeSpan(block.id, block.spanId, startSteps, endSteps).catch(() => {});
}

/** Moves (or copies) the focused recording to the day before (-1) or after (1), at the same time of day. */
function recordingToDay(by: 1 | -1, copy: boolean) {
  const s = useStore.getState();
  const id = s.focusedRecording;
  if (id === null) return;
  stepThroughRust("day", () => s.day, by, (day) => (copy ? s.copyRecording(id, null, day) : s.moveRecording(id, null, day)));
}

/** The rule whose row holds the keyboard focus on the Rules screen. */
const focusedRule = () => (document.activeElement instanceof HTMLElement ? (document.activeElement.closest<HTMLElement>("[data-rule-id]")?.dataset.ruleId ?? null) : null);

/** Moves the focused rule up (-1) or down (1) in priority; the focus stays on its row. */
function moveFocusedRule(by: 1 | -1) {
  const id = focusedRule();
  const el = document.activeElement;
  if (id === null) return;
  void useStore
    .getState()
    .moveRule(id, by)
    .then(() => el instanceof HTMLElement && el.isConnected && document.activeElement !== el && el.focus());
}

function overlayOpen(): boolean {
  const s = useStore.getState();
  return s.dialog !== null || s.paletteOpen || s.editor !== null;
}

/** ⌘Z in a text field undoes its typing (the menu's Undo replaces the native one); elsewhere the last recording change. */
function undoOrRedo(redo: boolean) {
  if (isTextInput(document.activeElement)) {
    document.execCommand(redo ? "redo" : "undo");
    return;
  }
  const s = useStore.getState();
  void (redo ? s.redo() : s.undo());
}

/** The palette would list Undo for its own search field; there it only searches. */
const undoRedoAvailable = () => (isTextInput(document.activeElement) && !useStore.getState().paletteOpen) || !overlayOpen();

function togglePalette() {
  const s = useStore.getState();
  s.setPaletteOpen(!s.paletteOpen);
}

/** `?` opens the overview of the scopes active now; pressed again it closes it. */
function toggleShortcuts() {
  const s = useStore.getState();
  if (s.dialog?.type === "shortcuts") return s.closeDialog();
  if (!s.dialog) s.openDialog({ type: "shortcuts", scopes: activeScopes() });
}

function toggleLaunchAtLogin() {
  const s = useStore.getState();
  if (s.settings) void s.setLaunchAtLogin(!s.settings.launchAtLogin);
}

function promptNewTag() {
  const { openDialog, saveTag } = useStore.getState();
  openDialog({ type: "prompt", title: "New tag", label: "Name", initial: "", confirmLabel: "Add", onSubmit: (name) => void saveTag(null, name).catch(() => {}) });
}

function toggleSettings() {
  const s = useStore.getState();
  if (s.dialog?.type === "settings") s.closeDialog();
  else s.openDialog({ type: "settings" });
}

/** A tracker that exists and is not paused. */
function isTracking() {
  const kind = useStore.getState().tracker?.state.kind;
  return kind !== undefined && kind !== "paused";
}

const APP_COMMANDS: Command[] = [
  { id: "settings.toggle", title: "Settings…", scope: "global", defaultBinding: "Mod+,", menu: true, run: toggleSettings },
  { id: "app.launchAtLogin", title: "Launch at login", scope: "global", menu: true, when: () => useStore.getState().settings !== null, checked: () => !!useStore.getState().settings?.launchAtLogin, run: toggleLaunchAtLogin },
  { id: "welcome.show", title: "Show welcome again", scope: "global", menu: true, run: showWelcomeAgain },
  { id: "shortcuts.show", title: "Keyboard shortcuts", scope: "global", defaultBinding: "Shift+?", menu: true, run: toggleShortcuts },
  { id: "palette.toggle", title: "Command palette", scope: "global", defaultBinding: "Mod+K", menu: true, run: togglePalette },
  { id: "sidebar.toggle", title: "Toggle sidebar", scope: "global", defaultBinding: "Mod+Ctrl+S", menu: true, run: () => useStore.getState().toggleSidebar() },
  ...VIEWS.map(({ id, label }, i): Command => ({ id: `view.${id}`, title: `Show ${label}`, scope: "global", defaultBinding: `Mod+${i + 1}`, menu: true, checked: () => useStore.getState().view === id, run: () => useStore.getState().setView(id) })),
  { id: "view.hours", title: "Show Hours (today's Day view)", scope: "global", menu: true, run: () => useStore.getState().showHours() },
  { id: "zoom.in", title: "Zoom in", scope: "global", defaultBinding: "Mod+=", menu: true, run: zoomIn },
  { id: "zoom.out", title: "Zoom out", scope: "global", defaultBinding: "Mod+-", menu: true, run: zoomOut },
  { id: "zoom.reset", title: "Reset zoom", scope: "global", defaultBinding: "Mod+0", menu: true, run: resetZoom },
  { id: "window.hide", title: "Close window", scope: "global", defaultBinding: "Mod+W", menu: true, run: () => void api.hideWindow() },
  { id: "tracker.pause", title: "Pause tracking", scope: "global", when: () => isTracking(), run: () => void useStore.getState().pauseTracker("resumed") },
  { id: "tracker.resume", title: "Resume tracking", scope: "global", when: () => useStore.getState().tracker?.state.kind === "paused", run: () => void useStore.getState().resumeTracker() },
  { id: "nav.prev", title: "Previous day, week, month or period", scope: "dated", defaultBinding: "P", menu: true, run: () => useStore.getState().step(-1) },
  { id: "nav.next", title: "Next day, week, month or period", scope: "dated", defaultBinding: "N", menu: true, run: () => useStore.getState().step(1) },
  { id: "nav.today", title: "Go to today", scope: "dated", defaultBinding: "O", menu: true, run: () => useStore.getState().goToday() },
  { id: "nav.goToDate", title: "Go to date…", scope: "dated", menu: true, run: () => useStore.getState().openDialog({ type: "datePicker" }) },
  { id: "day.nextBlock", title: "Next block", scope: "day", defaultBinding: "J", menu: true, run: () => moveFocus(stepBlock, 1) },
  { id: "day.prevBlock", title: "Previous block", scope: "day", defaultBinding: "K", menu: true, run: () => moveFocus(stepBlock, -1) },
  { id: "day.prevLane", title: "Block in the lane to the left", scope: "day", defaultBinding: "H", run: () => moveFocus(stepLane, -1) },
  { id: "day.nextLane", title: "Block in the lane to the right", scope: "day", defaultBinding: "L", run: () => moveFocus(stepLane, 1) },
  { id: "day.details", title: "Block details", scope: "day", defaultBinding: "Space", when: hasFocusedBlock, run: () => useStore.getState().toggleDetails() },
  { id: "day.blockMenu", title: "Block actions…", scope: "day", defaultBinding: "Ctrl+Enter", when: hasFocusedBlock, run: () => useStore.getState().requestBlockMenu() },
  { id: "day.selectBlock", title: "Select block", scope: "day", defaultBinding: "V", when: hasFocusedBlock, run: selectFocusedBlock },
  { id: "day.extendNext", title: "Extend selection to the next block", scope: "day", defaultBinding: "Shift+J", run: () => extendToBlock(1) },
  { id: "day.extendPrev", title: "Extend selection to the previous block", scope: "day", defaultBinding: "Shift+K", run: () => extendToBlock(-1) },
  { id: "day.settings", title: "Timeline settings", scope: "day", menu: true, run: () => useStore.getState().openDialog({ type: "timelineSettings" }) },
  { id: "activity.deleteDay", title: "Delete activity for this day…", scope: "day", menu: true, when: () => !!shownDayView(), run: () => openDeleteActivity(false) },
  { id: "activity.deleteRange", title: "Delete activity in a range…", scope: "day", menu: true, when: () => !!shownDayView(), run: () => openDeleteActivity(true) },
  { id: "day.clearFocus", title: "Clear selection", scope: "day", defaultBinding: "Escape", when: () => hasFocusedBlock() || hasSelection() || hasFocusedRecording() || hasFocusedSuggestion(), run: clearDayFocus },
  { id: "recording.next", title: "Next recording or suggestion", scope: "day", defaultBinding: "]", menu: true, run: () => stepEntry(1) },
  { id: "recording.prev", title: "Previous recording or suggestion", scope: "day", defaultBinding: "[", menu: true, run: () => stepEntry(-1) },
  { id: "recording.open", title: "Record selection or edit recording", scope: "day", defaultBinding: "Enter", when: () => hasSelection() || hasFocusedRecording(), run: openRecording },
  { id: "recording.new", title: "New recording…", scope: "global", menu: true, run: newRecording },
  { id: "recording.create", title: "Record selection", scope: "day", menu: true, when: hasSelection, run: () => (useStore.getState().focusRecording(null), openRecording()) },
  { id: "recording.edit", title: "Edit recording", scope: "day", menu: true, when: hasFocusedRecording, run: openRecording },
  { id: "recording.delete", title: "Delete recording", scope: "day", defaultBinding: "Backspace", menu: true, when: hasFocusedRecording, run: () => void useStore.getState().deleteRecording(useStore.getState().focusedRecording ?? "") },
  { id: "recording.nudgeEarlier", title: "Move recording earlier", scope: "day", defaultBinding: "Alt+ArrowUp", when: hasFocusedRecording, run: () => shiftRecording(-1, -1) },
  { id: "recording.nudgeLater", title: "Move recording later", scope: "day", defaultBinding: "Alt+ArrowDown", when: hasFocusedRecording, run: () => shiftRecording(1, 1) },
  { id: "recording.extend", title: "Extend recording", scope: "day", defaultBinding: "Alt+Shift+ArrowDown", when: hasFocusedRecording, run: () => shiftRecording(0, 1) },
  { id: "recording.shrink", title: "Shorten recording", scope: "day", defaultBinding: "Alt+Shift+ArrowUp", when: hasFocusedRecording, run: () => shiftRecording(0, -1) },
  { id: "recording.moveToPrevDay", title: "Move recording to the previous day", scope: "day", defaultBinding: "Alt+ArrowLeft", menu: true, when: hasFocusedRecording, run: () => recordingToDay(-1, false) },
  { id: "recording.moveToNextDay", title: "Move recording to the next day", scope: "day", defaultBinding: "Alt+ArrowRight", menu: true, when: hasFocusedRecording, run: () => recordingToDay(1, false) },
  { id: "recording.copyToPrevDay", title: "Copy recording to the previous day", scope: "day", defaultBinding: "Alt+Shift+ArrowLeft", menu: true, when: hasFocusedRecording, run: () => recordingToDay(-1, true) },
  { id: "recording.copyToNextDay", title: "Copy recording to the next day", scope: "day", defaultBinding: "Alt+Shift+ArrowRight", menu: true, when: hasFocusedRecording, run: () => recordingToDay(1, true) },
  { id: "timer.start", title: "Start timer…", scope: "global", defaultBinding: "Mod+T", menu: true, run: () => void api.timerPickerOpen() },
  { id: "timer.stop", title: "Stop timer", scope: "global", defaultBinding: "Mod+Shift+.", menu: true, when: timerRuns, run: () => void useStore.getState().stopTimer() },
  ...SUGGESTION_COMMANDS,
  { id: "timer.toggle", title: "Start or stop timer", scope: "global", run: () => (timerRuns() ? void useStore.getState().stopTimer() : void api.timerPickerOpen()) },
  { id: "edit.undo", title: "Undo recording change", scope: "global", defaultBinding: "Mod+Z", menu: true, when: undoRedoAvailable, run: () => undoOrRedo(false) },
  { id: "edit.redo", title: "Redo recording change", scope: "global", defaultBinding: "Mod+Shift+Z", menu: true, when: undoRedoAvailable, run: () => undoOrRedo(true) },
  { id: "reports.copyText", title: "Copy report as text", scope: "reports", defaultBinding: "Mod+Shift+C", menu: true, run: () => void copyReport() },
  { id: "reports.copyNotes", title: "Copy report notes", scope: "reports", defaultBinding: "Mod+Alt+C", menu: true, run: () => void copyReportNotes() },
  { id: "reports.save", title: "Save report…", scope: "reports", run: promptSaveReport },
  { id: "reports.rename", title: "Rename saved report…", scope: "reports", when: () => useStore.getState().openedReport !== null, run: promptRenameSavedReport },
  { id: "reports.delete", title: "Delete saved report…", scope: "reports", when: () => useStore.getState().openedReport !== null, run: promptDeleteSavedReport },
  { id: "projects.new", title: "New project…", scope: "projects", defaultBinding: "Mod+N", menu: true, run: () => useStore.getState().openDialog({ type: "project" }) },
  { id: "clients.new", title: "New client…", scope: "projects", menu: true, run: () => useStore.getState().openDialog({ type: "client" }) },
  { id: "tags.new", title: "New tag…", scope: "tags", defaultBinding: "Mod+N", menu: true, run: promptNewTag },
  { id: "rules.new", title: "New rule…", scope: "rules", defaultBinding: "Mod+N", menu: true, run: () => useStore.getState().openDialog({ type: "rule" }) },
  { id: "rules.moveUp", title: "Move rule up", scope: "rules", defaultBinding: "Alt+ArrowUp", when: () => focusedRule() !== null, run: () => moveFocusedRule(-1) },
  { id: "rules.moveDown", title: "Move rule down", scope: "rules", defaultBinding: "Alt+ArrowDown", when: () => focusedRule() !== null, run: () => moveFocusedRule(1) },
  { id: "logs.open", title: "Open log folder", scope: "global", menu: true, run: () => void api.openLogFolder() },
  { id: "data.reveal", title: "Show data folder", scope: "global", menu: true, run: revealDataFolder },
  { id: "data.exportBackup", title: "Export backup…", scope: "global", menu: true, run: exportBackup },
  { id: "help.readme", title: "Open README", scope: "global", menu: true, run: () => void api.openReadme().catch((e) => fail(e, "Couldn't open the README")) },
  { id: "app.quit", title: "Quit TimeTracker", scope: "global", defaultBinding: "Mod+Q", menu: true, run: () => void api.quitApp() },
];

/**
 * Registers the core scopes and the app's commands (again after `clearCommands`) and syncs user overrides
 * from the store. Module scopes must be registered first, or their commands throw.
 */
export function installCommands() {
  registerCoreScopes();
  if (allCommands().length === 0) registerCommands(APP_COMMANDS);
  setOverrides(useStore.getState().keybindings);
}

/** The key label of a command (`⌘K`), or "" when unbound; follows the user's rebinding. */
export function useBindingLabel(id: string): string {
  const custom = useStore((s) => s.keybindings);
  return bindingLabel(bindingFor(id, custom));
}

/** Runs a command by id (the palette and tests use this). */
export function runCommand(id: string): boolean {
  const c = getCommand(id);
  if (!c || c.when?.() === false) return false;
  void c.run();
  return true;
}

/** While a dialog or the palette is open, only these run; other keys belong to the overlay. */
const MODAL_ALLOWED = new Set(["settings.toggle", "shortcuts.show", "palette.toggle", "zoom.in", "zoom.out", "zoom.reset", "window.hide", "app.quit", "edit.undo", "edit.redo"]);

/** Commands that keep firing while their key is held. */
const REPEATABLE = new Set(["zoom.in", "zoom.out", "recording.nudgeEarlier", "recording.nudgeLater", "recording.extend", "recording.shrink"]);

const blockedByOverlay = (id: string) => overlayOpen() && !MODAL_ALLOWED.has(id);

/** Whether a native menu item runs its command now: in an active scope, enabled, not held back by an overlay. */
export function canRunFromMenu(c: Command, scopes: string[] = activeScopes()): boolean {
  return scopes.includes(c.scope) && c.when?.() !== false && !blockedByOverlay(c.id);
}

/** Runs a native menu or tray item's command in an active scope. */
export function runMenuCommand(id: string): boolean {
  const target = getCommand(id);
  if (!target || !canRunFromMenu(target)) return false;
  void target.run();
  return true;
}

/** A context menu takes its own keys, like a dialog. */
const inMenu = (target: EventTarget | null) => target instanceof Element && !!target.closest('[role="menu"]');

/** ↵ and Space activate the focused button or link rather than a command. */
function activatesControl(e: KeyboardEvent): boolean {
  if (e.metaKey || e.ctrlKey || e.altKey || e.shiftKey || (e.key !== "Enter" && e.key !== " ")) return false;
  return e.target instanceof Element && !!e.target.closest('button, a[href], [role="button"], [role="option"]');
}

/** Capture-phase keydown handler for the window. */
export function handleKeydown(e: KeyboardEvent) {
  if (isRecordingTarget(e.target) || activatesControl(e)) return;
  const c = resolveKeydown(e, activeScopes());
  if (!c || blockedByOverlay(c.id) || (inMenu(e.target) && !MODAL_ALLOWED.has(c.id))) return;
  e.preventDefault();
  e.stopPropagation();
  // A held key is swallowed rather than passed on, so the native menu accelerator does not repeat it either.
  if (e.repeat && !REPEATABLE.has(c.id)) return;
  void c.run();
}
