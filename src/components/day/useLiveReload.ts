import { useCallback, useEffect, useRef, useState } from "react";
import type { QueryKey } from "@tanstack/react-query";
import { invalidate } from "../../queries/queryClient";
import { useTracker } from "../../queries/status";

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
 * Refetches the shown queries under `keys` every 30 s while `live`, and on tracker status changes at most every
 * 10 s, so today's timeline and totals follow the open span. The tracker itself sends no `data_changed`. While
 * the window is hidden it stands still, and refetches once when the window shows again.
 */
export function useLiveReload(live: boolean, keys: QueryKey[]): void {
  const tracker = useTracker();
  const visible = useDocumentVisible();
  const last = useRef(Date.now());
  const seenTracker = useRef(tracker);
  const stale = useRef(false);
  const keysRef = useRef(keys);
  keysRef.current = keys;

  const bump = useCallback(() => {
    last.current = Date.now();
    invalidate(keysRef.current);
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
}
