import { useEffect, useLayoutEffect, useRef, type RefObject } from "react";
import type { DayView } from "../../api";
import { findBlock } from "../../lib/blockFocus";

const HOUR_MS = 3_600_000;
/** A past day opens scrolled to this long before its first activity. */
const LEAD_MS = HOUR_MS / 2;
/** A past day without activity opens at this hour. */
const EMPTY_DAY_HOUR = 8;
/** Today opens with now this far down the visible part, so the current activity shows. */
const NOW_AT = 0.8;

function openingTop(view: DayView, now: number | null, y: (ms: number) => number, height: number): number {
  if (now !== null) return y(now) - height * NOW_AT;
  return y((view.firstActivity ?? view.bounds[0] + EMPTY_DAY_HOUR * HOUR_MS) - LEAD_MS);
}

/** Scrolls a newly shown day to where it opens, again on each window opening, and the focused block into view. */
export function useTimelineScroll(ref: RefObject<HTMLElement | null>, view: DayView, focusedBlock: string | null, y: (ms: number) => number, now: number | null, openings: number) {
  const scrolled = useRef<{ day: string; openings: number } | null>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el || (scrolled.current?.day === view.day && scrolled.current.openings === openings)) return;
    scrolled.current = { day: view.day, openings };
    el.scrollTop = Math.max(0, openingTop(view, now, y, el.clientHeight));
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
