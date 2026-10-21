import { api, TEXT_SIZES, type Settings } from "../api";
import { copyReport, promptSaveReport } from "../components/reports/actions";
import { VIEWS } from "../components/views";
import { stepBlock, stepLane } from "../lib/blockFocus";
import { TIMELINE_ZOOM, useStore, type View } from "../state/store";
import { allCommands, bindingFor, bindingLabel, getCommand, isRecordingTarget, isTextInput, registerCommands, resolveKeydown, setOverrides, type Command } from "./registry";
import { activeScopes, registerScope } from "./scopes";

const CALENDAR_VIEWS: View[] = ["day", "week", "month"];

export * from "./registry";
export * from "./scopes";

/** Registers the core scopes; safe to call again (tests and re-installs do). A view's scope is active while it shows. */
export function registerCoreScopes() {
  registerScope({ id: "global", parent: null, label: "Everywhere", isActive: () => true });
  registerScope({ id: "calendar", parent: "global", label: "Day, Week and Month", isActive: () => CALENDAR_VIEWS.includes(useStore.getState().view) });
  for (const { id, label } of VIEWS) {
    const parent = CALENDAR_VIEWS.includes(id) ? "calendar" : "global";
    registerScope({ id, parent, label, isActive: () => useStore.getState().view === id });
  }
}

function saveTextSizes(sizes: (current: Settings) => Pick<Settings, "uiFontSize" | "dataFontSize">) {
  const s = useStore.getState();
  if (s.settings) void s.saveSettings({ ...s.settings, ...sizes(s.settings) }, { quiet: true });
}

/** Rust's `Settings::sanitized` clamps the sizes, so zooming past a bound is a no-op. */
const zoomText = (step: number) => saveTextSizes((c) => ({ uiFontSize: c.uiFontSize + step, dataFontSize: c.dataFontSize + step }));
const resetText = () => saveTextSizes(() => ({ uiFontSize: TEXT_SIZES.ui.default, dataFontSize: TEXT_SIZES.data.default }));

const onTimeline = () => useStore.getState().view === "day";
// The Day view zooms its timeline; every other view zooms the text.
const zoomIn = () => (onTimeline() ? useStore.getState().stepTimelineZoom(1) : zoomText(1));
const zoomOut = () => (onTimeline() ? useStore.getState().stepTimelineZoom(-1) : zoomText(-1));
const resetZoom = () => (onTimeline() ? useStore.getState().setTimelineZoom(TIMELINE_ZOOM.default) : resetText());

const timerRuns = () => !!useStore.getState().timer?.running;

/** Moves the block focus within the shown day's view. */
function moveFocus(step: typeof stepBlock, by: 1 | -1) {
  const s = useStore.getState();
  if (s.dayView?.day === s.day) s.focusBlock(step(s.dayView, s.focusedBlock, by));
}

const hasFocusedBlock = () => useStore.getState().focusedBlock !== null;
const hasSelection = () => useStore.getState().selection !== null;
const hasFocusedRecording = () => useStore.getState().focusedRecording !== null;

/** Edits the focused recording, else opens the editor for the selection. */
function openRecording() {
  const s = useStore.getState();
  if (s.focusedRecording) return s.openEditor({ mode: "edit", id: s.focusedRecording });
  const span = s.selectionSnap ?? s.selection;
  if (span) s.openEditor({ mode: "create", start: span.start, end: span.end });
}

function clearDayFocus() {
  const s = useStore.getState();
  s.focusBlock(null);
  s.setSelection(null);
  s.focusRecording(null);
}

/** Moves the focused recording's start and end by whole snap steps. */
function shiftRecording(startSteps: number, endSteps: number) {
  const s = useStore.getState();
  const block = s.dayView?.recordings.find((r) => r.id === s.focusedRecording);
  if (!block || block.clippedStart || block.clippedEnd || block.running) return;
  const step = (s.settings?.timeline.snapMinutes ?? 5) * 60_000;
  void s.updateRecording(block.id, { start: block.start + startSteps * step, end: block.end + endSteps * step }).catch(() => {});
}

