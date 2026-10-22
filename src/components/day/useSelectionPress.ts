import { type PointerEvent as ReactPointerEvent, type RefObject } from "react";
import { useStore } from "../../state/store";
import { pointerDrag } from "./pointerDrag";
import { msOfPx } from "./spanDrag";

/** Near the scroll box's top or bottom edge (px) a drag scrolls it. */
const EDGE_PX = 24;
const SCROLL_STEP_PX = 8;

interface Options {
  /** The day's first instant (ms). */
  origin: number;
  pxPerMinute: number;
  scrollRef: RefObject<HTMLElement | null>;
}

const blockIdAt = (target: EventTarget) => (target instanceof Element ? (target.closest<HTMLElement>("[data-block-id]")?.dataset.blockId ?? null) : null);

function autoScroll(box: HTMLElement | null, clientY: number) {
  const rect = box?.getBoundingClientRect();
  if (!box || !rect) return;
  if (clientY < rect.top + EDGE_PX) box.scrollTop -= SCROLL_STEP_PX;
  else if (clientY > rect.bottom - EDGE_PX) box.scrollTop += SCROLL_STEP_PX;
}

/**
 * The press handler of the lanes, the axis and the time column: a drag selects a span and opens the editor on
 * release (an open editor takes it as another span); a click on a block toggles its span in the editor's spans
 * (the first opens the editor); a click on nothing clears the selection.
 */
export function useSelectionPress({ origin, pxPerMinute, scrollRef }: Options) {
  function selectClicked(blockId: string | null) {
    const s = useStore.getState();
    s.focusRecording(null);
    s.focusBlock(blockId);
    if (blockId === null) s.setSelection(null);
    else void s.toggleBlock(blockId);
  }

  return (e: ReactPointerEvent<HTMLElement>) => {
    if (e.button !== 0) return;
    const el = e.currentTarget;
    const msAt = (clientY: number) => origin + msOfPx(clientY - el.getBoundingClientRect().top, pxPerMinute);
    const anchor = msAt(e.clientY);
    const blockId = blockIdAt(e.target);
    pointerDrag(e, {
      move: (_dy, ev) => {
        autoScroll(scrollRef.current, ev.clientY);
        const at = msAt(ev.clientY);
        useStore.getState().setSelection({ start: Math.min(anchor, at), end: Math.max(anchor, at), blockIds: [], moving: at < anchor ? "start" : "end" });
      },
      end: (_dy, dragged) => (dragged ? void useStore.getState().recordSelection() : selectClicked(blockId)),
    });
  };
}
