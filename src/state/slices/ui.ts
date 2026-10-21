import { api, type Recording, type RuleInput } from "../../api";
import type { Slice } from "../store";

export interface Toast {
  id: number;
  kind: "error" | "info" | "success";
  message: string;
  /** How many times the same message was reported while it was showing. */
  count: number;
  /** A button in the toast, e.g. Undo; running it dismisses the toast. */
  action?: ToastAction;
}

export interface ToastAction {
  label: string;
  run: () => void;
}

export type Dialog =
  /** `page`: the Settings page shown (ids from `settingsPages()` in `registry/settingsPages.ts`); the first one when absent. */
  | { type: "settings"; page?: string }
  | { type: "confirm"; title: string; message: string; confirmLabel: string; onConfirm: () => void }
  /** The shortcut overview of the scopes that were active when it opened. */
  | { type: "shortcuts"; scopes: string[] }
  /** Edit a project (`id`) or create one, for `clientId` when given. */
  | { type: "project"; id?: string; clientId?: string }
  /** Edit a rule (`id`) or create one, starting from `draft` when given. */
  | { type: "rule"; id?: string; draft?: RuleInput }
  /** Edit a client (`id`) or create one. */
  | { type: "client"; id?: string }
  /** One line of text, e.g. a name; `onSubmit` gets it trimmed and never blank. */
  | { type: "prompt"; title: string; label: string; initial: string; confirmLabel: string; onSubmit: (value: string) => void }
  /** The Day header's timeline settings. */
  | { type: "timelineSettings" }
  /** A recording write overlaps `recordings`; replace their overlapped part, or cancel. */
  | { type: "overlap"; recordings: Recording[]; onReplace: () => void; onCancel: () => void };

export interface UiSlice {
  toasts: Toast[];
  dialog: Dialog | null;
  paletteOpen: boolean;
  /** Text the palette opens with, e.g. `>` for commands. */
  paletteSeed: string;
  toast: (kind: Toast["kind"], message: string, action?: ToastAction) => void;
  dismissToast: (id: number) => void;
  openDialog: (dialog: Dialog) => void;
  closeDialog: () => void;
  setPaletteOpen: (open: boolean, seed?: string) => void;
}

let nextToastId = 1;

export const uiSlice: Slice<UiSlice> = {
  create: (set, get) => ({
    toasts: [],
    dialog: null,
    paletteOpen: false,
    paletteSeed: "",

    toast(kind, message, action) {
      if (kind === "error") void api.logError(message).catch(() => {});
      const same = get().toasts.find((t) => t.kind === kind && t.message === message);
      if (same) {
        set((s) => ({ toasts: s.toasts.map((t) => (t.id === same.id ? { ...t, count: t.count + 1 } : t)) }));
        return;
      }
      const id = nextToastId++;
      set((s) => ({ toasts: [...s.toasts.slice(-4), { id, kind, message, count: 1, action }] }));
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
  }),
};
