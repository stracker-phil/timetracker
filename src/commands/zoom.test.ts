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
      ["zoom.in", 14, 13],
      ["zoom.out", 12, 11],
    ])("%s saves the text sizes %s and %s", (id, ui, data) => {
      runCommand(id);
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ uiFontSize: ui, dataFontSize: data }));
    });

    it("zoom.reset saves the default text sizes", () => {
      useStore.setState({ settings: settings({ uiFontSize: 18, dataFontSize: 17 }) });
      runCommand("zoom.reset");
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ uiFontSize: 13, dataFontSize: 12 }));
    });

    it("leaves the timeline zoom alone", () => {
      runCommand("zoom.in");
      expect(zoom()).toBe(EMPTY_UI_STATE.timelineZoom);
    });
  });
});
