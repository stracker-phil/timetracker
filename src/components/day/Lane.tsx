import { memo, type CSSProperties } from "react";
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
  /** The blocks whose spans are in the selection or the open editor's spans. */
  selectedBlocks: string[];
}

/** The most columns a cluster of the lane's overlapping blocks takes: the lane widens to fit them. */
export const laneCols = (lane: LaneData) => lane.blocks.reduce((most, b) => Math.max(most, b.cols), 1);

const LOOKS = { summary: "summary", app: "pinned" } as const;

/** One lane's blocks, absolutely placed from their times and columns. */
export const Lane = memo(function Lane({ lane, origin, pxPerMinute, range, focusedBlock, selectedBlocks }: Props) {
  const y = (ms: number) => ((ms - origin) / 60_000) * pxPerMinute;
  const look = LOOKS[lane.kind as keyof typeof LOOKS] ?? "memory";
  return (
    <div className={`lane lane--${lane.kind}`} data-lane-id={lane.id} style={{ "--lane-cols": laneCols(lane) } as CSSProperties}>
      {lane.blocks.map((block) => {
        const top = y(block.start);
        const height = Math.max(MIN_BLOCK_PX, y(block.end) - top);
        if (top + height < range.top || top > range.bottom) return null;
        return <ActivityBlock key={block.id} block={block} look={look} top={top} height={height} focused={block.id === focusedBlock} selected={selectedBlocks.includes(block.id)} />;
      })}
    </div>
  );
});
