import { describe, expect, it } from "vitest";
import { place } from "./placement";

const size = { width: 100, height: 50 };
const view = { width: 800, height: 600 };

describe("place", () => {
  describe("at a point", () => {
    it("opens with its corner on the point when it fits", () => {
      expect(place({ point: { x: 10, y: 20 } }, size, view)).toEqual({ left: 10, top: 20 });
    });

    it("opens to the left of the point when it would pass the right edge", () => {
      expect(place({ point: { x: 750, y: 20 } }, size, view)).toEqual({ left: 650, top: 20 });
    });

    it("opens above the point when it would pass the bottom edge", () => {
      expect(place({ point: { x: 10, y: 580 } }, size, view)).toEqual({ left: 10, top: 530 });
    });
  });

  describe("under a box", () => {
    const box = { left: 100, top: 100, right: 200, bottom: 130 };

    it("opens under the box, left-aligned, with a gap", () => {
      expect(place({ below: box }, size, view)).toEqual({ left: 100, top: 134 });
    });

    it("right-aligns with the box when it would pass the right edge", () => {
      const edge = { left: 750, top: 100, right: 790, bottom: 130 };
      expect(place({ below: edge }, size, view)).toEqual({ left: 690, top: 134 });
    });

    it("opens above the box when it would pass the bottom edge", () => {
      const low = { left: 100, top: 560, right: 200, bottom: 590 };
      expect(place({ below: low }, size, view)).toEqual({ left: 100, top: 506 });
    });
  });

  describe("beside a row", () => {
    const row = { left: 100, top: 100, right: 300, bottom: 120 };

    it("opens at the row's right edge, its first row lined up with the parent row", () => {
      expect(place({ beside: row }, size, view)).toEqual({ left: 300, top: 96 });
    });

    it("opens on the row's left side when it would pass the right edge", () => {
      const edge = { left: 650, top: 100, right: 790, bottom: 120 };
      expect(place({ beside: edge }, size, view)).toEqual({ left: 550, top: 96 });
    });
  });

  describe("inside the window", () => {
    it.each([
      ["left", { point: { x: -10, y: 20 } }, { left: 4, top: 20 }],
      ["top", { point: { x: 10, y: -10 } }, { left: 10, top: 4 }],
      ["top of a submenu row near the top", { beside: { left: 100, top: 2, right: 300, bottom: 22 } }, { left: 300, top: 4 }],
    ])("keeps 4px clear of the %s edge", (_edge, placement, expected) => {
      expect(place(placement, size, view)).toEqual(expected);
    });

    it("shifts a flipped menu back inside when the other side doesn't fit either", () => {
      const tall = { width: 100, height: 400 };
      expect(place({ point: { x: 10, y: 300 } }, tall, view)).toEqual({ left: 10, top: 4 });
    });

    it("pins a menu larger than the window to the top left margin", () => {
      expect(place({ point: { x: 300, y: 300 } }, { width: 900, height: 700 }, view)).toEqual({ left: 4, top: 4 });
    });
  });
});
