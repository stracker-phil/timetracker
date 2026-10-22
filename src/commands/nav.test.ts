import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { resetStore, useStore, type View } from "../state/store";
import { defaultFilter } from "../state/slices/reports";
import { conflicts, handleKeydown, installCommands, runCommand } from "./index";

const mocked = vi.mocked(api, true);
const state = () => useStore.getState();

async function press(keys: string) {
  window.addEventListener("keydown", handleKeydown, true);
  try {
    await userEvent.keyboard(keys);
  } finally {
    window.removeEventListener("keydown", handleKeydown, true);
  }
}

describe("view commands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    installCommands();
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  it.each<[string, View]>([
    ["{Meta>}1{/Meta}", "day"],
    ["{Meta>}2{/Meta}", "week"],
    ["{Meta>}3{/Meta}", "month"],
    ["{Meta>}4{/Meta}", "reports"],
    ["{Meta>}5{/Meta}", "projects"],
    ["{Meta>}6{/Meta}", "tags"],
    ["{Meta>}7{/Meta}", "rules"],
  ])("%s shows the %s view", async (keys, view) => {
    resetStore({ view: view === "day" ? "rules" : "day" });
    await press(keys);
    expect(state().view).toBe(view);
  });

  it.each<View>(["day", "week", "month", "reports", "projects", "tags", "rules"])("view.%s shows that view from any other", (view) => {
    resetStore({ view: view === "day" ? "rules" : "day" });
    expect(runCommand(`view.${view}`)).toBe(true);
    expect(state().view).toBe(view);
  });
});

describe("date navigation commands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers({ toFake: ["Date"], now: new Date(2026, 9, 21, 12) });
    installCommands();
  });

  afterEach(() => {
    document.body.innerHTML = "";
    vi.useRealTimers();
  });

  it("has no default binding that two commands of overlapping scopes share", () => {
    expect(conflicts().map((c) => [c.binding, c.a.id, c.b.id])).toEqual([]);
  });

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

  describe("keys", () => {
    it.each<[View, string, string]>([
      ["day", "n", "2026-03-11"],
      ["day", "p", "2026-03-09"],
      ["week", "n", "2026-03-17"],
      ["week", "p", "2026-03-03"],
      ["month", "n", "2026-04-01"],
      ["month", "p", "2026-02-01"],
      ["day", "o", "2026-10-21"],
      ["week", "o", "2026-10-21"],
      ["month", "o", "2026-10-21"],
    ])("in the %s view %s moves the day to %s", async (view, keys, expected) => {
      resetStore({ view, day: "2026-03-10" });
      await press(keys);
      expect(state().day).toBe(expected);
      expect(state().view).toBe(view);
    });

    it.each<[string, string | null, number]>([
      ["n", null, 1],
      ["p", null, -1],
      ["o", "2026-10-21", 0],
    ])("in the Reports view %s shifts the period from anchor %s by %i", async (keys, anchor, by) => {
      const range = { kind: "thisWeek" } as const;
      resetStore({ view: "reports", day: "2026-03-10", reportFilter: { ...defaultFilter(null), range } });
      await press(keys);
      expect(mocked.reportRangeShift).toHaveBeenCalledWith(range, anchor, by);
      expect(state().day).toBe("2026-03-10");
    });

    it.each<[View, string]>([
      ["projects", "n"],
      ["projects", "p"],
      ["projects", "o"],
      ["tags", "n"],
      ["tags", "p"],
      ["tags", "o"],
      ["rules", "n"],
      ["rules", "p"],
      ["rules", "o"],
    ])("in the %s view %s leaves the day alone", async (view, keys) => {
      resetStore({ view, day: "2026-03-10" });
      await press(keys);
      expect(state().day).toBe("2026-03-10");
      expect(mocked.reportRangeShift).not.toHaveBeenCalled();
    });

    it.each(["n", "p", "o"])("%s types into a focused text field instead of navigating", async (keys) => {
      resetStore({ view: "day", day: "2026-03-10" });
      const input = document.body.appendChild(document.createElement("input"));
      input.focus();
      await press(keys);
      expect(input.value).toBe(keys);
      expect(state().day).toBe("2026-03-10");
    });

    it.each<[View, string]>([
      ["day", "t"],
      ["week", "t"],
      ["reports", "t"],
      ["week", "{Meta>}{ArrowRight}{/Meta}"],
      ["month", "{Meta>}{ArrowLeft}{/Meta}"],
    ])("in the %s view %s does nothing", async (view, keys) => {
      resetStore({ view, day: "2026-03-10" });
      await press(keys);
      expect(state().day).toBe("2026-03-10");
      expect(state().view).toBe(view);
      expect(mocked.reportRangeShift).not.toHaveBeenCalled();
    });
  });
});
