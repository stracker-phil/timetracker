import type { MenuEntry, MenuItem } from "../../state/slices/menu";

export const isItem = (entry: MenuEntry): entry is MenuItem => entry !== "separator" && !("caption" in entry);
export const isEnabled = (entry: MenuEntry | undefined): entry is MenuItem => !!entry && isItem(entry) && entry.enabled !== false;
const textOf = (entry: MenuEntry | undefined) => (entry && isItem(entry) ? entry.text : "");

/** The enabled row `by` steps from `active`, wrapping; from none, the first (↓) or the last (↑). */
export function nextEnabled(enabled: number[], active: number, by: 1 | -1): number {
  if (enabled.length === 0) return active;
  const at = enabled.indexOf(active);
  const next = at === -1 ? (by === 1 ? 0 : enabled.length - 1) : (at + by + enabled.length) % enabled.length;
  return enabled[next];
}

/** The next enabled row after `active` whose text starts with `letter`, ignoring case; undefined without one. */
export function rowStartingWith(entries: MenuEntry[], enabled: number[], active: number, letter: string): number | undefined {
  const from = enabled.indexOf(active);
  const order = [...enabled.slice(from + 1), ...enabled.slice(0, from + 1)];
  const needle = letter.toLowerCase();
  return order.find((index) => textOf(entries[index]).toLowerCase().startsWith(needle));
}
