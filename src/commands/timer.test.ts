import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { installCommands, runCommand } from "./index";
import { resetStore } from "../state/store";
import { seedQuery, TIMER_KEY } from "../test/queries";
import { runningStatus } from "../test/timerFixtures";

const mocked = vi.mocked(api, true);

describe("timer commands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    installCommands();
  });

  describe("timer:start", () => {
    it("asks the bus to open the timer picker", () => {
      expect(runCommand("timer:start")).toBe(true);
      expect(mocked.runCommand).toHaveBeenCalledWith("timer:start");
    });
  });

  describe("timer:stop", () => {
    it("does not run without a running timer", () => {
      seedQuery(TIMER_KEY, { running: null });
      expect(runCommand("timer:stop")).toBe(false);
      expect(mocked.runCommand).not.toHaveBeenCalled();
    });

    it("does not run before the timer status arrived", () => {
      expect(runCommand("timer:stop")).toBe(false);
    });

    it("asks the bus to stop the running timer", () => {
      seedQuery(TIMER_KEY, runningStatus());
      expect(runCommand("timer:stop")).toBe(true);
      expect(mocked.runCommand).toHaveBeenCalledWith("timer:stop");
    });
  });

  describe("timer:toggle", () => {
    it.each([
      ["no timer runs", { running: null }],
      ["a timer runs", runningStatus()],
    ])("asks the bus to toggle while %s", (_, timer) => {
      seedQuery(TIMER_KEY, timer);
      expect(runCommand("timer:toggle")).toBe(true);
      expect(mocked.runCommand).toHaveBeenCalledWith("timer:toggle");
    });
  });
});
