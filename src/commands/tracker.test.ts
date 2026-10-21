import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { installCommands, runCommand } from "./index";
import { resetStore, useStore } from "../state/store";
import { trackerStatus } from "../test/data";

const mocked = vi.mocked(api, true);
const paused = trackerStatus({ state: { kind: "paused", until: null }, text: "Paused" });

describe("tracker commands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ tracker: trackerStatus() });
    installCommands();
  });

  describe("tracker.pause", () => {
    it("pauses until resumed while tracking", async () => {
      expect(runCommand("tracker.pause")).toBe(true);
      await vi.waitFor(() => expect(useStore.getState().tracker?.state.kind).toBe("paused"));
      expect(mocked.trackerPause).toHaveBeenCalledWith("resumed");
    });

    it("also runs while the tracker waits for the permission", () => {
      resetStore({ tracker: trackerStatus({ state: { kind: "needsPermission" } }) });
      expect(runCommand("tracker.pause")).toBe(true);
    });

    it.each([
      ["paused", paused],
      ["off", null],
    ])("does not run while the tracker is %s", (_, tracker) => {
      resetStore({ tracker });
      expect(runCommand("tracker.pause")).toBe(false);
      expect(mocked.trackerPause).not.toHaveBeenCalled();
    });
  });

  describe("tracker.resume", () => {
    it("resumes while paused", async () => {
      resetStore({ tracker: paused });
      expect(runCommand("tracker.resume")).toBe(true);
      await vi.waitFor(() => expect(useStore.getState().tracker?.state.kind).toBe("running"));
      expect(mocked.trackerResume).toHaveBeenCalled();
    });

    it.each([
      ["running", trackerStatus()],
      ["off", null],
    ])("does not run while the tracker is %s", (_, tracker) => {
      resetStore({ tracker });
      expect(runCommand("tracker.resume")).toBe(false);
      expect(mocked.trackerResume).not.toHaveBeenCalled();
    });
  });
});
