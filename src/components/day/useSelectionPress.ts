import { useRef, type PointerEvent as ReactPointerEvent, type RefObject } from "react";
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
}

interface Modifiers {
  /** ⇧: grow the selection over the block. */
  extend: boolean;
  /** ⌘: add or remove the block; the editor opens once ⌘ is released. */
  toggle: boolean;
  /** ⌥: a span for the open editor's recording. */
  add: boolean;
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
 * release; a click on a block selects its span and opens the editor (⇧ extends first, ⌘ gathers blocks and opens it
 * when ⌘ is released); a click on nothing clears the selection.
 */
export function useSelectionPress({ origin, pxPerMinute, scrollRef }: Options) {
  // Set while ⌘-clicks gather blocks, so one release of ⌘ opens the editor once.
  const waitingForMeta = useRef(false);

  function recordOnMetaUp() {
    if (waitingForMeta.current) return;
    waitingForMeta.current = true;
    const up = (e: KeyboardEvent) => {
      if (e.key !== "Meta") return;
      window.removeEventListener("keyup", up);
      waitingForMeta.current = false;
      void useStore.getState().recordSelection();
    };
    window.addEventListener("keyup", up);
  }

  function selectClicked(blockId: string | null, { extend, toggle, add }: Modifiers) {
    const s = useStore.getState();
    s.focusRecording(null);
    if (blockId === null) {
      s.setSelection(null);
      s.focusBlock(null);
      return;
    }
    s.focusBlock(blockId);
    if (toggle) {
      s.toggleSelectionBlock(blockId);
      recordOnMetaUp();
      return;
    }
    if (!extend) s.setSelection(null);
    s.extendSelection(blockId);
    void s.recordSelection(add);
  }

  return (e: ReactPointerEvent<HTMLElement>) => {
    if (e.button !== 0) return;
    const el = e.currentTarget;
    const msAt = (clientY: number) => origin + ((clientY - el.getBoundingClientRect().top) / pxPerMinute) * 60_000;
    const anchor = msAt(e.clientY);
    const blockId = blockIdAt(e.target);
    const modifiers = { extend: e.shiftKey, toggle: e.metaKey, add: e.altKey && useStore.getState().editor !== null };
    pointerDrag(e, {
      move: (_dy, ev) => {
        autoScroll(scrollRef.current, ev.clientY);
        const at = msAt(ev.clientY);
        useStore.getState().setSelection({ start: Math.min(anchor, at), end: Math.max(anchor, at), blockIds: [], moving: at < anchor ? "start" : "end" });
      },
      end: (_dy, dragged) => (dragged ? void useStore.getState().recordSelection(modifiers.add) : selectClicked(blockId, modifiers)),
    });
  };
}
