import { Settings as SettingsIcon } from "lucide-react";
import { runCommand, useBindingLabel } from "../commands";

/** The window's drag strip: the app title and Settings; views and dates live in the sidebar. */
export function Toolbar() {
  const settingsKeys = useBindingLabel("settings.toggle");
  return (
    <header className="toolbar" data-tauri-drag-region>
      <span className="app-title" data-tauri-drag-region>TimeTracker</span>
      <span className="spacer" data-tauri-drag-region />
      <button className="icon-btn" aria-label="Settings" title={settingsKeys ? `Settings (${settingsKeys})` : "Settings"} onClick={() => runCommand("settings.toggle")}>
        <SettingsIcon size="1.077rem" />
      </button>
    </header>
  );
}
