// Re-exports the IPC types generated from Rust (`bun run types`); everything defined below is frontend-only.
import type { TextSize, UiState } from "./types.gen";

export type * from "./types.gen";

/** The text size steps, smallest first, with their pixel sizes: `ui` for the chrome (it also scales rem), `data` for
 *  times, durations and titles. Rust's `TextSize::nearest_to_px` migrates old pixel sizes to these steps. */
export const TEXT_SIZES: readonly { size: TextSize; ui: number; data: number }[] = [
  { size: "small", ui: 12, data: 11 },
  { size: "medium", ui: 13, data: 12 },
  { size: "large", ui: 15, data: 14 },
];

/** Rust's `TextSize::default()`. */
export const DEFAULT_TEXT_SIZE: TextSize = "medium";

/** Bounds of the numeric settings. Rust's `Settings::sanitized` is the source of truth; keep these equal. */
export const SETTINGS_LIMITS = {
  idleMinutes: { min: 1, max: 120 },
  awayPromptMinutes: { min: 1, max: 240 },
  zoom: { min: 0.5, max: 4 },
} as const;

/** Accent hues, defined by every palette. Rust's `ACCENTS` is the source of truth; keep these equal. */
export const ACCENTS = ["orange", "gold", "red", "magenta", "purple", "indigo", "blue", "teal", "graphite"] as const;

/** The app accent that follows the macOS accent color (Rust's `SYSTEM_ACCENT`); projects never take it. */
export const SYSTEM_ACCENT = "system";

/** How many shades a project color has; Rust's `projects::SHADES`. */
export const SHADES = 9;

/** Shortcut overrides by command id; `null` unbinds. Rust: the `Keybindings` alias (a map, so ts-rs can't export it). */
export type Keybindings = Record<string, string | null>;

/** Until Rust's state loads; `onboardingDone` is true so the welcome sheet never flashes before it. */
export const EMPTY_UI_STATE: UiState = { sidebarHidden: false, sidebarWidth: 200, timelineZoom: 2.5, paneSizes: {}, onboardingDone: true };
