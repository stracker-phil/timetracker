import type { MouseEvent } from "react";
import type { PauseUntil, TrackerState } from "../api";
import { useStore } from "../state/store";
import { useContextMenu } from "./ContextMenu";

const PAUSES: { label: string; until: PauseUntil }[] = [
  { label: "For 15 minutes", until: { minutes: 15 } },
  { label: "For 1 hour", until: { minutes: 60 } },
  { label: "Until tomorrow", until: "tomorrow" },
  { label: "Until resumed", until: "resumed" },
];

const TONES: Partial<Record<TrackerState["kind"], string>> = { running: "running", needsPermission: "attention" };

/** The tracker's status at the left of the status bar, with the same text as the tray. */
export function TrackerStatus() {
  const tracker = useStore((s) => s.tracker);
  const pauseTracker = useStore((s) => s.pauseTracker);
  const resumeTracker = useStore((s) => s.resumeTracker);
  const showPermissionDialog = useStore((s) => s.setPermissionDialogHidden);
  const menu = useContextMenu();
  const kind = tracker?.state.kind;
  const text = tracker?.text ?? "Tracking off";

  const openPauseMenu = (e: MouseEvent<HTMLButtonElement>) => {
    const { left, top } = e.currentTarget.getBoundingClientRect();
    menu.open(
      { x: left, y: top },
      PAUSES.map(({ label, until }) => ({ label, action: () => void pauseTracker(until) })),
    );
  };

  return (
    <div className="tracker-status">
      <span className={`tracker-dot ${(kind && TONES[kind]) ?? ""}`} aria-hidden="true" />
      <span className="grow-text" role="status" title={text}>
        {text}
      </span>
      {kind === "needsPermission" && (
        <button type="button" className="link-btn" onClick={() => showPermissionDialog(false)}>
          Allow…
        </button>
      )}
      {kind === "paused" && (
        <button type="button" className="link-btn" onClick={() => void resumeTracker()}>
          Resume
        </button>
      )}
      {(kind === "running" || kind === "away") && (
        <button type="button" className="link-btn" aria-haspopup="menu" onClick={openPauseMenu}>
          Pause
        </button>
      )}
      {menu.element}
    </div>
  );
}
