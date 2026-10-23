import { render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import type { Lane as LaneData } from "../../api";
import { resetStore } from "../../state/store";
import { appLane, at, block, lane, sourceLane, summaryLane } from "../../test/dayFixtures";
import { Lane, laneCols } from "./Lane";

const ORIGIN = at("09:00");

/** A block of `minutes` from 09:00; at one pixel per minute that many pixels tall. */
const tall = (minutes: number, patch: Parameters<typeof block>[0] = {}) =>
  block({ id: "a1", app: "Safari", label: "Docs", start: ORIGIN, end: ORIGIN + minutes * 60_000, activeMs: minutes * 60_000, ...patch });

function renderLane(l: LaneData) {
  return render(<Lane lane={l} origin={ORIGIN} pxPerMinute={1} range={{ top: 0, bottom: 600 }} focusedBlock={null} selectedBlocks={[]} />);
}

describe("Lane", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  describe("laneCols", () => {
    it("is at least 1, also without blocks", () => {
      expect(laneCols(lane())).toBe(1);
    });

    it("is the most columns of the lane's blocks", () => {
      const l = lane({ blocks: [tall(30, { id: "a1", cols: 1 }), tall(30, { id: "a2", col: 1, cols: 3 }), tall(30, { id: "a3", cols: 2 })] });
      expect(laneCols(l)).toBe(3);
    });
  });

  it("sets --lane-cols to the columns its overlapping blocks need", () => {
    const l = lane({ blocks: [tall(30, { id: "a1", cols: 2 }), tall(30, { id: "a2", col: 1, cols: 2, label: "Todo" })] });
    renderLane(l);
    expect(screen.getByLabelText("Safari: Docs").closest("[data-lane-id]")).toHaveStyle("--lane-cols: 2");
  });

  describe("memory blocks", () => {
    it("show the title and the duration when two lines fit", () => {
      renderLane(lane({ blocks: [tall(30)] }));
      const el = screen.getByLabelText("Safari: Docs");
      expect(within(el).getByText("Docs")).toBeInTheDocument();
      expect(within(el).getByText("30m")).toBeInTheDocument();
    });

    it("show the title but no duration when only one line fits", () => {
      renderLane(sourceLane("planner", [tall(15)]));
      const el = screen.getByLabelText("Safari: Docs");
      expect(within(el).getByText("Docs")).toBeInTheDocument();
      expect(within(el).queryByText("15m")).not.toBeInTheDocument();
      expect(within(el).getByRole("button", { name: "Actions for Safari" })).toBeInTheDocument();
    });

    it("show the duration from 28 pixels up", () => {
      renderLane(lane({ blocks: [tall(28)] }));
      expect(within(screen.getByLabelText("Safari: Docs")).getByText("28m")).toBeInTheDocument();
    });

    it("show no text when too short", () => {
      renderLane(lane({ blocks: [tall(8)] }));
      const el = screen.getByLabelText("Safari: Docs");
      expect(within(el).queryByText("Docs")).not.toBeInTheDocument();
      expect(within(el).queryByText("8m")).not.toBeInTheDocument();
    });
  });

  describe("pinned app lane blocks", () => {
    it("show the duration but neither the label nor the gear", () => {
      renderLane(appLane("Safari", "com.apple.Safari", [tall(30)]));
      const el = screen.getByLabelText("Safari: Docs");
      expect(within(el).getByText("30m")).toBeInTheDocument();
      expect(within(el).queryByText("Docs")).not.toBeInTheDocument();
      expect(within(el).queryByRole("button")).not.toBeInTheDocument();
    });

    it("show the duration even when only one line tall", () => {
      renderLane(appLane("Safari", "com.apple.Safari", [tall(15)]));
      expect(within(screen.getByLabelText("Safari: Docs")).getByText("15m")).toBeInTheDocument();
    });

    it("show no duration when too short for text", () => {
      renderLane(appLane("Safari", "com.apple.Safari", [tall(8)]));
      expect(within(screen.getByLabelText("Safari: Docs")).queryByText("8m")).not.toBeInTheDocument();
    });
  });

  describe("summary blocks", () => {
    it("keep the label and the gear", () => {
      renderLane(summaryLane([tall(60)]));
      const el = screen.getByLabelText("Safari: Docs");
      expect(within(el).getByText("Docs")).toBeInTheDocument();
      expect(within(el).getByText("1h 00m")).toBeInTheDocument();
      expect(within(el).getByRole("button", { name: "Actions for Safari" })).toBeInTheDocument();
    });
  });
});
