import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { resetStore, useStore } from "../state/store";
import { savedReport } from "../test/reportFixtures";
import { installCommands, listenPageCommands, notePaletteRunnable, paletteCanRun } from "./index";

const mocked = vi.mocked(api, true);
const state = () => useStore.getState();

/** Delivers a `page_command` event the way Rust does. */
async function pageCommand(id: string) {
  await listenPageCommands();
  mocked.onPageCommand.mock.calls.at(-1)![0](id);
}

describe("page_command", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ view: "week" });
    installCommands();
  });

  it("runs the command it names when it can run", async () => {
    await pageCommand("nav.goToDate");
    expect(state().dialog).toEqual({ type: "datePicker" });
  });

  it("runs a command of the global scope in any view", async () => {
    await pageCommand("view.projects");
    expect(state().view).toBe("projects");
  });

  it("ignores a command whose scope is not active", async () => {
    resetStore({ view: "projects" });
    await pageCommand("nav.goToDate");
    expect(state().dialog).toBeNull();
  });

  it("ignores a command whose condition does not hold", async () => {
    resetStore({ view: "reports", openedReport: null });
    await pageCommand("reports.rename");
    expect(state().dialog).toBeNull();
  });

  it("runs a command whose condition holds", async () => {
    resetStore({ view: "reports", savedReports: [savedReport({ id: "s1" })], openedReport: "s1" });
    await pageCommand("reports.rename");
    expect(state().dialog).toMatchObject({ type: "prompt", title: "Rename report" });
  });

  it("ignores an unknown id", async () => {
    await pageCommand("nothing.here");
    expect(state().view).toBe("week");
    expect(state().dialog).toBeNull();
  });

  describe("over an overlay", () => {
    it("holds back a command the overlay does not allow", async () => {
      resetStore({ view: "week", dialog: { type: "shortcuts", scopes: ["global"] } });
      await pageCommand("view.projects");
      expect(state().view).toBe("week");
    });

    it("holds commands back while the palette is open", async () => {
      resetStore({ view: "week", paletteOpen: true });
      await pageCommand("view.projects");
      expect(state().view).toBe("week");
    });

    it("still runs a command the overlay allows", async () => {
      resetStore({ view: "week", dialog: { type: "shortcuts", scopes: ["global"] } });
      await pageCommand("shortcuts.show");
      expect(state().dialog).toBeNull();
    });
  });
});

describe("the palette's snapshot of runnable page commands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ view: "projects" });
    installCommands();
  });

  it("holds the menu commands that can run when it is noted", () => {
    notePaletteRunnable();
    expect(paletteCanRun("projects.new")).toBe(true);
    expect(paletteCanRun("view.week")).toBe(true);
  });

  it("leaves out menu commands of other scopes and commands whose condition fails", () => {
    notePaletteRunnable();
    expect(paletteCanRun("rules.new")).toBe(false);
    expect(paletteCanRun("reports.rename")).toBe(false);
  });

  it("leaves out commands that have no menu item", () => {
    notePaletteRunnable();
    expect(paletteCanRun("app:quit")).toBe(false);
    expect(paletteCanRun("nothing.here")).toBe(false);
  });

  it("keeps the snapshot while the page changes behind the palette", () => {
    notePaletteRunnable();
    resetStore({ view: "rules", paletteOpen: true });
    expect(paletteCanRun("projects.new")).toBe(true);
    expect(paletteCanRun("rules.new")).toBe(false);
  });

  it("is taken again at the next opening", () => {
    notePaletteRunnable();
    resetStore({ view: "rules" });
    notePaletteRunnable();
    expect(paletteCanRun("projects.new")).toBe(false);
    expect(paletteCanRun("rules.new")).toBe(true);
  });

  it("holds nothing when noted over an overlay", () => {
    resetStore({ view: "projects", dialog: { type: "shortcuts", scopes: ["global"] } });
    notePaletteRunnable();
    expect(paletteCanRun("projects.new")).toBe(false);
  });
});
