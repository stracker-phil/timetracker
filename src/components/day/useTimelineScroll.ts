import { useEffect, useLayoutEffect, useRef, type RefObject } from "react";
import type { DayView } from "../../api";
import { findBlock } from "../../lib/blockFocus";

const HOUR_MS = 3_600_000;
/** A day opens scrolled to this long before its first activity. */
const LEAD_MS = HOUR_MS / 2;
/** A day without activity opens at this hour (today: at the current time). */
const EMPTY_DAY_HOUR = 8;

/** Where a day opens: its first activity, else now (today), else the morning. */
function openingTime(view: DayView, now: number | null): number {
  return view.firstActivity ?? now ?? view.bounds[0] + EMPTY_DAY_HOUR * HOUR_MS;
}

/**
 * Scrolls a newly shown day to where it opens, and the focused block into view. A reload of the same day
 * keeps the position.
 */
export function useTimelineScroll(ref: RefObject<HTMLElement | null>, view: DayView, focusedBlock: string | null, y: (ms: number) => number, now: number | null) {
  const scrolledDay = useRef<string | null>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el || scrolledDay.current === view.day) return;
    scrolledDay.current = view.day;
    el.scrollTop = Math.max(0, y(openingTime(view, now) - LEAD_MS));
  });

  useEffect(() => {
    const el = ref.current;
    const found = findBlock(view, focusedBlock);
    if (!el || !found) return;
    const top = y(found.block.start);
    const bottom = Math.max(top, y(found.block.end));
    const head = el.firstElementChild?.clientHeight ?? 0;
    if (top < el.scrollTop + head) el.scrollTop = top - head;
    else if (bottom > el.scrollTop + el.clientHeight) el.scrollTop = Math.min(top - head, bottom - el.clientHeight);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [focusedBlock]);
}
