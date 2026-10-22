import { readFileSync } from "node:fs";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api, EMPTY_UI_STATE } from "../api";
import { VIEWS } from "../components/views";
import { resetStore, useStore, type View } from "../state/store";
import { at, DAY, dayView } from "../test/dayFixtures";
import { suggestion } from "../test/ruleFixtures";
import { settings } from "../test/settings";
import { runningStatus } from "../test/timerFixtures";
import { allCommands, getCommand, installCommands } from "./index";
import { installMenuSync, menuState, syncMenu } from "./menu";

const mocked = vi.mocked(api, true);
const state = () => useStore.getState();
const enabled = () => menuState().enabled;
const checked = () => menuState().checked;

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

function focusSuggestion(id: string) {
  const button = document.body.appendChild(document.createElement("button"));
  button.dataset.suggestionId = id;
  button.focus();
  return button;
}

describe("the native menu list", () => {
  const source = readFileSync("src-tauri/src/menu/items.rs", "utf8");
  const list = source.slice(source.indexOf("pub const MENU_ITEMS"), source.indexOf("];", source.indexOf("pub const MENU_ITEMS")));
  const rustItems = [...list.matchAll(/\b(item|check)\(\s*"([^"]+)",\s*"[^"]+",\s*"([^"]*)",?\s*\)/g)].map(([, kind, id, binding]) => ({ kind, id, binding }));

  beforeEach(() => installCommands());

  it("lists the same command ids as the commands marked as menu items", () => {
    const marked = allCommands()
      .filter((c) => c.menu)
      .map((c) => c.id);
    expect(rustItems.map((i) => i.id).sort()).toEqual(marked.sort());
  });

  it("makes exactly the commands with a checkmark checkbox items", () => {
    const checkboxes = allCommands()
      .filter((c) => c.menu && c.checked)
      .map((c) => c.id);
    expect(rustItems.filter((i) => i.kind === "check").map((i) => i.id).sort()).toEqual(checkboxes.sort());
  });

  it("shows the default binding of each command, Record Selection borrowing the one of recording.open", () => {
    const shown = (id: string) => getCommand(id === "recording.create" ? "recording.open" : id)?.defaultBinding ?? "";
    const mismatches = rustItems.filter((i) => i.binding !== shown(i.id)).map((i) => [i.id, i.binding, shown(i.id)]);
    expect(mismatches).toEqual([]);
  });
});

