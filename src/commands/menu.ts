import { api, type MenuState } from "../api";
import { useStore } from "../state/store";
import { canRunFromMenu } from "./pageCommands";
import { allCommands } from "./registry";
import { activeScopes } from "./scopes";
import { queryClient } from "../queries/queryClient";

/** This window's menu page commands (`page:<id>` in `menu/items.rs`) that can run now and the checked ones. */
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

/** Keeps the menu in step with the store, the loaded data (the shown day's suggestions) and the keyboard focus (a text field). */
export function installMenuSync(): () => void {
  sent = "";
  const onFocus = () => queueMicrotask(syncMenu);
  const unsubscribe = useStore.subscribe(syncMenu);
  const unsubscribeData = queryClient.getQueryCache().subscribe((event) => {
    if (event.type === "updated") syncMenu();
  });
  window.addEventListener("focusin", onFocus);
  window.addEventListener("focusout", onFocus);
  syncMenu();
  return () => {
    unsubscribe();
    unsubscribeData();
    window.removeEventListener("focusin", onFocus);
    window.removeEventListener("focusout", onFocus);
  };
}
