import { useSyncExternalStore } from "react";
import type { Scheme, Settings } from "../api";

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

type AppearanceSettings = Pick<Settings, "mode" | "scheme">;

const DEFAULT_APPEARANCE: AppearanceSettings = { mode: "system", scheme: "default" };

export type Tone = "light" | "dark";

/** The palette of each surface: the content and the sidebar. */
export interface Look {
  content: Tone;
  sidebar: Tone;
}

/** In dark mode the whole window is dark; in light mode the scheme decides (Default: a dark sidebar). */
export function resolveLook({ mode, scheme }: AppearanceSettings, systemDark: boolean): Look {
  const dark = mode === "system" ? systemDark : mode === "dark";
  if (dark || scheme === "dark") return { content: "dark", sidebar: "dark" };
  return { content: "light", sidebar: scheme === "light" ? "light" : "dark" };
}

/** The look `scheme` gives in light mode, which is what sets the schemes apart. */
export const lightModeLook = (scheme: Scheme): Look => resolveLook({ mode: "light", scheme }, false);

function applyLook(look: Look) {
  const { dataset } = document.documentElement;
  dataset.content = look.content;
  dataset.sidebar = look.sidebar;
}

/** A family name the user typed, quoted for CSS and put in front of the bundled stack. */
function fontStack(family: string | null, bundled: string): string | null {
  if (!family) return null;
  return `"${family.replace(/["\\]/g, "\\$&")}", var(${bundled})`;
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
  setOrRemove(root.style, "--font-ui", fontStack(settings.fontUi, "--font-ui-bundled"));
  setOrRemove(root.style, "--font-mono", fontStack(settings.fontMono, "--font-mono-bundled"));
}

/** Applies the default look for the OS appearance before settings load, so the first paint has colors. */
export function applyInitialLook() {
  applyLook(resolveLook(DEFAULT_APPEARANCE, systemIsDark()));
}
