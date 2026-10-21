import { EMPTY_UI_STATE, api, type DayMode, type UiState } from "../../api";
import { fail } from "../requests";
import type { Slice } from "../store";

export interface ViewStateSlice {
  /** Window layout that survives a restart (sidebar, pane sizes, last day, timeline zoom). */
  uiState: UiState;
  loadUiState: () => Promise<void>;
  /** Stores a splitter position (px) under its pane id. */
  setPaneSize: (pane: string, px: number) => void;
  toggleSidebar: () => void;
  rememberDay: (day: string) => void;
  /** px per minute, clamped to the timeline's range. */
  setTimelineZoom: (pxPerMinute: number) => void;
  /** The next zoom step in (`by` 1) or out (-1). */
  stepTimelineZoom: (by: 1 | -1) => void;
  /** The Day view shows the timeline or the list of activity. */
  setDayMode: (dayMode: DayMode) => void;
  /** The welcome sheet's Done. */
  finishOnboarding: () => Promise<void>;
  /** Shows the welcome sheet again ("Show welcome again" in Settings → General). */
  showWelcome: () => void;
}

export const TIMELINE_ZOOM = { min: 1, max: 8, default: 2.5 };
/** The zoom levels the zoom commands step through. */
const TIMELINE_STEPS = [1, 1.5, 2, 2.5, 3, 4, 5, 6, 8];

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

      rememberDay(day) {
        if (get().uiState.lastDay !== day) patchUiState(() => ({ lastDay: day }));
      },

      setTimelineZoom(pxPerMinute) {
        const timelineZoom = Math.min(TIMELINE_ZOOM.max, Math.max(TIMELINE_ZOOM.min, pxPerMinute));
        if (get().uiState.timelineZoom !== timelineZoom) patchUiState(() => ({ timelineZoom }));
      },

      stepTimelineZoom(by) {
        const current = get().uiState.timelineZoom;
        const next = by === 1 ? TIMELINE_STEPS.find((z) => z > current) : [...TIMELINE_STEPS].reverse().find((z) => z < current);
        get().setTimelineZoom(next ?? current);
      },

      setDayMode(dayMode) {
        if (get().uiState.dayMode !== dayMode) patchUiState(() => ({ dayMode }));
      },

      async finishOnboarding() {
        // Patched too, so a layout write still pending can't bring the sheet back.
        patchUiState(() => ({ onboardingDone: true }));
        try {
          await api.onboardingDone();
        } catch (e) {
          fail(e, "Could not save that the welcome was finished");
        }
      },

      showWelcome() {
        patchUiState(() => ({ onboardingDone: false }));
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
