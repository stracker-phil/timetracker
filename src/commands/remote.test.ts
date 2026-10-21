import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { resetStore, useStore } from "../state/store";
import { answerRemote } from "./remote";

const mocked = vi.mocked(api, true);

describe("answerRemote", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ view: "week", day: "2026-01-01" });
  });

  it("shows the requested day in the Day view and replies without an error", async () => {
    await answerRemote({ id: "7",request: { type: "showDay", day: "2026-10-20" } });
    expect(useStore.getState().day).toBe("2026-10-20");
    expect(useStore.getState().view).toBe("day");
    expect(mocked.remoteReply).toHaveBeenCalledWith("7", null, null);
  });

  it("replies with an error for an unknown request and leaves the view alone", async () => {
    await answerRemote({ id: "8",request: { type: "teleport" } } as never);
    expect(mocked.remoteReply).toHaveBeenCalledWith("8", null, expect.stringContaining("Unknown request"));
    expect(useStore.getState().view).toBe("week");
    expect(useStore.getState().day).toBe("2026-01-01");
  });
});
