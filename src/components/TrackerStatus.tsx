import type { MouseEvent } from "react";
import type { PauseUntil, TrackerState } from "../api";
import { useStore } from "../state/store";
import { useContextMenu, type MenuAnchor } from "./ContextMenu";

const PAUSES: { label: string; until: PauseUntil }[] = [
  { label: "For 15 minutes", until: { minutes: 15 } },
  { label: "For 1 hour", until: { minutes: 60 } },
  { label: "Until tomorrow", until: "tomorrow" },
  { label: "Until resumed", until: "resumed" },
];

const TONES: Partial<Record<TrackerState["kind"], string>> = { running: "running", needsPermission: "attention" };

/** The pause durations as a context menu; render `element`. */
export function usePauseMenu() {
  const menu = useContextMenu();
  const pauseTracker = useStore((s) => s.pauseTracker);
  const open = (at: MenuAnchor) => menu.open(at, PAUSES.map(({ label, until }) => ({ label, action: () => void pauseTracker(until) })));
  return { open, element: menu.element };
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
  const pauseMenu = usePauseMenu();

  const openPauseMenu = (e: MouseEvent<HTMLButtonElement>) => {
    const { left, top } = e.currentTarget.getBoundingClientRect();
    pauseMenu.open({ x: left, y: top });
  };

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
        <button type="button" className="link-btn" aria-haspopup="menu" onClick={openPauseMenu}>
          Pause
        </button>
      )}
      {pauseMenu.element}
    </div>
  );
}