describe("menuState", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ view: "day" });
    installCommands();
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  describe("recording items", () => {
    const recordingItems = ["recording.create", "recording.edit", "recording.delete", "recording.moveToPrevDay", "recording.copyToNextDay"];

    it("enables none of them without a selection or a focused recording", () => {
      expect(enabled().filter((id) => recordingItems.includes(id))).toEqual([]);
    });

    it("enables only Record Selection for a selection", () => {
      state().setSelection({ start: at("09:00"), end: at("10:00"), blockIds: [] });
      expect(enabled().filter((id) => recordingItems.includes(id))).toEqual(["recording.create"]);
    });

    it("enables edit, delete, move and copy for a focused recording", () => {
      state().focusRecording("r1");
      expect(enabled()).toEqual(expect.arrayContaining(["recording.edit", "recording.delete", "recording.moveToPrevDay", "recording.moveToNextDay", "recording.copyToPrevDay", "recording.copyToNextDay"]));
      expect(enabled()).not.toContain("recording.create");
    });

    it("enables neither Edit Recording nor the block steps in the List mode", () => {
      resetStore({ view: "day", focusedRecording: "r1", uiState: { ...EMPTY_UI_STATE, dayMode: "list" } });
      expect(enabled()).not.toContain("recording.edit");
      expect(enabled()).not.toContain("day.nextBlock");
    });
  });

  describe("timer.stop", () => {
    it("is enabled while a timer runs", () => {
      resetStore({ timer: runningStatus() });
      expect(enabled()).toContain("timer.stop");
    });

    it("is disabled without a running timer", () => {
      resetStore({ timer: { running: null } });
      expect(enabled()).not.toContain("timer.stop");
    });
  });

  describe("checkmarks", () => {
    it.each(VIEWS.map((v) => v.id))("checks only view.%s while that view shows", (view) => {
      resetStore({ view });
      expect(checked().filter((id) => id.startsWith("view."))).toEqual([`view.${view}`]);
    });

    it("checks day.toggleList in the List mode only", () => {
      expect(checked()).not.toContain("day.toggleList");
      resetStore({ view: "day", uiState: { ...EMPTY_UI_STATE, dayMode: "list" } });
      expect(checked()).toContain("day.toggleList");
    });

    it.each([
      [true, true],
      [false, false],
    ])("checks app.launchAtLogin when the setting is %s: %s", (launchAtLogin, isChecked) => {
      resetStore({ settings: settings({ launchAtLogin }) });
      expect(checked().includes("app.launchAtLogin")).toBe(isChecked);
    });

    it("disables app.launchAtLogin until the settings are loaded", () => {
      expect(enabled()).not.toContain("app.launchAtLogin");
      resetStore({ settings: settings() });
      expect(enabled()).toContain("app.launchAtLogin");
    });
  });

  describe("items of one view", () => {
    const ALL: View[] = ["day", "week", "month", "reports", "projects", "rules"];
    const DATED: View[] = ["day", "week", "month", "reports"];

    it.each<[string, View[]]>([
      ["reports.copyText", ["reports"]],
      ["reports.copyNotes", ["reports"]],
      ["projects.new", ["projects"]],
      ["clients.new", ["projects"]],
      ["rules.new", ["rules"]],
      ["day.settings", ["day"]],
      ["nav.prev", DATED],
      ["nav.next", DATED],
      ["nav.today", DATED],
      ["nav.goToDate", DATED],
    ])("%s is enabled in exactly its views", (id, views) => {
      const shown = ALL.filter((view) => {
        resetStore({ view });
        return enabled().includes(id);
      });
      expect(shown).toEqual(views);
    });
  });

  describe("while an overlay is open", () => {
    const ALLOWED = ["settings.toggle", "shortcuts.show", "palette.toggle", "zoom.in", "zoom.out", "zoom.reset", "window.hide", "app.quit", "edit.undo", "edit.redo"];

    it.each<[string, Parameters<typeof resetStore>[0]]>([
      ["a dialog", { dialog: { type: "settings" } }],
      ["the palette", { paletteOpen: true }],
      ["the recording editor", { editor: { mode: "create", start: at("09:00"), end: at("10:00") } }],
    ])("enables only the items that still run over %s", (_, patch) => {
      resetStore({ view: "day", settings: settings(), timer: runningStatus(), ...patch });
      const rest = enabled().filter((id) => !ALLOWED.includes(id));
      expect(rest).toEqual([]);
      expect(enabled()).toEqual(expect.arrayContaining(["settings.toggle", "app.quit", "zoom.in"]));
    });
  });

  describe("suggestions", () => {
    const proposed = suggestion();

    beforeEach(() => resetStore({ view: "day", day: DAY, dayView: dayView({ suggestions: [proposed] }) }));

    it("enables the accept items while a suggestion's button has the focus", () => {
      expect(enabled()).not.toContain("suggestion.accept");
      focusSuggestion(proposed.id);
      expect(enabled()).toEqual(expect.arrayContaining(["suggestion.accept", "suggestion.acceptRule"]));
    });

    it("keeps them disabled for a focused button of a suggestion the day does not hold", () => {
      focusSuggestion("other:1:2");
      expect(enabled()).not.toContain("suggestion.accept");
    });
  });
});

describe("menu sync", () => {
  let stop = () => {};

  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ view: "day" });
    installCommands();
  });

  afterEach(() => {
    stop();
    stop = () => {};
    document.body.innerHTML = "";
  });

  it("sends the state when the sync is installed", () => {
    stop = installMenuSync();
    expect(mocked.menuSync).toHaveBeenCalledWith(menuState());
  });

  it("sends nothing while the state is unchanged and sends again once it changed", () => {
    stop = installMenuSync();
    mocked.menuSync.mockClear();
    syncMenu();
    expect(mocked.menuSync).not.toHaveBeenCalled();
    state().setView("week");
    syncMenu();
    expect(mocked.menuSync).toHaveBeenCalledWith(expect.objectContaining({ checked: expect.arrayContaining(["view.week"]) }));
  });

  it("sends again after a failed send", async () => {
    mocked.menuSync.mockRejectedValueOnce({ kind: "io", message: "gone", code: null });
    stop = installMenuSync();
    await flush();
    mocked.menuSync.mockClear();
    syncMenu();
    expect(mocked.menuSync).toHaveBeenCalledWith(menuState());
  });

  it("follows store changes", () => {
    stop = installMenuSync();
    mocked.menuSync.mockClear();
    state().setView("projects");
    expect(mocked.menuSync).toHaveBeenCalledWith(expect.objectContaining({ enabled: expect.arrayContaining(["projects.new"]) }));
  });

  it("follows the keyboard focus onto and off a suggestion", async () => {
    const proposed = suggestion();
    resetStore({ view: "day", day: DAY, dayView: dayView({ suggestions: [proposed] }) });
    stop = installMenuSync();
    mocked.menuSync.mockClear();
    const button = focusSuggestion(proposed.id);
    await flush();
    expect(mocked.menuSync).toHaveBeenLastCalledWith(expect.objectContaining({ enabled: expect.arrayContaining(["suggestion.accept"]) }));
    button.blur();
    await flush();
    expect(mocked.menuSync.mock.lastCall?.[0].enabled).not.toContain("suggestion.accept");
  });

  it("stops following once uninstalled", () => {
    installMenuSync()();
    mocked.menuSync.mockClear();
    state().setView("rules");
    expect(mocked.menuSync).not.toHaveBeenCalled();
  });
});
