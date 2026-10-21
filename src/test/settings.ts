import type { Settings } from "../api/types";

/** Equal to Rust's `Settings::default()`. */
export function settings(patch: Partial<Settings> = {}): Settings {
  return {
    theme: "system",
    lightScheme: "ayu-light",
    darkScheme: "ayu-mirage",
    accent: "orange",
    uiFontSize: 13,
    dataFontSize: 12,
    fontUi: null,
    fontMono: null,
    logLevel: "info",
    launchAtLogin: false,
    dataDir: null,
    tracker: { idleMinutes: 5, awayPromptEnabled: true, awayPromptMinutes: 10, pollSeconds: 2 },
    capture: { excludedApps: [], excludedDomains: [], captureUrls: true, captureDocuments: true },
    timeline: {
      summaryLane: true,
      summaryMinutes: 15,
      showAllActivity: true,
      compact: false,
      groupByApp: true,
      continuity: "short",
      noise: "low",
      titles: "document",
      snapMinutes: 5,
      zoom: 1,
    },
    apps: {},
    reports: { rounding: "none", defaultGroup: "project" },
    timerShortcut: "Mod+Shift+T",
    mcp: { exposeActivity: false },
    ...patch,
  };
}
