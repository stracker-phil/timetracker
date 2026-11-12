/** Settings pages by id (ADR 0034) and the search over their rows (ADR 0049). Core and modules register their
 *  own pages; the Settings window and the palette read them here. */
import { useSyncExternalStore, type ComponentType, type ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import type { Settings } from "../api";
import { settingsScore } from "../lib/rank";

export interface SettingsPageProps {
  s: Settings;
  /** Saves `{ ...current, ...patch }` quietly; a failure is toasted. */
  set: (patch: Partial<Settings>) => void;
  /** `set` for a field (`fields.tsx`): resolves Rust's message when it refused the value (`invalid_input`),
   *  for the field to show instead of a toast; else null. */
  commit: (patch: Partial<Settings>) => Promise<string | null>;
}

/** A row the search finds: `id` is the row's `anchor`, `title` its label. */
export interface SettingsRow {
  id: string;
  title: string;
  /** Synonyms the search finds the row by; the label itself is the title. */
  keywords?: string[];
  /** The anchor revealed instead while this row isn't shown (a row that depends on a switch). */
  under?: string;
}

interface SettingsPage {
  id: string;
  group: string;
  label: string;
  icon: LucideIcon;
  intro?: ReactNode;
  /** A page that keeps its own state (a module's settings file) ignores the props. */
  Body: ComponentType<SettingsPageProps>;
  /** Position inside its group; pages without one follow, by label. */
  order?: number;
  /** Synonyms the search finds the page by. */
  keywords?: string[];
  /** Every anchored row of the page; a function for rows built from data (Shortcuts lists the commands). */
  rows?: SettingsRow[] | (() => SettingsRow[]);
}

/** Nav order of the known groups; any other group follows them, by name. */
const GROUP_ORDER = ["App", "Tracking", "System"];

const pages = new Map<string, SettingsPage>();
const listeners = new Set<() => void>();
let version = 0;

/** Adds a page, or replaces the one with the same id. */
export function registerSettingsPage(page: SettingsPage): void {
  pages.set(page.id, page);
  version += 1;
  listeners.forEach((notify) => notify());
}

function groupRank(group: string): number {
  const index = GROUP_ORDER.indexOf(group);
  return index === -1 ? GROUP_ORDER.length : index;
}

function compare(a: SettingsPage, b: SettingsPage): number {
  const byGroup = groupRank(a.group) - groupRank(b.group) || a.group.localeCompare(b.group);
  if (byGroup !== 0) return byGroup;
  const rank = (p: SettingsPage) => p.order ?? Number.MAX_SAFE_INTEGER;
  return rank(a) - rank(b) || a.label.localeCompare(b.label);
}

/** Every page in nav order: by group, then `order`, then label. */
export function settingsPages(): SettingsPage[] {
  return [...pages.values()].sort(compare);
}

export function pageRows(page: SettingsPage): SettingsRow[] {
  return typeof page.rows === "function" ? page.rows() : (page.rows ?? []);
}

const findRow = (page: SettingsPage, id: string | undefined): SettingsRow | undefined => pageRows(page).find((row) => row.id === id);

/** A page, and the row on it when the request named one. */
export interface SettingsMatch {
  page: SettingsPage;
  row?: SettingsRow;
}

/** The named page, the page holding the named row (with that row), or the first page when the name is absent or
 *  unknown; undefined while no page is registered. */
export function resolveSettingsPage(id: string | undefined): SettingsMatch | undefined {
  const all = settingsPages();
  const named = all.find((page) => page.id === id);
  if (named) return { page: named };
  for (const page of all) {
    const row = findRow(page, id);
    if (row) return { page, row };
  }
  return all.length > 0 ? { page: all[0] } : undefined;
}

function matchScore({ page, row }: SettingsMatch, needle: string): number {
  return row ? settingsScore(row.title, row.keywords ?? [], needle) : settingsScore(page.label, [page.group, ...(page.keywords ?? [])], needle);
}

/** Every page and row matching `query`, best first; ties keep the nav order, a page before its rows. Empty for a
 *  blank query. */
export function searchSettings(query: string): SettingsMatch[] {
  const needle = query.trim();
  if (!needle) return [];
  return settingsPages()
    .flatMap((page): SettingsMatch[] => [{ page }, ...pageRows(page).map((row) => ({ page, row }))])
    .map((match) => ({ match, score: matchScore(match, needle) }))
    .filter((entry) => entry.score > 0)
    .sort((a, b) => b.score - a.score)
    .map((entry) => entry.match);
}

export function settingsGroups(): { group: string; pages: SettingsPage[] }[] {
  const groups: { group: string; pages: SettingsPage[] }[] = [];
  for (const page of settingsPages()) {
    const last = groups.at(-1);
    if (last?.group === page.group) last.pages.push(page);
    else groups.push({ group: page.group, pages: [page] });
  }
  return groups;
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => void listeners.delete(listener);
}

/** Re-renders the caller when a page is registered, so a module may register after the window opened. */
export function useSettingsPagesVersion(): number {
  return useSyncExternalStore(subscribe, () => version);
}
