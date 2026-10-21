import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { resetStore, useStore, type View } from "../state/store";
import { handleKeydown, installCommands, runCommand } from "./index";

const state = () => useStore.getState();

describe("calendar navigation commands", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["Date"], now: new Date(2026, 9, 21, 12) });
    installCommands();
  });

  afterEach(() => vi.useRealTimers());

  it.each<[View, string, string]>([
    ["day", "nav.next", "2026-03-11"],
    ["day", "nav.prev", "2026-03-09"],
    ["week", "nav.next", "2026-03-17"],
    ["week", "nav.prev", "2026-03-03"],
    ["month", "nav.next", "2026-04-01"],
    ["month", "nav.prev", "2026-02-01"],
  ])("in the %s view %s goes to %s", (view, id, expected) => {
    resetStore({ view, day: "2026-03-10" });
    expect(runCommand(id)).toBe(true);
    expect(state().day).toBe(expected);
    expect(state().view).toBe(view);
  });

  it.each<View>(["day", "week", "month"])("nav.today shows today in the %s view", (view) => {
    resetStore({ view, day: "2026-03-10" });
    expect(runCommand("nav.today")).toBe(true);
    expect(state().day).toBe("2026-10-21");
    expect(state().view).toBe(view);
  });

  it.each<[View, string]>([
    ["reports", "{Meta>}{ArrowRight}{/Meta}"],
    ["reports", "{Meta>}{ArrowLeft}{/Meta}"],
    ["projects", "{Meta>}{ArrowRight}{/Meta}"],
    ["projects", "t"],
  ])("a key press in the %s view (%s) leaves the day alone", async (view, keys) => {
    resetStore({ view, day: "2026-03-10" });
    window.addEventListener("keydown", handleKeydown, true);
    try {
      await userEvent.keyboard(keys);
    } finally {
      window.removeEventListener("keydown", handleKeydown, true);
    }
    expect(state().day).toBe("2026-03-10");
  });

  it("⌘→ and ⌘← step by the view's unit", async () => {
    resetStore({ view: "week", day: "2026-03-10" });
    window.addEventListener("keydown", handleKeydown, true);
    try {
      await userEvent.keyboard("{Meta>}{ArrowRight}{/Meta}");
      expect(state().day).toBe("2026-03-17");
      await userEvent.keyboard("{Meta>}{ArrowLeft}{ArrowLeft}{/Meta}");
      expect(state().day).toBe("2026-03-03");
    } finally {
      window.removeEventListener("keydown", handleKeydown, true);
    }
  });
});
