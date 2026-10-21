import type { PointerEvent as ReactPointerEvent } from "react";

/** Capture is a nicety: a pointer the browser no longer tracks (or a synthetic one) cannot be captured. */
function capture(run: () => void) {
  try {
    run();
  } catch {
    // The drag still follows events on the element.
  }
}

/** Below this distance (px) a press is a click, not a drag. */
const DRAG_MIN_PX = 3;

interface DragHandlers {
  /** `dy`: px moved since the press. */
  move: (dy: number, e: PointerEvent) => void;
  /** `dragged`: the pointer moved past a click's tolerance. */
  end: (dy: number, dragged: boolean, e: PointerEvent) => void;
}

/** Follows the pointer from a press until its release, with pointer capture so it never fights the scroll box. */
export function pointerDrag(e: ReactPointerEvent<HTMLElement>, { move, end }: DragHandlers) {
  const el = e.currentTarget;
  const [x0, y0] = [e.clientX, e.clientY];
  let dragged = false;
  capture(() => el.setPointerCapture(e.pointerId));
  const onMove = (ev: PointerEvent) => {
    const dy = ev.clientY - y0;
    if (!dragged && Math.abs(dy) < DRAG_MIN_PX && Math.abs(ev.clientX - x0) < DRAG_MIN_PX) return;
    dragged = true;
    move(dy, ev);
  };
  const onUp = (ev: PointerEvent) => {
    el.removeEventListener("pointermove", onMove);
    el.removeEventListener("pointerup", onUp);
    el.removeEventListener("pointercancel", onUp);
    capture(() => el.releasePointerCapture(ev.pointerId));
    end(ev.clientY - y0, dragged, ev);
  };
  el.addEventListener("pointermove", onMove);
  el.addEventListener("pointerup", onUp);
  el.addEventListener("pointercancel", onUp);
}
