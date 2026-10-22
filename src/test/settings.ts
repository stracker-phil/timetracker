import type { Settings } from "../api/types";

/** Equal to Rust's `Settings::default()`. */
export function settings(patch: Partial<Settings> = {}): Settings {
  return {
    lightScheme: "default",
    darkScheme: "dark",
    accent: "orange",
    textSize: "medium",
    calendarTitle: "short",
    dayTitle: "long",
    launchAtLogin: false,
    dataDir: null,
    tracker: { idleMinutes: 5, awayPromptEnabled: true, awayPromptMinutes: 10 },
    capture: { excludedApps: [], excludedDomains: [], captureUrls: true, captureDocuments: true },
    timeline: {
      summaryLane: true,
      summaryMinutes: 5,
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
