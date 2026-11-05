import { queryOptions, useQuery } from "@tanstack/react-query";
import { api, type Keybindings, type Settings } from "../api";
import { queryClient } from "./queryClient";

export const SETTINGS_KEY = ["settings"] as const;
export const KEYBINDINGS_KEY = ["keybindings"] as const;

const NO_BINDINGS: Keybindings = {};

// Both are read outside components (`currentSettings`, `currentKeybindings`), so kept while nothing shows them.
export const settingsQuery = queryOptions({ queryKey: SETTINGS_KEY, queryFn: api.getSettings, gcTime: Infinity, meta: { failure: "Could not load settings" } });

/** No overrides when they can't be read: every command keeps its default key. */
export const keybindingsQuery = queryOptions({ queryKey: KEYBINDINGS_KEY, queryFn: () => api.getKeybindings().catch(() => NO_BINDINGS), gcTime: Infinity });

/** `null` until loaded. */
export const useSettings = (): Settings | null => useQuery(settingsQuery).data ?? null;

export const useKeybindings = (): Keybindings => useQuery(keybindingsQuery).data ?? NO_BINDINGS;

/** The settings as loaded, for code outside components; null before the first load. */
export const currentSettings = (): Settings | null => queryClient.getQueryData(SETTINGS_KEY) ?? null;

export const currentKeybindings = (): Keybindings => queryClient.getQueryData(KEYBINDINGS_KEY) ?? NO_BINDINGS;

/** The settings as Rust saved or announced them. */
export const setSettingsData = (settings: Settings) => queryClient.setQueryData(SETTINGS_KEY, settings);

export const setKeybindingsData = (bindings: Keybindings) => queryClient.setQueryData(KEYBINDINGS_KEY, bindings);

/** Loads settings and shortcuts once per window, before the commands install; a failure is toasted and resolves. */
export async function loadSettings(): Promise<void> {
  await Promise.all([queryClient.fetchQuery(settingsQuery).catch(() => {}), queryClient.fetchQuery(keybindingsQuery)]);
}
