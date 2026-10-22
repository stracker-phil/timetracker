import { type PointerEvent as ReactPointerEvent, type RefObject } from "react";
import { useStore } from "../../state/store";
import { pointerDrag } from "./pointerDrag";
import { selectedSpan } from "./spanDrag";
import { tickPointer, type TickGrid } from "./tickPointer";

/** Near the scroll box's top or bottom edge (px) a drag scrolls it. */
const EDGE_PX = 24;
const SCROLL_STEP_PX = 8;

interface Options {
  grid: TickGrid;
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
 * The press handler of the lanes, the axis and the time column: a drag selects the span between the tick it was
 * pressed on and the pointer's tick (`selectedSpan`) and opens the editor on
 * release (an open editor takes it as another span); a click on a block toggles its span in the editor's spans
 * (the first opens the editor); a click on nothing clears the selection.
 */
export function useSelectionPress({ grid, scrollRef }: Options) {
  function selectClicked(blockId: string | null) {
    const s = useStore.getState();
    s.focusRecording(null);
    s.focusBlock(blockId);
    if (blockId === null) s.setSelection(null);
    else void s.toggleBlock(blockId);
  }

  return (e: ReactPointerEvent<HTMLElement>) => {
    if (e.button !== 0) return;
    if (grid.ticks.length < 2) return;
    const tickAt = tickPointer(grid, e.currentTarget);
    const origin = tickAt(e.clientY).index;
    const blockId = blockIdAt(e.target);
    pointerDrag(e, {
      move: (_dy, ev) => {
        autoScroll(scrollRef.current, ev.clientY);
        useStore.getState().setSelection({ ...selectedSpan(grid.ticks, origin, tickAt(ev.clientY)), blockIds: [] });
      },
      end: (_dy, dragged) => (dragged ? void useStore.getState().recordSelection() : selectClicked(blockId)),
    });
  };
}
