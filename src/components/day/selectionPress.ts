import type { PointerEvent as ReactPointerEvent, RefObject } from "react";
import { useStore } from "../../state/store";
import { pointerDrag } from "./pointerDrag";

/** Near the scroll box's top or bottom edge (px) a drag scrolls it. */
const EDGE_PX = 24;
const SCROLL_STEP_PX = 8;

interface Options {
  /** The day's first instant (ms). */
  origin: number;
  pxPerMinute: number;
  scrollRef: RefObject<HTMLElement | null>;
  /** Releasing a drag over it opens the editor. */
  columnRef: RefObject<HTMLElement | null>;
}

const blockIdAt = (target: EventTarget) => (target instanceof Element ? (target.closest<HTMLElement>("[data-block-id]")?.dataset.blockId ?? null) : null);

function isOver(el: HTMLElement | null, e: PointerEvent): boolean {
  const rect = el?.getBoundingClientRect();
  return !!rect && e.clientX >= rect.left && e.clientX < rect.right;
}

function autoScroll(box: HTMLElement | null, clientY: number) {
  const rect = box?.getBoundingClientRect();
  if (!box || !rect) return;
  if (clientY < rect.top + EDGE_PX) box.scrollTop -= SCROLL_STEP_PX;
  else if (clientY > rect.bottom - EDGE_PX) box.scrollTop += SCROLL_STEP_PX;
}

/** A click on a block selects its span (⇧ adds it to the selection); a click on nothing clears it. */
function selectClicked(blockId: string | null, extend: boolean) {
  const s = useStore.getState();
  s.focusRecording(null);
  if (blockId === null) {
    s.setSelection(null);
    s.focusBlock(null);
    return;
  }
  if (!extend) s.setSelection(null);
  s.extendSelection(blockId);
  s.focusBlock(blockId);
}

/** Releasing a drag over the recordings column opens the editor for the selected span. */
function dropSelection(column: HTMLElement | null, e: PointerEvent) {
  const s = useStore.getState();
  const span = s.selectionSnap ?? s.selection;
  if (span && isOver(column, e)) s.openEditor({ mode: "create", start: span.start, end: span.end });
}

/** The press handler of the lanes and the axis: a drag selects a span, a click selects a block. */
export function selectionPress({ origin, pxPerMinute, scrollRef, columnRef }: Options) {
  return (e: ReactPointerEvent<HTMLElement>) => {
    if (e.button !== 0) return;
    const el = e.currentTarget;
    const msAt = (clientY: number) => origin + ((clientY - el.getBoundingClientRect().top) / pxPerMinute) * 60_000;
    const anchor = msAt(e.clientY);
    const blockId = blockIdAt(e.target);
    const extend = e.shiftKey;
    pointerDrag(e, {
      move: (_dy, ev) => {
        autoScroll(scrollRef.current, ev.clientY);
        const at = msAt(ev.clientY);
        useStore.getState().setSelection({ start: Math.min(anchor, at), end: Math.max(anchor, at), blockIds: [] });
      },
      end: (_dy, dragged, ev) => (dragged ? dropSelection(columnRef.current, ev) : selectClicked(blockId, extend)),
    });
  };
}
