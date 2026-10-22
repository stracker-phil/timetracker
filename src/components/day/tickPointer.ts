/**
 * The shown day's snap grid as Rust sends it (`DayView.ticks`, ascending) with the timeline's own mapping from a
 * time to its px from the top of the timeline body. Every drag on the timeline picks its times from `ticks`.
 */
export interface TickGrid {
  ticks: number[];
  y: (ms: number) => number;
}

/** Where a pointer is on the grid: the tick drawn nearest it, and whether it is above that tick's line. */
export interface TickAt {
  index: number;
  above: boolean;
}

/** The index of the tick drawn nearest `px`. */
export function tickNearPx({ ticks, y }: TickGrid, px: number): number {
  let lo = 0;
  let hi = ticks.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (y(ticks[mid]) < px) lo = mid + 1;
    else hi = mid;
  }
  return lo > 0 && px - y(ticks[lo - 1]) <= y(ticks[lo]) - px ? lo - 1 : lo;
}

/** The index of the tick drawn nearest a time the day already holds, such as an edge off the grid. */
export const tickNear = (grid: TickGrid, ms: number) => tickNearPx(grid, grid.y(ms));

/**
 * The pointer's place on the grid for a press on `el` inside the timeline body: the event wrapper every timeline
 * drag reads the pointer through, so the pointer's own position never reaches a time.
 */
export function tickPointer(grid: TickGrid, el: Element): (clientY: number) => TickAt {
  const body = el.closest(".timeline-body") ?? el;
  return (clientY) => {
    const px = clientY - body.getBoundingClientRect().top;
    const index = tickNearPx(grid, px);
    return { index, above: px < grid.y(grid.ticks[index]) };
  };
}
