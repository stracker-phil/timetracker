import { render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { resetStore } from "../../state/store";
import { appLane, at, block, lane, sourceLane, summaryLane } from "../../test/dayFixtures";
import { LaneHeader } from "./LaneHeader";

const MINUTE_MS = 60_000;

describe("LaneHeader", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  it("shows the label of a summary, activity and source lane", () => {
    render(<LaneHeader lanes={[summaryLane([]), lane(), sourceLane("planner", [])]} trackedMs={0} />);
    const heads = screen.getAllByRole("columnheader");
    expect(heads.map((h) => h.textContent)).toEqual(["Summary", "Activity0m", "planner"]);
  });

  it("shows the tracked time in the head of the activity lane only", () => {
    render(<LaneHeader lanes={[lane(), sourceLane("planner", [])]} trackedMs={90 * MINUTE_MS} />);
    expect(within(screen.getByRole("columnheader", { name: "Activity" })).getByText("1h 30m")).toBeInTheDocument();
    expect(within(screen.getByRole("columnheader", { name: "planner" })).queryByText("1h 30m")).not.toBeInTheDocument();
  });

  it("shows an app lane's head as its icon, named by the app", () => {
    render(<LaneHeader lanes={[appLane("Safari", "com.apple.Safari", [block({ start: at("09:00") })])]} trackedMs={0} />);
    const head = screen.getByRole("columnheader", { name: "Safari" });
    expect(head).toHaveAttribute("title", "Safari");
    expect(within(head).queryByText("Safari")).not.toBeInTheDocument();
  });

  it("sets --lane-cols on the head to the columns of the lane", () => {
    const wide = lane({ blocks: [block({ id: "a1", cols: 2 }), block({ id: "a2", col: 1, cols: 2 })] });
    render(<LaneHeader lanes={[wide]} trackedMs={0} />);
    expect(screen.getByRole("columnheader")).toHaveStyle("--lane-cols: 2");
  });
});
