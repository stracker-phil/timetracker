import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { resetStore, useStore } from "../store";

describe("ui slice", () => {
  describe("goToDay", () => {
    beforeEach(() => resetStore({ view: "reports" }));

    it("sets the day and switches to the Day view", () => {
      useStore.getState().goToDay("2026-10-20");
      expect(useStore.getState().day).toBe("2026-10-20");
      expect(useStore.getState().view).toBe("day");
    });
  });

  it("starts on today as YYYY-MM-DD", () => {
    resetStore();
    expect(useStore.getState().day).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });
});
