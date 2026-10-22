import { useSyncExternalStore } from "react";
import { SYSTEM_ACCENT, type Scheme, type Settings } from "../api/types";

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

/** A family name the user typed, quoted for CSS and put in front of the system stack. */
function fontStack(family: string | null, fallback: string): string | null {
  if (!family) return null;
  return `"${family.replace(/["\\]/g, "\\$&")}", var(${fallback})`;
}

function setOrRemove(style: CSSStyleDeclaration, name: string, value: string | null) {
  if (value) style.setProperty(name, value);
  else style.removeProperty(name);
}

/** Applies appearance settings to <html> so CSS tokens pick them up. */
export function applyAppearance(settings: Settings, systemDark: boolean) {
  const root = document.documentElement;
  applyLook(resolveLook(settings, systemDark));
  root.dataset.accent = settings.accent;
  root.style.setProperty("--ui-font-size", `${settings.uiFontSize}px`);
  root.style.setProperty("--data-font-size", `${settings.dataFontSize}px`);
  setOrRemove(root.style, "--font-ui", fontStack(settings.fontUi, "--font-ui-system"));
  setOrRemove(root.style, "--font-mono", fontStack(settings.fontMono, "--font-mono-system"));
}

/** Applies the default look for the OS appearance before settings load, so the first paint has colors. */
export function applyInitialLook() {
  applyLook(resolveLook(DEFAULT_APPEARANCE, systemIsDark()));
  document.documentElement.dataset.accent = SYSTEM_ACCENT;
}
