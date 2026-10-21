import { useCallback, useEffect, useRef, useState } from "react";
import { useStore } from "../../state/store";

const TICK_MS = 30_000;
const STATUS_MIN_MS = 10_000;

function useDocumentVisible(): boolean {
  const [visible, setVisible] = useState(() => document.visibilityState === "visible");
  useEffect(() => {
    const update = () => setVisible(document.visibilityState === "visible");
    document.addEventListener("visibilitychange", update);
    return () => document.removeEventListener("visibilitychange", update);
  }, []);
  return visible;
}

/**
 * A counter that grows every 30 s while `live`, and on tracker status changes at most every 10 s, so today's
 * timeline follows the open span. The tracker itself sends no `data_changed`. While the window is hidden it
 * stands still, and grows once when the window shows again.
 */
export function useLiveReload(live: boolean): number {
  const tracker = useStore((s) => s.tracker);
  const visible = useDocumentVisible();
  const [tick, setTick] = useState(0);
  const last = useRef(Date.now());
  const seenTracker = useRef(tracker);
  const stale = useRef(false);

  const bump = useCallback(() => {
    last.current = Date.now();
    setTick((t) => t + 1);
  }, []);

  useEffect(() => {
    if (!live) return;
    if (!visible) {
      stale.current = true;
      return;
    }
    if (stale.current) {
      stale.current = false;
      bump();
    }
    const timer = setInterval(bump, TICK_MS);
    return () => clearInterval(timer);
  }, [live, visible, bump]);

  useEffect(() => {
    if (seenTracker.current === tracker) return;
    seenTracker.current = tracker;
    if (!live || !visible) return;
    const wait = last.current + STATUS_MIN_MS - Date.now();
    if (wait <= 0) return bump();
    const timer = setTimeout(bump, wait);
    return () => clearTimeout(timer);
  }, [tracker, live, visible, bump]);

  return tick;
}
