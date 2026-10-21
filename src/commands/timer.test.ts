import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { installCommands, runCommand } from "./index";
import { resetStore } from "../state/store";
import { recording } from "../test/recordingFixtures";
import { runningStatus } from "../test/timerFixtures";

const mocked = vi.mocked(api, true);

describe("timer commands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    installCommands();
  });

  describe("timer.start", () => {
    it("opens the timer picker", () => {
      expect(runCommand("timer.start")).toBe(true);
      expect(mocked.timerPickerOpen).toHaveBeenCalled();
    });
  });

  describe("timer.stop", () => {
    it("does not run without a running timer", () => {
      resetStore({ timer: { running: null } });
      expect(runCommand("timer.stop")).toBe(false);
      expect(mocked.timerStop).not.toHaveBeenCalled();
    });

    it("stops the running timer", () => {
      resetStore({ timer: runningStatus() });
      mocked.timerStop.mockResolvedValueOnce(recording());
      expect(runCommand("timer.stop")).toBe(true);
      expect(mocked.timerStop).toHaveBeenCalled();
    });
  });

  describe("timer.toggle", () => {
    it("opens the picker while no timer runs", () => {
      expect(runCommand("timer.toggle")).toBe(true);
      expect(mocked.timerPickerOpen).toHaveBeenCalled();
      expect(mocked.timerStop).not.toHaveBeenCalled();
    });

    it("stops the timer while one runs", () => {
      resetStore({ timer: runningStatus() });
      expect(runCommand("timer.toggle")).toBe(true);
      expect(mocked.timerStop).toHaveBeenCalled();
      expect(mocked.timerPickerOpen).not.toHaveBeenCalled();
    });
  });
});
