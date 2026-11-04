import { type RefObject, useEffect, useRef } from "react";
import type { MenuEntry, MenuItem } from "../menu";

/** A menu on screen the palette lists (ADR 0039), e.g. the Day toolbar's ⋯ or the status bar's Pause. */
export interface PaletteMenu {
  /** The palette's section. */
  section: string;
  /** Before each item's text: "Pause tracking" → "Pause tracking: For 1 hour". */
  title: string;
  entries: () => MenuEntry[];
}

/** One chosen row of a menu on screen. */
export interface PaletteMenuItem {
  key: string;
  section: string;
  title: string;
  run: () => void;
}

const menus = new Map<string, RefObject<PaletteMenu | null>>();

/** Offers `menu` under `id` while mounted; the latest registration wins, null offers nothing. */
export function usePaletteMenu(id: string, menu: PaletteMenu | null) {
  const latest = useRef(menu);
  useEffect(() => {
    latest.current = menu;
  });
  const offered = menu !== null;
  useEffect(() => {
    if (!offered) return;
    menus.set(id, latest);
    return () => {
      if (menus.get(id) === latest) menus.delete(id);
    };
  }, [id, offered]);
}

/** A menu's rows, submenus flattened ("Pause tracking: For 1 hour"); captions, separators and greyed rows drop. */
function flatten(menu: PaletteMenu, entries: MenuEntry[], path: string[] = []): PaletteMenuItem[] {
  return entries.flatMap((entry): PaletteMenuItem[] => {
    if (entry === "separator" || "caption" in entry) return [];
    const item: MenuItem = entry;
    if (item.enabled === false) return [];
    const at = [...path, item.text];
    if (item.items) return flatten(menu, item.items, at);
    const run = item.run;
    if (!run) return [];
    const title = `${menu.title}: ${at.join(" › ")}`;
    return [{ key: `menu:${menu.section}:${title}`, section: menu.section, title, run }];
  });
}

let noted: PaletteMenuItem[] = [];

/** Notes the rows of the menus on screen, before the palette opens over them. */
export function notePaletteMenus() {
  noted = [...menus.values()].flatMap((menu) => (menu.current ? flatten(menu.current, menu.current.entries()) : []));
}

/** The rows `notePaletteMenus` noted, in registration order. */
export const paletteMenuItems = (): readonly PaletteMenuItem[] => noted;
