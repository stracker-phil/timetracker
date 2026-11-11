import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { resetStore, useStore } from "../state/store";
import { installCommands } from "./index";
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

  it("reveals the selected entry in the Day view and replies without an error", async () => {
    const entry = { day: "2026-10-20", kind: "recording", id: "r1" } as const;
    await answerRemote({ id: "14", request: { type: "select", entry } });
    expect(useStore.getState().view).toBe("day");
    expect(useStore.getState().day).toBe("2026-10-20");
    expect(useStore.getState().revealing).toEqual(entry);
    expect(mocked.remoteReply).toHaveBeenCalledWith("14", null, null);
  });

  describe("runCommand", () => {
    beforeEach(() => installCommands());

    it("runs a menu page command and replies without an error", async () => {
      await answerRemote({ id: "9", request: { type: "runCommand", id: "view.projects" } });
      expect(useStore.getState().view).toBe("projects");
      expect(mocked.remoteReply).toHaveBeenCalledWith("9", null, null);
    });

    it("replies with an error for an id that is no menu page command", async () => {
      await answerRemote({ id: "10", request: { type: "runCommand", id: "nothing.here" } });
      expect(mocked.remoteReply).toHaveBeenCalledWith("10", null, "Unknown command “page:nothing.here”.");
    });

    it("does not run a command that has no menu item", async () => {
      await answerRemote({ id: "11", request: { type: "runCommand", id: "day.nextLane" } });
      expect(mocked.remoteReply).toHaveBeenCalledWith("11", null, expect.stringContaining("Unknown command"));
    });

    it("replies with an error and runs nothing when the command cannot run now", async () => {
      await answerRemote({ id: "12", request: { type: "runCommand", id: "projects.new" } });
      expect(mocked.remoteReply).toHaveBeenCalledWith("12", null, "“page:projects.new” isn't available right now.");
      expect(useStore.getState().dialog).toBeNull();
    });

    it("replies with an error while an overlay holds the command back", async () => {
      resetStore({ view: "week", dialog: { type: "shortcuts", scopes: ["global"] } });
      await answerRemote({ id: "13", request: { type: "runCommand", id: "view.projects" } });
      expect(mocked.remoteReply).toHaveBeenCalledWith("13", null, expect.stringContaining("isn't available right now"));
      expect(useStore.getState().view).toBe("week");
    });
  });

  it("replies with an error for an unknown request and leaves the view alone", async () => {
    await answerRemote({ id: "8",request: { type: "teleport" } } as never);
    expect(mocked.remoteReply).toHaveBeenCalledWith("8", null, expect.stringContaining("Unknown request"));
    expect(useStore.getState().view).toBe("week");
    expect(useStore.getState().day).toBe("2026-01-01");
  });
});
