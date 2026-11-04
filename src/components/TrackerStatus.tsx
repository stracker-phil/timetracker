import { useState } from "react";
import type { PauseUntil, TrackerState } from "../api";
import { useStore } from "../state/store";
import { useMenuOpenOn, type MenuEntry } from "./menu";

const PAUSES: { label: string; until: PauseUntil }[] = [
  { label: "For 15 minutes", until: { minutes: 15 } },
  { label: "For 1 hour", until: { minutes: 60 } },
  { label: "Until tomorrow", until: "tomorrow" },
  { label: "Until resumed", until: "resumed" },
];

const TONES: Partial<Record<TrackerState["kind"], string>> = { running: "running", needsPermission: "attention" };

/** The pause durations, as a menu's rows. */
export function pauseMenu(): MenuEntry[] {
  const { pauseTracker } = useStore.getState();
  return PAUSES.map(({ label, until }) => ({ text: label, run: () => void pauseTracker(until) }));
}

/** The tracker's dot and the status text Rust computed (the tray shows the same). */
export function TrackerLine() {
  const tracker = useStore((s) => s.tracker);
  const kind = tracker?.state.kind;
  const text = tracker?.text ?? "Tracking off";
  return (
    <>
      <span className={`tracker-dot ${(kind && TONES[kind]) ?? ""}`} aria-hidden="true" />
      <span className="grow-text" role="status" title={text}>
        {text}
      </span>
    </>
  );
}

/** The tracker's status in the status bar while the sidebar is hidden. */
export function TrackerStatus() {
  const kind = useStore((s) => s.tracker?.state.kind);
  const resumeTracker = useStore((s) => s.resumeTracker);
  const showPermissionDialog = useStore((s) => s.setPermissionDialogHidden);
  const [pauseButton, setPauseButton] = useState<HTMLButtonElement | null>(null);
  const pauseOpen = useMenuOpenOn(pauseButton);

  return (
    <div className="tracker-status">
      <TrackerLine />
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
        <button
          ref={setPauseButton}
          type="button"
          className="link-btn"
          aria-haspopup="menu"
          aria-expanded={pauseOpen}
          onClick={(e) => useStore.getState().openMenu(pauseMenu(), e.currentTarget, "Pause tracking")}
        >
          Pause
        </button>
      )}
    </div>
  );
}
