/** The core settings pages and the rows the search finds. Modules register their own pages from their install
 *  function. */
import { Activity, Database, EyeOff, Gauge, GanttChart, Info, Keyboard, Palette, Power } from "lucide-react";
import { registerSettingsPage, type SettingsRow } from "../../registry/settingsPages";
import { AboutPage, ABOUT_ROWS } from "./AboutPage";
import { showWelcomeAgain } from "./actions";
import { APPEARANCE_ROWS, AppearancePage } from "./AppearancePage";
import { CAPTURE_ROWS, CapturePage, TRACKER_ROWS, TrackerPage } from "./CapturePage";
import { Group, Row } from "./controls";
import { DATA_ROWS, DataPage } from "./DataPage";
import { LaunchAtLoginSwitch } from "./LaunchAtLoginSwitch";
import { ShortcutsPage, shortcutRows } from "./ShortcutsPage";
import { STATUS_ROWS, StatusPage } from "./StatusPage";
import { TIMELINE_ROWS, TimelinePage } from "./TimelinePage";

const GENERAL_ROWS: SettingsRow[] = [
  { id: "launch-at-login", title: "Launch at login", keywords: ["startup", "login", "autostart", "menu bar"] },
  { id: "welcome-sheet", title: "Welcome sheet", keywords: ["Show welcome again", "onboarding", "Accessibility"] },
];

function GeneralPage() {
  return (
    <Group>
      <LaunchAtLoginSwitch />
      <Row label="Welcome sheet" anchor="welcome-sheet" description="What TimeTracker records, the Accessibility permission and how it starts.">
        <button type="button" className="btn" onClick={showWelcomeAgain}>
          Show welcome again
        </button>
      </Row>
    </Group>
  );
}

export function registerCoreSettingsPages() {
  registerSettingsPage({ id: "general", group: "App", label: "General", icon: Power, order: 0, intro: "How TimeTracker starts.", Body: GeneralPage, rows: GENERAL_ROWS });
  registerSettingsPage({
    id: "appearance",
    group: "App",
    label: "Appearance",
    icon: Palette,
    order: 10,
    intro: "How TimeTracker looks. Changes apply right away.",
    Body: AppearancePage,
    keywords: ["theme", "look"],
    rows: APPEARANCE_ROWS,
  });
  registerSettingsPage({
    id: "shortcuts",
    group: "App",
    label: "Shortcuts",
    icon: Keyboard,
    order: 20,
    intro: "Click a shortcut and press the new keys. Backspace unbinds, Escape cancels.",
    Body: ShortcutsPage,
    keywords: ["keys", "keybindings", "hotkeys"],
    rows: shortcutRows,
  });
  registerSettingsPage({
    id: "capture",
    group: "Tracking",
    label: "Capture",
    icon: EyeOff,
    order: 10,
    intro: "What the tracker records. It never records keystrokes, screenshots, audio, your clipboard or page content, and nothing leaves this Mac.",
    Body: CapturePage,
    keywords: ["privacy"],
    rows: CAPTURE_ROWS,
  });
  registerSettingsPage({
    id: "tracker",
    group: "Tracking",
    label: "Tracker",
    icon: Activity,
    order: 20,
    intro: "When time counts as away, and when TimeTracker asks about it.",
    Body: TrackerPage,
    keywords: ["away", "idle"],
    rows: TRACKER_ROWS,
  });
  registerSettingsPage({
    id: "timeline",
    group: "Tracking",
    label: "Timeline",
    icon: GanttChart,
    order: 30,
    intro: "How the Day view shows your activity.",
    Body: TimelinePage,
    keywords: ["Day view", "lanes"],
    rows: TIMELINE_ROWS,
  });
  registerSettingsPage({
    id: "data",
    group: "System",
    label: "Data",
    icon: Database,
    order: 0,
    intro: "Where TimeTracker keeps your activity and recordings. Everything stays on this Mac.",
    Body: DataPage,
    keywords: ["privacy", "storage", "backup"],
    rows: DATA_ROWS,
  });
  registerSettingsPage({ id: "status", group: "System", label: "Status", icon: Gauge, order: 10, Body: StatusPage, keywords: ["diagnostics"], rows: STATUS_ROWS });
  registerSettingsPage({ id: "about", group: "System", label: "About", icon: Info, order: 20, Body: AboutPage, keywords: ["version"], rows: ABOUT_ROWS });
}
