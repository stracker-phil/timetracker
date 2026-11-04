/** The page's side of the menus' `page:<id>` items (ADR 0038, 0039): which page commands can run now, running
 *  them when Rust asks (`page_command`, links, `timetracker run`), and what the palette may list of them. */
import { api } from "../api";
import { useStore } from "../state/store";
import { allCommands, getCommand, type Command } from "./registry";
import { activeScopes } from "./scopes";

export function overlayOpen(): boolean {
  const s = useStore.getState();
  return s.dialog !== null || s.paletteOpen || s.editor !== null;
}

/** While a dialog or the palette is open, only these run; other keys belong to the overlay. */
export const MODAL_ALLOWED = new Set(["app:settings", "shortcuts.show", "palette.toggle", "zoom.in", "zoom.out", "zoom.reset", "window:close", "app:background", "app:quit", "edit.undo", "edit.redo"]);

export const blockedByOverlay = (id: string) => overlayOpen() && !MODAL_ALLOWED.has(id);

/** Whether a menu item runs its page command now: in an active scope, enabled, not held back by an overlay. */
export function canRunFromMenu(c: Command, scopes: string[] = activeScopes()): boolean {
  return scopes.includes(c.scope) && c.when?.() !== false && !blockedByOverlay(c.id);
}

/** Runs a page command from a menu (`page:<id>`), a link or `timetracker run`, when it can run now. */
export function runMenuCommand(id: string): boolean {
  const target = getCommand(id);
  if (!target || !canRunFromMenu(target)) return false;
  void target.run();
  return true;
}

/** Runs the menus' page commands this window gets (`page_command`). */
export const listenPageCommands = () => api.onPageCommand(runMenuCommand);

let paletteRunnable: ReadonlySet<string> = new Set();

/** Notes the menu page commands that can run, before the palette opens: open, it holds them back, and it
 *  takes the focus some of them read. */
export function notePaletteRunnable() {
  const scopes = activeScopes();
  paletteRunnable = new Set(
    allCommands()
      .filter((c) => c.menu && canRunFromMenu(c, scopes))
      .map((c) => c.id),
  );
}

/** Whether the palette lists the menu item `page:<id>`: its command could run when the palette opened. */
export const paletteCanRun = (id: string) => paletteRunnable.has(id);
