import { EMPTY_UI_STATE, api, type UiState } from "../../api";
import { fail } from "../requests";
import type { Slice } from "../store";

export interface ViewStateSlice {
  /** Window layout that survives a restart (sidebar, pane sizes, last day, zoom). */
  uiState: UiState;
  loadUiState: () => Promise<void>;
  /** Stores a splitter position (px) under its pane id. */
  setPaneSize: (pane: string, px: number) => void;
  toggleSidebar: () => void;
}

export const viewStateSlice: Slice<ViewStateSlice> = {
  create: (set, get) => {
    const patchUiState = (patch: (current: UiState) => Partial<UiState>) => {
      const ui: UiState = { ...get().uiState, ...patch(get().uiState) };
      set({ uiState: ui });
      persistUiState(ui);
    };

    return {
      uiState: EMPTY_UI_STATE,

      async loadUiState() {
        try {
          set({ uiState: await api.getUiState() });
        } catch (e) {
          fail(e, "Could not load the window layout");
        }
      },

      setPaneSize(pane, px) {
        patchUiState((current) => ({ paneSizes: { ...current.paneSizes, [pane]: Math.round(px) } }));
      },

      toggleSidebar() {
        patchUiState((current) => ({ sidebarHidden: !current.sidebarHidden }));
      },
    };
  },
};

let pending: { ui: UiState; timer: ReturnType<typeof setTimeout> } | null = null;

/** Debounces layout writes (drags fire constantly): a write lands 500 ms after the last change. */
function persistUiState(ui: UiState) {
  if (pending) clearTimeout(pending.timer);
  pending = { ui, timer: setTimeout(writeUiState, 500) };
}

function writeUiState() {
  if (!pending) return;
  const { ui } = pending;
  pending = null;
  void api.setUiState(ui).catch((e) => fail(e, "Could not save the window layout"));
}

/** Writes a debounced layout change now; called when the window unloads. */
export function flushUiState() {
  if (pending) clearTimeout(pending.timer);
  writeUiState();
}
