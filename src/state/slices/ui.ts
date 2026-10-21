import { api } from "../../api";
import { formatDay } from "../../lib/format";
import type { Slice } from "../store";

export interface Toast {
  id: number;
  kind: "error" | "info" | "success";
  message: string;
  /** How many times the same message was reported while it was showing. */
  count: number;
}

export type Dialog =
  /** `page`: the Settings page shown (ids from `settingsPages()` in `registry/settingsPages.ts`); the first one when absent. */
  | { type: "settings"; page?: string }
  | { type: "confirm"; title: string; message: string; confirmLabel: string; onConfirm: () => void }
  /** The shortcut overview of the scopes that were active when it opened. */
  | { type: "shortcuts"; scopes: string[] };

export type View = "day" | "week" | "month" | "reports" | "projects";

export interface UiSlice {
  toasts: Toast[];
  dialog: Dialog | null;
  paletteOpen: boolean;
  /** Text the palette opens with, e.g. `>` for commands. */
  paletteSeed: string;
  view: View;
  /** The day the Day view shows (`YYYY-MM-DD`). */
  day: string;
  toast: (kind: Toast["kind"], message: string) => void;
  dismissToast: (id: number) => void;
  openDialog: (dialog: Dialog) => void;
  closeDialog: () => void;
  setPaletteOpen: (open: boolean, seed?: string) => void;
  setView: (view: View) => void;
  /** Shows `day` in the Day view. */
  goToDay: (day: string) => void;
}

let nextToastId = 1;

export const uiSlice: Slice<UiSlice> = {
  create: (set, get) => ({
    toasts: [],
    dialog: null,
    paletteOpen: false,
    paletteSeed: "",
    view: "day",
    day: formatDay(Date.now()),

    toast(kind, message) {
      if (kind === "error") void api.logError(message).catch(() => {});
      const same = get().toasts.find((t) => t.kind === kind && t.message === message);
      if (same) {
        set((s) => ({ toasts: s.toasts.map((t) => (t.id === same.id ? { ...t, count: t.count + 1 } : t)) }));
        return;
      }
      const id = nextToastId++;
      set((s) => ({ toasts: [...s.toasts.slice(-4), { id, kind, message, count: 1 }] }));
    },

    dismissToast(id) {
      set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
    },

    openDialog(dialog) {
      set({ dialog });
    },

    closeDialog() {
      set({ dialog: null });
    },

    setPaletteOpen(paletteOpen, seed = "") {
      set({ paletteOpen, paletteSeed: seed });
    },

    setView(view) {
      set({ view });
    },

    goToDay(day) {
      set({ view: "day", day });
    },
  }),
};
