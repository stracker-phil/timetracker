import { Pause, Play, Settings } from "lucide-react";
import { runCommand, useBindingLabel } from "../commands";
import { useStore } from "../state/store";
import { TrackerLine, usePauseMenu } from "./TrackerStatus";

const withKeys = (label: string, keys: string) => (keys ? `${label} (${keys})` : label);

/** Play or pause; a right-click on Pause offers the durations. */
function PlayPause() {
  const kind = useStore((s) => s.tracker?.state.kind);
  const { pauseTracker, resumeTracker, setPermissionDialogHidden } = useStore.getState();
  const pauseMenu = usePauseMenu();

  if (kind === "needsPermission") {
    return (
      <button type="button" className="link-btn" onClick={() => setPermissionDialogHidden(false)}>
        Allow…
      </button>
    );
  }
  if (kind === "paused") {
    return (
      <button type="button" className="icon-btn sidebar-play" aria-label="Resume tracking" title="Resume tracking" onClick={() => void resumeTracker()}>
        <Play size="0.923rem" />
      </button>
    );
  }
  if (kind !== "running" && kind !== "away") return null;
  return (
    <>
      <button
        type="button"
        className="icon-btn sidebar-play"
        aria-label="Pause tracking"
        title="Pause tracking; right-click for how long"
        onClick={() => void pauseTracker("resumed")}
        onContextMenu={pauseMenu.open}
      >
        <Pause size="0.923rem" />
      </button>
      {pauseMenu.element}
    </>
  );
}

/** The bottom of the sidebar: the tracker's status with play / pause, and the Settings gear. */
export function SidebarStatus() {
  const settingsKeys = useBindingLabel("settings.toggle");
  return (
    <div className="sidebar-foot">
      <div className="sidebar-status">
        <TrackerLine />
        <PlayPause />
      </div>
      <button type="button" className="icon-btn" aria-label="Settings" title={withKeys("Settings", settingsKeys)} onClick={() => runCommand("settings.toggle")}>
        <Settings size="1.077rem" />
      </button>
    </div>
  );
}
