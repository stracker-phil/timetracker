import { useEffect, useRef, useState, type CSSProperties, type MouseEvent, type PointerEvent, type ReactNode } from "react";
import type { DayView } from "../../api";
import { findBlock } from "../../lib/blockFocus";
import { isToday } from "../../lib/day";
import { formatClock } from "../../lib/format";
import { useStore, type Store } from "../../state/store";
import { useContextMenu } from "../ContextMenu";
import { blockMenu } from "./blockMenu";
import { drawnParts, selectionBand } from "./drawnParts";
import { HoverCard } from "./HoverCard";
import { Lane } from "./Lane";
import { LaneHeader } from "./LaneHeader";
import { RecordingsColumn } from "./RecordingsColumn";
import { HALF_HOURS_FROM_PX, TimeAxis } from "./TimeAxis";
import { useSelectionPress } from "./useSelectionPress";
import { useRuleDrag } from "./useRuleDrag";
import { useNow } from "./useNow";
import { useTimelineScroll } from "./useTimelineScroll";
import { useVisibleRange } from "./useVisibleRange";
import { ZoomControl } from "./ZoomControl";

const MINUTE_MS = 60_000;
const SHOW_CARD_MS = 300;
const HIDE_CARD_MS = 150;
const NOW_TICK_MS = 30_000;
/** The time column draws 5-minute lines only while they are at least 6px apart. */
const FIVE_MINUTES_FROM_PX = 1.2;
/** Compact density draws the day at this share of the zoom's height. */
const COMPACT_SCALE = 0.6;

const NO_BLOCKS: string[] = [];

/** The blocks marked selected: those clicked into the open editor's spans, else the selection's. */
const selectHighlightedBlocks = (s: Store) => (s.editor ? s.draftBlockIds : (s.selection?.blockIds ?? NO_BLOCKS));

const blockIdAt = (target: EventTarget) => (target instanceof Element ? (target.closest<HTMLElement>("[data-block-id]")?.dataset.blockId ?? null) : null);
const gearAt = (target: EventTarget) => (target instanceof Element ? target.closest<HTMLElement>("[data-block-gear]") : null);

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

/** The day's scroll box: the time column and the lanes; the zoom floats over them. `empty` shows over the lanes of a day without anything. */
export function Timeline({ view, empty }: { view: DayView; empty?: ReactNode }) {
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
  const now = useNow(live, NOW_TICK_MS);
  const [origin, end] = view.bounds;
  const y = (ms: number) => ((ms - origin) / MINUTE_MS) * pxPerMinute;
  const element = (id: string | null) => (id === null ? null : (scrollRef.current?.querySelector(`[data-block-id="${id}"]`) ?? null));

  const openings = useStore((s) => s.openings);
  useTimelineScroll(scrollRef, view, focusedBlock, y, live ? now : null, openings);
  const columnRef = useRef<HTMLDivElement>(null);
  const grid = { ticks: view.ticks, y };
  const press = useSelectionPress({ grid, scrollRef });
  const ruleDrag = useRuleDrag(columnRef, scrollRef);
  // ⌥ on a block drags it onto a project for a rule; while the editor is open it is a plain press.
  const pressLanes = (e: PointerEvent<HTMLDivElement>) => {
    if (gearAt(e.target) !== null) return;
    const ruling = e.altKey && e.button === 0 && useStore.getState().editor === null;
    const found = ruling ? findBlock(view, blockIdAt(e.target)) : null;
    if (found) ruleDrag.start(e, found.block);
    else press(e);
  };
  const selection = useStore((s) => s.selectionSnap);
  const draft = useStore((s) => s.draft);
  const selectedBlocks = useStore(selectHighlightedBlocks);
  const draftBands = drawnParts(draft?.spans ?? [], view.bounds);
  const ownBand = selectionBand(selection, draftBands);

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
        <div
          className={pxPerMinute >= FIVE_MINUTES_FROM_PX ? "timeline-body five-minutes" : "timeline-body"}
          style={{ height: y(end), "--hour-px": `${60 * pxPerMinute}px` } as CSSProperties}
        >
          <TimeAxis bounds={view.bounds} pxPerMinute={pxPerMinute} onPointerDown={press} />
          <RecordingsColumn ref={columnRef} recordings={view.recordings} suggestions={view.suggestions} grid={grid} bounds={view.bounds} selection={selection} onPress={press} />
          <div
            className={pxPerMinute >= HALF_HOURS_FROM_PX ? "lanes half-hours" : "lanes"}
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
              <Lane key={lane.id} lane={lane} origin={origin} pxPerMinute={pxPerMinute} range={range} focusedBlock={focusedBlock} selectedBlocks={selectedBlocks} />
            ))}
            {draftBands.map((band) => (
              <div key={band.start} className="selection" style={{ top: y(band.start), height: y(band.end) - y(band.start) }} aria-hidden="true" />
            ))}
            {ownBand && (
              <div
                className="selection"
                style={{ top: y(ownBand.start), height: y(ownBand.end) - y(ownBand.start) }}
                aria-label={`Selection ${formatClock(ownBand.start)}–${formatClock(ownBand.end)}`}
              />
            )}
            {live && now >= origin && now < end && <div className="now-line" style={{ top: y(now) }} aria-hidden="true" />}
            {empty && <div className="lanes-empty">{empty}</div>}
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
