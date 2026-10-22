import { useSyncExternalStore } from "react";
import { DEFAULT_TEXT_SIZE, SYSTEM_ACCENT, TEXT_SIZES, type Scheme, type Settings } from "../api/types";

// One query for the whole app; matchMedia is missing in some test environments.
const darkQuery = typeof window.matchMedia === "function" ? window.matchMedia("(prefers-color-scheme: dark)") : null;

function subscribeToSystemTheme(onChange: () => void) {
  darkQuery?.addEventListener("change", onChange);
  return () => darkQuery?.removeEventListener("change", onChange);
}

export const systemIsDark = () => darkQuery?.matches ?? false;

export function useSystemDark(): boolean {
  return useSyncExternalStore(subscribeToSystemTheme, systemIsDark);
}

type AppearanceSettings = Pick<Settings, "lightScheme" | "darkScheme">;

const DEFAULT_APPEARANCE: AppearanceSettings = { lightScheme: "default", darkScheme: "dark" };

export type Tone = "light" | "dark";

/** The palette of each surface: the content and the sidebar. */
export interface Look {
  content: Tone;
  sidebar: Tone;
}

/** Default: a dark sidebar beside light content; Light and Dark: one tone for both. */
export function schemeLook(scheme: Scheme): Look {
  if (scheme === "default") return { content: "light", sidebar: "dark" };
  return { content: scheme, sidebar: scheme };
}

/** The look of the scheme picked for the current macOS mode. */
export function resolveLook({ lightScheme, darkScheme }: AppearanceSettings, systemDark: boolean): Look {
  return schemeLook(systemDark ? darkScheme : lightScheme);
}

function applyLook(look: Look) {
  const { dataset } = document.documentElement;
  dataset.content = look.content;
  dataset.sidebar = look.sidebar;
}

/** Applies appearance settings to <html> so CSS tokens pick them up. */
export function applyAppearance(settings: Settings, systemDark: boolean) {
  const root = document.documentElement;
  applyLook(resolveLook(settings, systemDark));
  root.dataset.accent = settings.accent;
  const sizeOf = (size: string) => TEXT_SIZES.find((t) => t.size === size);
  const text = sizeOf(settings.textSize) ?? sizeOf(DEFAULT_TEXT_SIZE)!;
  root.style.setProperty("--ui-font-size", `${text.ui}px`);
  root.style.setProperty("--data-font-size", `${text.data}px`);
}

/** Applies the default look for the OS appearance before settings load, so the first paint has colors. */
export function applyInitialLook() {
  applyLook(resolveLook(DEFAULT_APPEARANCE, systemIsDark()));
  document.documentElement.dataset.accent = SYSTEM_ACCENT;
}
