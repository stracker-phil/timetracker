import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { resetStore, useStore } from "../store";

describe("ui slice", () => {
  beforeEach(() => resetStore());

  describe("toast", () => {
    it("keeps the action of a toast", () => {
      const run = vi.fn();
      useStore.getState().toast("info", "Hidden", { label: "Undo", run });
      const [toast] = useStore.getState().toasts;
      expect(toast.action?.label).toBe("Undo");
      toast.action?.run();
      expect(run).toHaveBeenCalled();
    });

    it("counts a repeated message instead of adding a second toast", () => {
      useStore.getState().toast("info", "Same");
      useStore.getState().toast("info", "Same");
      expect(useStore.getState().toasts).toHaveLength(1);
      expect(useStore.getState().toasts[0].count).toBe(2);
    });
  });
});
