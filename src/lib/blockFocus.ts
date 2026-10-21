import type { ActivityBlock, DayView } from "../api";

/** Keyboard focus moves over the blocks of a `DayView`. */

interface Placed {
  block: ActivityBlock;
  lane: number;
}

function placed(view: DayView): Placed[] {
  return view.lanes
    .flatMap((lane, index) => lane.blocks.map((block) => ({ block, lane: index })))
    .sort((a, b) => a.block.start - b.block.start || a.lane - b.lane || a.block.col - b.block.col);
}

export function findBlock(view: DayView, id: string | null): Placed | null {
  if (id === null) return null;
  for (const [index, lane] of view.lanes.entries()) {
    const block = lane.blocks.find((b) => b.id === id);
    if (block) return { block, lane: index };
  }
  return null;
}

/** The next (`by` 1) or previous (-1) block in time order across lanes; from nothing, the first or last. */
export function stepBlock(view: DayView, id: string | null, by: 1 | -1): string | null {
  const order = placed(view);
  if (order.length === 0) return null;
  const at = order.findIndex((p) => p.block.id === id);
  if (at === -1) return (by === 1 ? order[0] : order[order.length - 1]).block.id;
  return order[Math.max(0, Math.min(order.length - 1, at + by))].block.id;
}

/** The block of the next lane with blocks to the right (`by` 1) or left (-1) closest in time to `id`'s. */
export function stepLane(view: DayView, id: string | null, by: 1 | -1): string | null {
  const from = findBlock(view, id);
  if (!from) return stepBlock(view, id, 1);
  const middle = (from.block.start + from.block.end) / 2;
  for (let lane = from.lane + by; lane >= 0 && lane < view.lanes.length; lane += by) {
    const blocks = view.lanes[lane].blocks;
    if (blocks.length === 0) continue;
    const distance = (b: ActivityBlock) => (b.start <= middle && middle < b.end ? 0 : Math.min(Math.abs(b.start - middle), Math.abs(b.end - middle)));
    return blocks.reduce((best, b) => (distance(b) < distance(best) ? b : best)).id;
  }
  return from.block.id;
}
