import { describe, expect, it, vi } from "vitest";
import { tickNear, tickNearPx, tickPointer, type TickGrid } from "./tickPointer";

/** Five ticks 120 px apart: 0, 120, 240, 360 and 480 px. */
const grid: TickGrid = { ticks: [0, 60, 120, 180, 240], y: (ms) => ms * 2 };

describe("tickNearPx", () => {
  it.each([
    [1, 0],
    [59, 0],
    [61, 1],
    [250, 2],
    [419, 3],
    [421, 4],
  ])("picks the tick nearest %i px", (px, index) => {
    expect(tickNearPx(grid, px)).toBe(index);
  });

  it.each([
    [-300, 0],
    [9_999, 4],
  ])("clamps %i px to the first or last tick", (px, index) => {
    expect(tickNearPx(grid, px)).toBe(index);
  });
});

describe("tickNear", () => {
  it("returns the tick of a time on the grid", () => {
    expect(tickNear(grid, 180)).toBe(3);
  });

  it("returns the tick drawn nearest a time between ticks", () => {
    expect(tickNear(grid, 70)).toBe(1);
    expect(tickNear(grid, 100)).toBe(2);
  });

  it("clamps a time outside the grid to its first or last tick", () => {
    expect(tickNear(grid, -40)).toBe(0);
    expect(tickNear(grid, 1_000)).toBe(4);
  });
});

describe("tickPointer", () => {
  function timelineBody(top: number) {
    const body = document.createElement("div");
    body.className = "timeline-body";
    vi.spyOn(body, "getBoundingClientRect").mockReturnValue({ top } as DOMRect);
    const child = document.createElement("div");
    body.append(child);
    return { body, child };
  }

  it("reads the pointer from the top of the timeline body around the pressed element", () => {
    const { child } = timelineBody(100);
    const at = tickPointer(grid, child);
    expect(at(100 + 130).index).toBe(1);
    expect(at(100 + 250).index).toBe(2);
  });

  it("follows the body when it is scrolled up", () => {
    const { child } = timelineBody(-200);
    expect(tickPointer(grid, child)(40).index).toBe(2);
  });

  it("tells whether the pointer is above or below its tick's line", () => {
    const { child } = timelineBody(0);
    const at = tickPointer(grid, child);
    expect(at(110)).toEqual({ index: 1, above: true });
    expect(at(130)).toEqual({ index: 1, above: false });
  });

  it("reads from the element itself when it is not in a timeline body", () => {
    const el = document.createElement("div");
    vi.spyOn(el, "getBoundingClientRect").mockReturnValue({ top: 50 } as DOMRect);
    expect(tickPointer(grid, el)(50 + 245).index).toBe(2);
  });
});
