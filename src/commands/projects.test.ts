import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { resetStore, useStore, type View } from "../state/store";
import { installCommands, runMenuCommand } from "./index";

describe("project commands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    installCommands();
  });

  it.each<[string, { type: string }]>([
    ["projects.new", { type: "project" }],
    ["clients.new", { type: "client" }],
  ])("%s opens its sheet while the Projects view shows", (id, dialog) => {
    resetStore({ view: "projects" });
    expect(runMenuCommand(id)).toBe(true);
    expect(useStore.getState().dialog).toEqual(dialog);
  });

  it.each<View>(["day", "week", "month", "reports"])("projects.new does not run in the %s view", (view) => {
    resetStore({ view });
    expect(runMenuCommand("projects.new")).toBe(false);
    expect(useStore.getState().dialog).toBeNull();
  });
});
