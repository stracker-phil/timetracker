import { Settings as SettingsIcon } from "lucide-react";
import { runCommand, useBindingLabel } from "../commands";
import { useStore } from "../state/store";
import { SegmentedButtons } from "./settings/controls";

const TIME_VIEWS = ["day", "week", "month"] as const;

/** Day, Week and Month as one control; Reports and Projects are only in the sidebar. */
function ViewSwitch() {
  const view = useStore((s) => s.view);
  const setView = useStore((s) => s.setView);
  const current = TIME_VIEWS.find((v) => v === view) ?? null;
  return (
    <div className="view-switch">
      <SegmentedButtons label="View" options={TIME_VIEWS} value={current} onChange={setView} />
    </div>
  );
}

export function Toolbar() {
  const settingsKeys = useBindingLabel("settings.toggle");
  return (
    <header className="toolbar" data-tauri-drag-region>
      <span className="app-title" data-tauri-drag-region>TimeTracker</span>
      <ViewSwitch />
      <button className="icon-btn" aria-label="Settings" title={settingsKeys ? `Settings (${settingsKeys})` : "Settings"} onClick={() => runCommand("settings.toggle")}>
        <SettingsIcon size="1.077rem" />
      </button>
    </header>
  );
}
