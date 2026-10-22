// Re-exports the IPC types generated from Rust (`bun run types`); everything defined below is frontend-only.
import type { LogLevel, UiState } from "./types.gen";

export type * from "./types.gen";

/** Text size bounds and defaults in px; Rust's `Settings::sanitized` clamps to the same range. */
export const TEXT_SIZES = {
  ui: { min: 10, max: 24, default: 13 },
  data: { min: 9, max: 28, default: 12 },
} as const;

/** Bounds of the numeric settings. Rust's `Settings::sanitized` is the source of truth; keep these equal. */
export const SETTINGS_LIMITS = {
  idleMinutes: { min: 1, max: 120 },
  awayPromptMinutes: { min: 1, max: 240 },
  pollSeconds: { min: 1, max: 10 },
  zoom: { min: 0.5, max: 4 },
} as const;

/** Least to most detail, as the Logging page lists them. */
const LOG_LEVEL_RANK: Record<LogLevel, number> = { error: 0, warn: 1, info: 2, debug: 3, trace: 4 };
/** The `Record` makes the compiler check every Rust level is listed. */
export const LOG_LEVELS = (Object.keys(LOG_LEVEL_RANK) as LogLevel[]).sort((a, b) => LOG_LEVEL_RANK[a] - LOG_LEVEL_RANK[b]);

/** Accent hues, defined by every palette. Rust's `ACCENTS` is the source of truth; keep these equal. */
export const ACCENTS = ["orange", "gold", "red", "magenta", "purple", "indigo", "blue", "teal", "graphite"] as const;

/** How many shades a project color has; Rust's `projects::SHADES`. */
export const SHADES = 9;

/** Families bundled in `src/assets/fonts/`; the font settings fall back to them when empty. */
export const BUNDLED_FONTS = { ui: "Geist", mono: "JetBrains Mono" } as const;

/** Shortcut overrides by command id; `null` unbinds. Rust: the `Keybindings` alias (a map, so ts-rs can't export it). */
export type Keybindings = Record<string, string | null>;

/** Until Rust's state loads; `onboardingDone` is true so the welcome sheet never flashes before it. */
export const EMPTY_UI_STATE: UiState = { sidebarHidden: false, sidebarWidth: 200, lastDay: null, timelineZoom: 2.5, paneSizes: {}, dayMode: "timeline", onboardingDone: true };