/** ⌘Z in a text field undoes typing, not a recording change. */
const outsideTextField = () => !isTextInput(document.activeElement);

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
  { id: "shortcuts.show", title: "Keyboard shortcuts", scope: "global", defaultBinding: "Shift+?", menu: true, run: toggleShortcuts },
  { id: "palette.toggle", title: "Command palette", scope: "global", defaultBinding: "Mod+K", menu: true, run: togglePalette },
  { id: "sidebar.toggle", title: "Toggle sidebar", scope: "global", defaultBinding: "Mod+Ctrl+S", menu: true, run: () => useStore.getState().toggleSidebar() },
  ...VIEWS.map(({ id, label }, i): Command => ({ id: `view.${id}`, title: `Show ${label}`, scope: "global", defaultBinding: `Mod+${i + 1}`, menu: true, run: () => useStore.getState().setView(id) })),
  { id: "zoom.in", title: "Zoom in", scope: "global", defaultBinding: "Mod+=", menu: true, run: zoomIn },
  { id: "zoom.out", title: "Zoom out", scope: "global", defaultBinding: "Mod+-", menu: true, run: zoomOut },
  { id: "zoom.reset", title: "Reset zoom", scope: "global", defaultBinding: "Mod+0", menu: true, run: resetZoom },
  { id: "window.hide", title: "Close window", scope: "global", defaultBinding: "Mod+W", menu: true, run: () => void api.hideWindow() },
  { id: "tracker.pause", title: "Pause tracking", scope: "global", when: () => isTracking(), run: () => void useStore.getState().pauseTracker("resumed") },
  { id: "tracker.resume", title: "Resume tracking", scope: "global", when: () => useStore.getState().tracker?.state.kind === "paused", run: () => void useStore.getState().resumeTracker() },
  { id: "nav.prev", title: "Previous day, week or month", scope: "calendar", defaultBinding: "Mod+ArrowLeft", run: () => useStore.getState().step(-1) },
  { id: "nav.next", title: "Next day, week or month", scope: "calendar", defaultBinding: "Mod+ArrowRight", run: () => useStore.getState().step(1) },
  { id: "nav.today", title: "Go to today", scope: "calendar", defaultBinding: "T", run: () => useStore.getState().goToday() },
  { id: "day.nextBlock", title: "Next block", scope: "day", defaultBinding: "J", run: () => moveFocus(stepBlock, 1) },
  { id: "day.prevBlock", title: "Previous block", scope: "day", defaultBinding: "K", run: () => moveFocus(stepBlock, -1) },
  { id: "day.prevLane", title: "Block in the lane to the left", scope: "day", defaultBinding: "H", run: () => moveFocus(stepLane, -1) },
  { id: "day.nextLane", title: "Block in the lane to the right", scope: "day", defaultBinding: "L", run: () => moveFocus(stepLane, 1) },
  { id: "day.details", title: "Block details", scope: "day", defaultBinding: "Space", when: hasFocusedBlock, run: () => useStore.getState().toggleDetails() },
  { id: "day.blockMenu", title: "Block actions…", scope: "day", defaultBinding: "Ctrl+Enter", when: hasFocusedBlock, run: () => useStore.getState().requestBlockMenu() },
  { id: "day.settings", title: "Timeline settings", scope: "day", run: () => useStore.getState().openDialog({ type: "timelineSettings" }) },
  { id: "day.clearFocus", title: "Clear selection", scope: "day", defaultBinding: "Escape", when: () => hasFocusedBlock() || hasSelection() || hasFocusedRecording(), run: clearDayFocus },
  { id: "recording.open", title: "Record selection or edit recording", scope: "day", defaultBinding: "Enter", when: () => hasSelection() || hasFocusedRecording(), run: openRecording },
  { id: "recording.create", title: "New recording from selection", scope: "day", menu: true, when: hasSelection, run: () => (useStore.getState().focusRecording(null), openRecording()) },
  { id: "recording.edit", title: "Edit recording", scope: "day", menu: true, when: hasFocusedRecording, run: openRecording },
  { id: "recording.delete", title: "Delete recording", scope: "day", defaultBinding: "Backspace", menu: true, when: hasFocusedRecording, run: () => void useStore.getState().deleteRecording(useStore.getState().focusedRecording ?? "") },
  { id: "recording.nudgeEarlier", title: "Move recording earlier", scope: "day", defaultBinding: "Alt+ArrowUp", when: hasFocusedRecording, run: () => shiftRecording(-1, -1) },
  { id: "recording.nudgeLater", title: "Move recording later", scope: "day", defaultBinding: "Alt+ArrowDown", when: hasFocusedRecording, run: () => shiftRecording(1, 1) },
  { id: "recording.extend", title: "Extend recording", scope: "day", defaultBinding: "Alt+Shift+ArrowDown", when: hasFocusedRecording, run: () => shiftRecording(0, 1) },
  { id: "recording.shrink", title: "Shorten recording", scope: "day", defaultBinding: "Alt+Shift+ArrowUp", when: hasFocusedRecording, run: () => shiftRecording(0, -1) },
  { id: "timer.start", title: "Start timer…", scope: "global", defaultBinding: "Mod+T", menu: true, run: () => void api.timerPickerOpen() },
  { id: "timer.stop", title: "Stop timer", scope: "global", defaultBinding: "Mod+Shift+.", menu: true, when: timerRuns, run: () => void useStore.getState().stopTimer() },
  { id: "timer.toggle", title: "Start or stop timer", scope: "global", run: () => (timerRuns() ? void useStore.getState().stopTimer() : void api.timerPickerOpen()) },
  { id: "edit.undo", title: "Undo recording change", scope: "global", defaultBinding: "Mod+Z", when: outsideTextField, run: () => void useStore.getState().undo() },
  { id: "edit.redo", title: "Redo recording change", scope: "global", defaultBinding: "Mod+Shift+Z", when: outsideTextField, run: () => void useStore.getState().redo() },
  { id: "reports.copyText", title: "Copy report as text", scope: "reports", defaultBinding: "Mod+Shift+C", run: () => void copyReport() },
  { id: "reports.save", title: "Save report…", scope: "reports", run: promptSaveReport },
  { id: "projects.new", title: "New project…", scope: "projects", defaultBinding: "Mod+N", run: () => useStore.getState().openDialog({ type: "project" }) },
  { id: "clients.new", title: "New client…", scope: "projects", run: () => useStore.getState().openDialog({ type: "client" }) },
  { id: "logs.open", title: "Open log folder", scope: "global", menu: true, run: () => void api.openLogFolder() },
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
const MODAL_ALLOWED = new Set(["settings.toggle", "shortcuts.show", "palette.toggle", "zoom.in", "zoom.out", "zoom.reset", "window.hide", "app.quit"]);

/** Commands that keep firing while their key is held. */
const REPEATABLE = new Set(["zoom.in", "zoom.out", "recording.nudgeEarlier", "recording.nudgeLater", "recording.extend", "recording.shrink"]);

function blockedByOverlay(id: string): boolean {
  const s = useStore.getState();
  return (s.dialog !== null || s.paletteOpen || s.editor !== null) && !MODAL_ALLOWED.has(id);
}

/** Runs a native menu or tray item's command in an active scope. */
export function runMenuCommand(id: string): boolean {
  const target = getCommand(id);
  if (!target || !activeScopes().includes(target.scope) || target.when?.() === false || blockedByOverlay(id)) return false;
  void target.run();
  return true;
}

/** Capture-phase keydown handler for the window. */
export function handleKeydown(e: KeyboardEvent) {
  if (isRecordingTarget(e.target)) return;
  const c = resolveKeydown(e, activeScopes());
  if (!c || blockedByOverlay(c.id)) return;
  e.preventDefault();
  e.stopPropagation();
  // A held key is swallowed rather than passed on, so the native menu accelerator does not repeat it either.
  if (e.repeat && !REPEATABLE.has(c.id)) return;
  void c.run();
}
