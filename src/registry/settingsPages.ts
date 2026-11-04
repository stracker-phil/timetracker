/** Settings pages by id. Core and modules register their own; the Settings window reads them here. */
import { useSyncExternalStore, type ComponentType, type ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import type { Settings } from "../api";

export interface SettingsPageProps {
  s: Settings;
  /** Saves `{ ...current, ...patch }` quietly; a failure is toasted. */
  set: (patch: Partial<Settings>) => void;
  /** `set` for a field (`fields.tsx`): resolves Rust's message when it refused the value (`invalid_input`),
   *  for the field to show instead of a toast; else null. */
  commit: (patch: Partial<Settings>) => Promise<string | null>;
}

/** A group of a page (`Group anchor`): `open_settings` with its id opens the page scrolled to it. */
export interface SettingsSection {
  id: string;
  title: string;
  /** The group's row labels and other words the search finds it by. */
  keywords?: string[];
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
  sections?: SettingsSection[];
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
  const byOrder = rank(a) - rank(b);
  if (byOrder !== 0) return byOrder;
  return a.label.localeCompare(b.label);
}

/** Every page in nav order: by group, then `order`, then label. */
export function settingsPages(): SettingsPage[] {
  return [...pages.values()].sort(compare);
}

const hasSection = (page: SettingsPage, id: string | undefined) => page.sections?.some((section) => section.id === id) ?? false;

/** The named page, the page holding the named section (`section`: its id), or the first when the name is absent
 *  or unknown; undefined while no page is registered. */
export function resolveSettingsPage(idOrSection: string | undefined): { page: SettingsPage; section?: string } | undefined {
  const all = settingsPages();
  const named = all.find((p) => p.id === idOrSection);
  if (named) return { page: named };
  const holder = all.find((p) => hasSection(p, idOrSection));
  if (holder) return { page: holder, section: idOrSection };
  return all.length > 0 ? { page: all[0] } : undefined;
}

/** A page in the nav; `section` when the search found the page by one of its sections. */
export interface SettingsNavEntry {
  page: SettingsPage;
  section?: SettingsSection;
}

/** Pages whose label or group, or a section's title or keywords, contain `query` (any case); all for "". */
function searchSettingsPages(query: string): SettingsNavEntry[] {
  const needle = query.trim().toLowerCase();
  const has = (text: string) => text.toLowerCase().includes(needle);
  return settingsPages().flatMap((page) => {
    if (has(page.label) || has(page.group)) return [{ page }];
    const section = page.sections?.find((candidate) => has(candidate.title) || candidate.keywords?.some(has));
    return section ? [{ page, section }] : [];
  });
}

/** The nav groups in nav order, narrowed to `query` when one is given. */
export function settingsGroups(query = ""): { group: string; pages: SettingsNavEntry[] }[] {
  const groups: { group: string; pages: SettingsNavEntry[] }[] = [];
  for (const hit of searchSettingsPages(query)) {
    const last = groups.at(-1);
    if (last?.group === hit.page.group) last.pages.push(hit);
    else groups.push({ group: hit.page.group, pages: [hit] });
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
