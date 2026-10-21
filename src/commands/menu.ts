import { api, type MenuState } from "../api";
import { useStore } from "../state/store";
import { activeScopes, allCommands, canRunFromMenu } from "./index";

/** The native menu's items that can run now and the checked ones, as `menu/items.rs` lists them. */
export function menuState(): MenuState {
  const scopes = activeScopes();
  const items = allCommands().filter((c) => c.menu);
  return {
    enabled: items.filter((c) => canRunFromMenu(c, scopes)).map((c) => c.id),
    checked: items.filter((c) => c.checked?.()).map((c) => c.id),
  };
}

let sent = "";

/** Reports the menu state to Rust when it changed since the last report. */
export function syncMenu() {
  if (allCommands().length === 0) return;
  const state = menuState();
  const key = JSON.stringify(state);
  if (key === sent) return;
  sent = key;
  api.menuSync(state).catch(() => (sent = ""));
}

/** Keeps the menu in step with the store and with the keyboard focus (a focused suggestion, a text field). */
export function installMenuSync(): () => void {
  sent = "";
  const onFocus = () => queueMicrotask(syncMenu);
  const unsubscribe = useStore.subscribe(syncMenu);
  window.addEventListener("focusin", onFocus);
  window.addEventListener("focusout", onFocus);
  syncMenu();
  return () => {
    unsubscribe();
    window.removeEventListener("focusin", onFocus);
    window.removeEventListener("focusout", onFocus);
  };
}
