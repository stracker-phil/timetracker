import { useEffect, useRef, useState } from "react";
import { useStore } from "../../state/store";

const TICK_MS = 30_000;
const STATUS_MIN_MS = 10_000;

/**
 * A counter that grows every 30 s while `live`, and on tracker status changes at most every 10 s, so today's
 * timeline follows the open span. The tracker itself sends no `data_changed`.
 */
export function useLiveReload(live: boolean): number {
  const tracker = useStore((s) => s.tracker);
  const [tick, setTick] = useState(0);
  const last = useRef(Date.now());
  const seenTracker = useRef(tracker);

  useEffect(() => {
    if (!live) return;
    const timer = setInterval(() => {
      last.current = Date.now();
      setTick((t) => t + 1);
    }, TICK_MS);
    return () => clearInterval(timer);
  }, [live]);

  useEffect(() => {
    if (!live || seenTracker.current === tracker) return;
    seenTracker.current = tracker;
    const bump = () => {
      last.current = Date.now();
      setTick((t) => t + 1);
    };
    const wait = last.current + STATUS_MIN_MS - Date.now();
    if (wait <= 0) return bump();
    const timer = setTimeout(bump, wait);
    return () => clearTimeout(timer);
  }, [tracker, live]);

  return tick;
}
