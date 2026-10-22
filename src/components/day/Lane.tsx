import { memo } from "react";
import type { Lane as LaneData } from "../../api";
import { ActivityBlock } from "./ActivityBlock";
import type { VisibleRange } from "./useVisibleRange";

const MIN_BLOCK_PX = 2;

interface Props {
  lane: LaneData;
  /** The day's first instant (ms). */
  origin: number;
  pxPerMinute: number;
  /** The band to render (px); blocks outside it are left out. */
  range: VisibleRange;
  focusedBlock: string | null;
}

/** One lane's blocks, absolutely placed from their times and columns. */
export const Lane = memo(function Lane({ lane, origin, pxPerMinute, range, focusedBlock }: Props) {
  const y = (ms: number) => ((ms - origin) / 60_000) * pxPerMinute;
  const look = lane.kind === "summary" ? "summary" : "memory";
  return (
    <div className={`lane lane--${lane.kind}`} data-lane-id={lane.id}>
      {lane.blocks.map((block) => {
        const top = y(block.start);
        const height = Math.max(MIN_BLOCK_PX, y(block.end) - top);
        if (top + height < range.top || top > range.bottom) return null;
        return <ActivityBlock key={block.id} block={block} look={look} top={top} height={height} focused={block.id === focusedBlock} />;
      })}
    </div>
  );
});
