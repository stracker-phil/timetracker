import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api, EMPTY_UI_STATE } from "../api";
import { flushUiState, resetStore, useStore } from "../state/store";
import { settings } from "../test/settings";
import { installCommands, runCommand } from "./index";

const mocked = vi.mocked(api, true);
const zoom = () => useStore.getState().uiState.timelineZoom;

describe("zoom commands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ settings: settings() });
    installCommands();
  });

  afterEach(() => flushUiState());

  describe("in the Day view", () => {
    it.each([
      ["zoom.in", 2.5, 3],
      ["zoom.in", 8, 8],
      ["zoom.in", 1, 1.5],
      ["zoom.out", 2.5, 2],
      ["zoom.out", 1, 1],
      ["zoom.out", 4, 3],
      ["zoom.reset", 6, 2.5],
    ])("%s from %s px per minute gives %s", (id, from, expected) => {
      useStore.setState({ view: "day", uiState: { ...EMPTY_UI_STATE, timelineZoom: from } });
      runCommand(id);
      expect(zoom()).toBe(expected);
    });

    it("leaves the text sizes alone", () => {
      useStore.setState({ view: "day" });
      runCommand("zoom.in");
      runCommand("zoom.reset");
      expect(mocked.updateSettings).not.toHaveBeenCalled();
    });
  });

  describe.each(["week", "month", "reports", "projects"] as const)("in the %s view", (view) => {
    beforeEach(() => useStore.setState({ view }));

    it.each([
      ["zoom.in", "small", "medium"],
      ["zoom.in", "medium", "large"],
      ["zoom.out", "large", "medium"],
      ["zoom.out", "medium", "small"],
      ["zoom.reset", "small", "medium"],
      ["zoom.reset", "large", "medium"],
    ] as const)("%s from %s saves %s", (id, from, expected) => {
      useStore.setState({ settings: settings({ textSize: from }) });
      runCommand(id);
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ textSize: expected }));
    });

    it.each([
      ["zoom.in", "large"],
      ["zoom.out", "small"],
      ["zoom.reset", "medium"],
    ] as const)("%s saves nothing at %s", (id, at) => {
      useStore.setState({ settings: settings({ textSize: at }) });
      runCommand(id);
      expect(mocked.updateSettings).not.toHaveBeenCalled();
    });

    it("steps from the saved size twice in a row", async () => {
      useStore.setState({ settings: settings({ textSize: "small" }) });
      runCommand("zoom.in");
      await vi.waitFor(() => expect(useStore.getState().settings?.textSize).toBe("medium"));
      runCommand("zoom.in");
      await vi.waitFor(() => expect(useStore.getState().settings?.textSize).toBe("large"));
    });

    it("leaves the timeline zoom alone", () => {
      runCommand("zoom.in");
      expect(zoom()).toBe(EMPTY_UI_STATE.timelineZoom);
    });
  });
});
