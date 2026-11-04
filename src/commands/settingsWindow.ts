/** The Settings window's own keys: a scope that is active only there, and its keydown handler. The main
 *  window's commands act on its views, so they never run in Settings. */
import { api } from "../api";
import { isSettingsWindow } from "../lib/settingsWindow";
import { isRecordingTarget, isTextInput, resolveKeydown, type Command } from "./registry";
import { registerScope } from "./scopes";

const SETTINGS_SCOPE = "settings";

export function registerSettingsScope() {
  registerScope({ id: SETTINGS_SCOPE, parent: "global", label: "Settings window", isActive: isSettingsWindow });
}

const searchField = () => document.querySelector<HTMLInputElement>('input[aria-label="Search Settings"]');

/** Escape belongs to a focused text field, an open sheet or an open menu first. */
const canClose = () => !isTextInput(document.activeElement) && !document.querySelector('[role="dialog"], [role="menu"]');

export const SETTINGS_COMMANDS: Command[] = [
  { id: "settings.search", title: "Search Settings", scope: SETTINGS_SCOPE, defaultBinding: "Mod+F", run: () => searchField()?.focus() },
  { id: "settings.close", title: "Close Settings", scope: SETTINGS_SCOPE, defaultBinding: "Escape", when: canClose, run: () => void api.hideWindow() },
];

/** Capture-phase keydown handler of the Settings window: runs only the `settings` scope's commands. */
export function handleSettingsKeydown(e: KeyboardEvent) {
  if (isRecordingTarget(e.target)) return;
  const c = resolveKeydown(e, [SETTINGS_SCOPE]);
  if (!c) return;
  e.preventDefault();
  e.stopPropagation();
  if (!e.repeat) void c.run();
}
