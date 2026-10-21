import { useEffect, useRef, useState, type CSSProperties, type MouseEvent } from "react";
import type { ActivityBlock, DayView, Lane as LaneData } from "../../api";
import { copyText } from "../../lib/clipboard";
import { findBlock } from "../../lib/blockFocus";
import { isToday } from "../../lib/day";
import { useStore } from "../../state/store";
import { useContextMenu, type MenuEntry } from "../ContextMenu";
import { HoverCard } from "./HoverCard";
import { Lane } from "./Lane";
import { LaneHeader } from "./LaneHeader";
import { Spans } from "./Spans";
import { HALF_HOURS_FROM_PX, TimeAxis } from "./TimeAxis";
import { useTimelineScroll } from "./useTimelineScroll";
import { useVisibleRange } from "./useVisibleRange";

const MINUTE_MS = 60_000;
const SHOW_CARD_MS = 300;
const HIDE_CARD_MS = 150;
const NOW_TICK_MS = 30_000;

const blockIdAt = (target: EventTarget) => (target instanceof Element ? (target.closest<HTMLElement>("[data-block-id]")?.dataset.blockId ?? null) : null);

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

function blockMenu(block: ActivityBlock, lane: LaneData): MenuEntry[] {
  const { setAppSetting, toast } = useStore.getState();
  const bundleId = block.bundleId;
  const entries: MenuEntry[] = [];
  if (bundleId && block.source === "tracker") {
    entries.push(
      lane.kind === "app"
        ? { label: "Back to activity lane", action: () => void setAppSetting(bundleId, { ownLane: false }) }
        : { label: `Own lane for ${block.app}`, action: () => void setAppSetting(bundleId, { ownLane: true }) },
      {
        label: `Hide ${block.app}`,
        action: () =>
          void setAppSetting(bundleId, { hidden: true }).then(
            (hidden) => hidden && toast("info", `${block.app} is hidden from the timeline`, { label: "Undo", run: () => void setAppSetting(bundleId, { hidden: false }) }),
          ),
      },
      "separator",
    );
  }
  entries.push({ label: "Copy title", action: () => void copyText(block.title, "the title") });
  const { url } = block;
  if (url) entries.push({ label: "Copy URL", action: () => void copyText(url, "the URL") });
  return entries;
}

/** The day's scroll box. */
export function Timeline({ view }: { view: DayView }) {
  const pxPerMinute = useStore((s) => s.uiState.timelineZoom);
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

  const openMenu = (anchor: { x: number; y: number } | MouseEvent, id: string) => {
    const found = findBlock(view, id);
    if (found) menu.open(anchor, blockMenu(found.block, view.lanes[found.lane]));
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
    <div className="timeline" ref={scrollRef} onMouseLeave={hover.leave}>
      <LaneHeader lanes={view.lanes} />
      <div className="timeline-body" style={{ height: y(end) }}>
        <div className="recordings-col" aria-label="Recordings" />
        <TimeAxis bounds={view.bounds} pxPerMinute={pxPerMinute} />
        <div
          className={pxPerMinute >= HALF_HOURS_FROM_PX ? "lanes half-hours" : "lanes"}
          style={{ "--hour-px": `${60 * pxPerMinute}px` } as CSSProperties}
          onMouseOver={(e) => hover.over(blockIdAt(e.target))}
          onClick={(e) => useStore.getState().focusBlock(blockIdAt(e.target))}
          onContextMenu={(e) => {
            const id = blockIdAt(e.target);
            if (id) openMenu(e, id);
          }}
        >
          <Spans away={view.away} gaps={view.gaps} y={y} />
          {view.lanes.map((lane) => (
            <Lane key={lane.id} lane={lane} origin={origin} pxPerMinute={pxPerMinute} range={range} focusedBlock={focusedBlock} />
          ))}
          {live && now >= origin && now < end && <div className="now-line" style={{ top: y(now) }} aria-hidden="true" />}
        </div>
      </div>
      {card && <HoverCard block={card.block} anchor={element(card.block.id)} onPointerEnter={hover.keep} onPointerLeave={hover.leave} />}
      {menu.element}
    </div>
  );
}
