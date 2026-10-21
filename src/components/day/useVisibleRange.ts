import { useEffect, useState, type RefObject } from "react";

export interface VisibleRange {
  top: number;
  bottom: number;
}

/**
 * The band of the scroll box's content worth rendering, in px: the visible part and one screen above and
 * below it. It moves in half-screen steps, so scrolling re-renders the timeline every half screen, not every frame.
 */
export function useVisibleRange(ref: RefObject<HTMLElement | null>): VisibleRange {
  const [range, setRange] = useState<VisibleRange>({ top: 0, bottom: 0 });
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    let frame = 0;
    const update = () => {
      const screen = el.clientHeight;
      const step = Math.max(1, Math.floor(screen / 2));
      const snapped = Math.floor(el.scrollTop / step) * step;
      const top = snapped - screen;
      const bottom = snapped + step + 2 * screen;
      setRange((current) => (current.top === top && current.bottom === bottom ? current : { top, bottom }));
    };
    const onChange = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(update);
    };
    update();
    el.addEventListener("scroll", onChange, { passive: true });
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(onChange);
    observer?.observe(el);
    return () => {
      cancelAnimationFrame(frame);
      el.removeEventListener("scroll", onChange);
      observer?.disconnect();
    };
  }, [ref]);
  return range;
}
