import { useEffect, useRef, useState, type CSSProperties, type MouseEvent, type PointerEvent } from "react";
import type { DayView } from "../../api";
import { findBlock } from "../../lib/blockFocus";
import { isToday } from "../../lib/day";
import { formatClock } from "../../lib/format";
import { useStore } from "../../state/store";
import { useContextMenu } from "../ContextMenu";
import { blockMenu } from "./blockMenu";
import { HoverCard } from "./HoverCard";
import { Lane } from "./Lane";
import { LaneHeader } from "./LaneHeader";
import { Spans } from "./Spans";
import { RecordingsColumn } from "./RecordingsColumn";
import { HALF_HOURS_FROM_PX, TimeAxis } from "./TimeAxis";
import { selectionPress } from "./selectionPress";
import { useSnappedSpan } from "./useSnappedSpan";
import { useRuleDrag } from "./useRuleDrag";
import { useTimelineScroll } from "./useTimelineScroll";
import { useVisibleRange } from "./useVisibleRange";
import { ZoomControl } from "./ZoomControl";

const MINUTE_MS = 60_000;
const SHOW_CARD_MS = 300;
const HIDE_CARD_MS = 150;
const NOW_TICK_MS = 30_000;
/** Compact density draws the day at this share of the zoom's height. */
const COMPACT_SCALE = 0.6;

const blockIdAt = (target: EventTarget) => (target instanceof Element ? (target.closest<HTMLElement>("[data-block-id]")?.dataset.blockId ?? null) : null);
const gearAt = (target: EventTarget) => (target instanceof Element ? target.closest<HTMLElement>("[data-block-gear]") : null);

function useNow(live: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!live) return;
    const timer = setInterval(() => setNow(Date.now()), NOW_TICK_MS);
    return () => clearInterval(timer);
  }, [live]);
  return now;
}

/** The hovered block, shown after a delay; the card itself keeps it shown. */
function useHoveredBlock() {
  const [hovered, setHovered] = useState<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const after = (ms: number, next: string | null) => {
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setHovered(next), ms);
  };
  useEffect(() => () => clearTimeout(timer.current), []);
  const leave = () => after(HIDE_CARD_MS, null);
  const keep = () => clearTimeout(timer.current);
  return {
    hovered,
    over: (id: string | null) => {
      if (id === null) leave();
      else if (id !== hovered) after(SHOW_CARD_MS, id);
      else keep();
    },
    keep,
    leave,
  };
}

/** The day's scroll box: the time column and the lanes; the zoom floats over them. */
export function Timeline({ view }: { view: DayView }) {
  const zoom = useStore((s) => s.uiState.timelineZoom);
  const pxPerMinute = view.compact ? zoom * COMPACT_SCALE : zoom;
  const focusedBlock = useStore((s) => s.focusedBlock);
  const detailsBlock = useStore((s) => s.detailsBlock);
  const menuRequest = useStore((s) => s.blockMenuRequest);
  const scrollRef = useRef<HTMLDivElement>(null);
  const range = useVisibleRange(scrollRef);
  const hover = useHoveredBlock();
  const menu = useContextMenu();
  const live = isToday(view.day);
  const now = useNow(live);
  const [origin, end] = view.bounds;
  const y = (ms: number) => ((ms - origin) / MINUTE_MS) * pxPerMinute;
  const element = (id: string | null) => (id === null ? null : (scrollRef.current?.querySelector(`[data-block-id="${id}"]`) ?? null));

  useTimelineScroll(scrollRef, view, focusedBlock, y, live ? now : null);
  const columnRef = useRef<HTMLDivElement>(null);
  const press = selectionPress({ origin, pxPerMinute, scrollRef, columnRef });
  const ruleDrag = useRuleDrag(columnRef, scrollRef);
  const pressLanes = (e: PointerEvent<HTMLDivElement>) => {
    if (gearAt(e.target) !== null) return;
    const found = e.altKey && e.button === 0 ? findBlock(view, blockIdAt(e.target)) : null;
    if (found) ruleDrag.start(e, found.block);
    else press(e);
  };
  const selection = useSnappedSpan(useStore((s) => s.selection));
  const selectionStart = selection?.start;
  const selectionEnd = selection?.end;
  useEffect(() => {
    const { setSelectionSnap } = useStore.getState();
    setSelectionSnap(selectionStart === undefined || selectionEnd === undefined ? null : { start: selectionStart, end: selectionEnd });
  }, [selectionStart, selectionEnd]);

  const openMenu = (anchor: { x: number; y: number } | MouseEvent, id: string) => {
    const found = findBlock(view, id);
    if (found) menu.open(anchor, blockMenu(found.block, view.lanes[found.lane], useStore.getState().settings));
  };

  useEffect(() => {
    const el = element(focusedBlock);
    if (menuRequest === 0 || !el || focusedBlock === null) return;
    const rect = el.getBoundingClientRect();
    openMenu({ x: rect.left + 8, y: rect.top + 8 }, focusedBlock);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [menuRequest]);

  const cardId = hover.hovered ?? detailsBlock;
  const card = findBlock(view, cardId);

  return (
    <div className="timeline-frame">
      <div className={view.compact ? "timeline timeline--compact" : "timeline"} ref={scrollRef} onMouseLeave={hover.leave}>
        <LaneHeader lanes={view.lanes} trackedMs={view.totals.trackedMs} />
        <div className="timeline-body" style={{ height: y(end) }}>
          <Spans away={view.away} gaps={view.gaps} y={y} />
          <TimeAxis bounds={view.bounds} pxPerMinute={pxPerMinute} onPointerDown={press} />
          <RecordingsColumn ref={columnRef} recordings={view.recordings} suggestions={view.suggestions} y={y} pxPerMinute={pxPerMinute} selection={selection} />
          <div
            className={pxPerMinute >= HALF_HOURS_FROM_PX ? "lanes half-hours" : "lanes"}
            style={{ "--hour-px": `${60 * pxPerMinute}px` } as CSSProperties}
            onMouseOver={(e) => hover.over(blockIdAt(e.target))}
            onPointerDown={pressLanes}
            onClick={(e) => {
              const gear = gearAt(e.target);
              const id = blockIdAt(e.target);
              if (!gear || !id) return;
              const rect = gear.getBoundingClientRect();
              openMenu({ x: rect.left, y: rect.bottom }, id);
            }}
            onContextMenu={(e) => {
              const id = blockIdAt(e.target);
              if (id) openMenu(e, id);
            }}
          >
            {view.lanes.map((lane) => (
              <Lane key={lane.id} lane={lane} origin={origin} pxPerMinute={pxPerMinute} range={range} focusedBlock={focusedBlock} />
            ))}
            {selection && (
              <div className="selection" style={{ top: y(selection.start), height: y(selection.end) - y(selection.start) }} aria-label="Selection">
                <span className="selection-label">
                  {formatClock(selection.start)}–{formatClock(selection.end)}
                </span>
              </div>
            )}
            {live && now >= origin && now < end && <div className="now-line" style={{ top: y(now) }} aria-hidden="true" />}
          </div>
        </div>
      </div>
      <ZoomControl />
      {card && <HoverCard block={card.block} anchor={element(card.block.id)} onPointerEnter={hover.keep} onPointerLeave={hover.leave} />}
      {menu.element}
      {ruleDrag.strip}
    </div>
  );
}
