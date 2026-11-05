import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { installCommands, runCommand } from "./index";
import { resetStore } from "../state/store";
import { trackerStatus } from "../test/data";
import { seedQuery, TRACKER_KEY } from "../test/queries";

const mocked = vi.mocked(api, true);
const paused = trackerStatus({ state: { kind: "paused", until: null }, text: "Paused" });

describe("tracker commands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    seedQuery(TRACKER_KEY, trackerStatus());
    installCommands();
  });

  describe("tracker:pause:resumed", () => {
    it("asks the bus to pause until resumed while tracking", () => {
      expect(runCommand("tracker:pause:resumed")).toBe(true);
      expect(mocked.runCommand).toHaveBeenCalledWith("tracker:pause:resumed");
    });

    it("also runs while the tracker waits for the permission", () => {
      seedQuery(TRACKER_KEY, trackerStatus({ state: { kind: "needsPermission" } }));
      expect(runCommand("tracker:pause:resumed")).toBe(true);
    });

    it.each([
      ["paused", paused],
      ["off", null],
    ])("does not run while the tracker is %s", (_, tracker) => {
      seedQuery(TRACKER_KEY, tracker);
      expect(runCommand("tracker:pause:resumed")).toBe(false);
      expect(mocked.runCommand).not.toHaveBeenCalled();
    });
  });

  describe("tracker:resume", () => {
    it("asks the bus to resume while paused", () => {
      seedQuery(TRACKER_KEY, paused);
      expect(runCommand("tracker:resume")).toBe(true);
      expect(mocked.runCommand).toHaveBeenCalledWith("tracker:resume");
    });

    it.each([
      ["running", trackerStatus()],
      ["off", null],
    ])("does not run while the tracker is %s", (_, tracker) => {
      seedQuery(TRACKER_KEY, tracker);
      expect(runCommand("tracker:resume")).toBe(false);
      expect(mocked.runCommand).not.toHaveBeenCalled();
    });
  });
});
