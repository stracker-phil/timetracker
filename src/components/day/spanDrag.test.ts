import { describe, expect, it } from "vitest";
import { sameSpan, selectedSpan, spanMoved, spanWithEdge } from "./spanDrag";
import type { TickGrid } from "./tickPointer";

const ticks = [0, 60, 120, 180, 240];
const grid: TickGrid = { ticks, y: (ms) => ms * 2 };

describe("selectedSpan", () => {
  it.each([
    ["below", 1, 3, { start: 60, end: 180 }],
    ["above", 3, 1, { start: 60, end: 180 }],
  ])("runs from the origin tick to the pointer's tick with the pointer %s the origin", (_name, origin, index, span) => {
    expect(selectedSpan(ticks, origin, { index, above: false })).toEqual(span);
  });

  it("runs one tick toward the pointer on the origin tick", () => {
    expect(selectedSpan(ticks, 2, { index: 2, above: true })).toEqual({ start: 60, end: 120 });
    expect(selectedSpan(ticks, 2, { index: 2, above: false })).toEqual({ start: 120, end: 180 });
  });

  it("runs down from the first tick whatever side the pointer is on", () => {
    expect(selectedSpan(ticks, 0, { index: 0, above: true })).toEqual({ start: 0, end: 60 });
  });

  it("runs up from the last tick whatever side the pointer is on", () => {
    expect(selectedSpan(ticks, 4, { index: 4, above: false })).toEqual({ start: 180, end: 240 });
  });
});

describe("spanWithEdge", () => {
  it("puts the end on the tick and keeps the start", () => {
    expect(spanWithEdge(ticks, { start: 60, end: 120 }, "end", 3)).toEqual({ start: 60, end: 180 });
  });

  it("puts the start on the tick and keeps the end", () => {
    expect(spanWithEdge(ticks, { start: 60, end: 120 }, "start", 0)).toEqual({ start: 0, end: 120 });
  });

  it.each([0, 1])("stops the end one tick after the start when dragged to tick %i", (index) => {
    expect(spanWithEdge(ticks, { start: 60, end: 180 }, "end", index)).toEqual({ start: 60, end: 120 });
  });

  it.each([3, 4])("stops the start one tick before the end when dragged to tick %i", (index) => {
    expect(spanWithEdge(ticks, { start: 60, end: 180 }, "start", index)).toEqual({ start: 120, end: 180 });
  });

  it("keeps the time of the other edge when it is off the grid", () => {
    expect(spanWithEdge(ticks, { start: 70, end: 125 }, "end", 4)).toEqual({ start: 70, end: 240 });
    expect(spanWithEdge(ticks, { start: 70, end: 125 }, "start", 0)).toEqual({ start: 0, end: 125 });
  });

  it("keeps the dragged edge a tick from the other edge when that is off the grid", () => {
    expect(spanWithEdge(ticks, { start: 70, end: 125 }, "end", 1)).toEqual({ start: 70, end: 120 });
    expect(spanWithEdge(ticks, { start: 70, end: 125 }, "start", 3)).toEqual({ start: 120, end: 125 });
  });

  it("leaves the span as it is when the grid has no room for the edge", () => {
    const span = { start: 240, end: 250 };
    expect(spanWithEdge(ticks, span, "end", 0)).toEqual(span);
  });
});

describe("spanMoved", () => {
  it.each([
    [1, { start: 120, end: 180 }],
    [-1, { start: 0, end: 60 }],
    [2, { start: 180, end: 240 }],
  ])("moves both edges by %i ticks", (by, moved) => {
    expect(spanMoved(grid, { start: 60, end: 120 }, by)).toEqual(moved);
  });

  it("stops at the ends of the grid with its length kept", () => {
    expect(spanMoved(grid, { start: 60, end: 120 }, 99)).toEqual({ start: 180, end: 240 });
    expect(spanMoved(grid, { start: 60, end: 120 }, -99)).toEqual({ start: 0, end: 60 });
  });

  it("returns the span itself when it does not move", () => {
    const span = { start: 60, end: 120 };
    expect(spanMoved(grid, span, 0)).toBe(span);
    expect(spanMoved(grid, { start: 180, end: 240 }, 3)).toEqual({ start: 180, end: 240 });
  });

  it("starts from the ticks drawn nearest edges that are off the grid", () => {
    expect(spanMoved(grid, { start: 70, end: 125 }, 1)).toEqual({ start: 120, end: 180 });
  });
});

describe("sameSpan", () => {
  it("compares both edges", () => {
    expect(sameSpan({ start: 1, end: 2 }, { start: 1, end: 2 })).toBe(true);
    expect(sameSpan({ start: 1, end: 2 }, { start: 1, end: 3 })).toBe(false);
    expect(sameSpan({ start: 1, end: 2 }, { start: 0, end: 2 })).toBe(false);
  });
});
