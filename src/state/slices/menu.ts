import type { ReactNode } from "react";
import type { Slice } from "../store";

/** A row of a menu: runs `run` when chosen, or opens `items` as a submenu. */
export interface MenuItem {
  text: string;
  run?: () => void;
  /** False greys the row out; it can't be chosen and the keys skip it. */
  enabled?: boolean;
  /** Makes the row a choice; true shows the check. */
  checked?: boolean;
  /** With `checked`: an on/off switch rather than one choice of a group. */
  toggle?: boolean;
  /** A destructive action, in red. */
  danger?: boolean;
  /** At the trailing end: a short hint or a key. */
  hint?: string;
  /** A second line under the text. */
  detail?: string;
  /** The tooltip, e.g. why the row is disabled. */
  title?: string;
  /** Before the text: a line icon or a dot. */
  icon?: ReactNode;
  items?: MenuEntry[];
}

/** A row, a separator line, or a caption that starts a group of rows. */
export type MenuEntry = MenuItem | "separator" | { caption: string };

/** Where a menu opens: at a point (a right-click) or under an element (a button, which also gets the focus back). */
export type MenuAt = { x: number; y: number } | HTMLElement;

export interface OpenMenu {
  /** Tells menus apart, so a new one starts fresh. */
  id: number;
  entries: MenuEntry[];
  at: MenuAt;
  /** Its accessible name. */
  label: string;
  /** Where the focus goes back to when it closes. */
  opener: HTMLElement | null;
}

export interface MenuSlice {
  menu: OpenMenu | null;
  /** Shows a menu, replacing an open one; opening it again on the same element closes it. */
  openMenu: (entries: MenuEntry[], at: MenuAt, label?: string) => void;
  closeMenu: () => void;
}

let opened = 0;

export const menuSlice: Slice<MenuSlice> = {
  create: (set, get) => ({
    menu: null,
    openMenu: (entries, at, label = "Menu") => {
      if (at instanceof HTMLElement && get().menu?.at === at) return set({ menu: null });
      const focused = document.activeElement;
      const opener = at instanceof HTMLElement ? at : focused instanceof HTMLElement && focused !== document.body ? focused : null;
      opened += 1;
      set({ menu: { id: opened, entries, at, label, opener } });
    },
    closeMenu: () => set({ menu: null }),
  }),
};
