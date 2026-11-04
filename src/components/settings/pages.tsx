/** The core settings pages and the sections the search finds. Modules register their own pages from their
 *  install function. */
import { Activity, Database, EyeOff, GanttChart, Keyboard, Palette, Power } from "lucide-react";
import { registerSettingsPage } from "../../registry/settingsPages";
import { showWelcomeAgain } from "./actions";
import { AppearancePage } from "./AppearancePage";
import { CapturePage, TrackerPage } from "./CapturePage";
import { Group, Row } from "./controls";
import { DataPage } from "./DataPage";
import { LaunchAtLoginSwitch } from "./LaunchAtLoginSwitch";
import { ShortcutsPage } from "./ShortcutsPage";
import { TimelinePage } from "./TimelinePage";

function GeneralPage() {
  return (
    <>
      <Group section="startup">
        <LaunchAtLoginSwitch />
      </Group>
      <Group title="Welcome" section="welcome">
        <Row label="Welcome sheet" description="What TimeTracker records, the Accessibility permission and how it starts.">
          <button type="button" className="btn" onClick={showWelcomeAgain}>
            Show welcome again
          </button>
        </Row>
      </Group>
    </>
  );
}

export function registerCoreSettingsPages() {
  registerSettingsPage({
    id: "general",
    group: "App",
    label: "General",
    icon: Power,
    order: 0,
    intro: "How TimeTracker starts.",
    Body: GeneralPage,
    sections: [
      { id: "startup", title: "Launch at login", keywords: ["login", "startup", "autostart", "menu bar"] },
      { id: "welcome", title: "Welcome", keywords: ["Welcome sheet", "Show welcome again", "onboarding", "Accessibility"] },
    ],
  });
  registerSettingsPage({
    id: "appearance",
    group: "App",
    label: "Appearance",
    icon: Palette,
    order: 10,
    intro: "How TimeTracker looks. Changes apply right away.",
    Body: AppearancePage,
    sections: [
      { id: "look", title: "Look", keywords: ["Light mode scheme", "Dark mode scheme", "scheme", "theme", "dark mode", "Accent", "color", "colour"] },
      { id: "dates", title: "Dates", keywords: ["Calendar title", "Day title", "month", "weekday"] },
      { id: "text", title: "Text", keywords: ["Text size", "font", "zoom"] },
    ],
  });
  registerSettingsPage({
    id: "shortcuts",
    group: "App",
    label: "Shortcuts",
    icon: Keyboard,
    order: 20,
    intro: "Click a shortcut and press the new keys. Backspace unbinds, Escape cancels.",
    Body: ShortcutsPage,
    sections: [
      { id: "global-shortcut", title: "Global shortcut", keywords: ["Start timer (system-wide)", "hotkey", "system-wide"] },
      { id: "command-shortcuts", title: "In TimeTracker", keywords: ["keybindings", "keys", "commands"] },
    ],
  });
  registerSettingsPage({
    id: "capture",
    group: "Tracking",
    label: "Capture",
    icon: EyeOff,
    order: 10,
    intro: "What the tracker records. It never records keystrokes, screenshots, audio, your clipboard or page content, and nothing leaves this Mac.",
    Body: CapturePage,
    sections: [
      { id: "recorded", title: "Recorded", keywords: ["Capture page addresses in browsers", "URL", "Capture document paths", "documents", "files"] },
      { id: "excluded-apps", title: "Excluded apps", keywords: ["exclude", "ignore", "privacy", "Add app"] },
      { id: "excluded-websites", title: "Excluded websites", keywords: ["domain", "website", "exclude", "privacy"] },
      { id: "external-sources", title: "External sources", keywords: ["CLI", "command line", "planner"] },
    ],
  });
  registerSettingsPage({
    id: "tracker",
    group: "Tracking",
    label: "Tracker",
    icon: Activity,
    order: 20,
    intro: "When time counts as away, and when TimeTracker asks about it.",
    Body: TrackerPage,
    sections: [{ id: "away", title: "Away", keywords: ["Away after", "idle", "Ask what I did after being away", "away prompt", "For longer than"] }],
  });
  registerSettingsPage({
    id: "timeline",
    group: "Tracking",
    label: "Timeline",
    icon: GanttChart,
    order: 30,
    intro: "How the Day view shows your activity.",
    Body: TimelinePage,
    sections: [
      {
        id: "lanes",
        title: "Lanes",
        keywords: ["Summary lane", "Summary interval", "Show all activity", "Compact activity", "Group by app and website", "Join short gaps", "Hide activity shorter than", "Titles", "Time snapping", "snap", "grid"],
      },
      { id: "apps", title: "Apps", keywords: ["Hidden", "Own lane", "hide"] },
    ],
  });
  registerSettingsPage({
    id: "data",
    group: "System",
    label: "Data",
    icon: Database,
    order: 0,
    intro: "Where TimeTracker keeps your activity and recordings. Everything stays on this Mac.",
    Body: DataPage,
    sections: [
      { id: "folder", title: "Folder", keywords: ["Location", "Reveal in Finder", "Move", "database"] },
      { id: "contents", title: "Contents", keywords: ["Activities", "Recordings", "Projects", "First activity", "Size on disk"] },
      { id: "backup", title: "Backup", keywords: ["JSON backup", "Export backup", "export"] },
      { id: "mcp", title: "MCP server", keywords: ["Claude", "Expose activity to the MCP server", "AI"] },
    ],
  });
}
