import { readFileSync } from "node:fs";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { VIEWS } from "../components/views";
import { resetStore, useStore, type View } from "../state/store";
import { at, DAY, dayView } from "../test/dayFixtures";
import { dayKey, seedQuery, SETTINGS_KEY } from "../test/queries";
import { storedSuggestion, suggestion } from "../test/ruleFixtures";
import { settings } from "../test/settings";
import { BUS_COMMANDS } from "./busCommands";
import { allCommands, getCommand, installCommands } from "./index";
import { installMenuSync, menuState, syncMenu } from "./menu";

const mocked = vi.mocked(api, true);
const state = () => useStore.getState();
const enabled = () => menuState().enabled;
const checked = () => menuState().checked;

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("the menus in menu/items.rs", () => {
  const PAGE = "page:";
  const source = readFileSync("src-tauri/src/menu/items.rs", "utf8");
  const menus = source.slice(source.indexOf("pub const APP_MENU"), source.indexOf("pub const SHARED_ACCELERATORS"));
  const rustItems = [...menus.matchAll(/\b(item|check)\(\s*"([^"]+)",\s*"([^"]+)",\s*"([^"]*)",?\s*\)/g)].map(([, kind, label, command, binding]) => ({ kind, label, command, binding }));
  const pageItems = rustItems.filter((i) => i.command.startsWith(PAGE)).map((i) => ({ ...i, id: i.command.slice(PAGE.length) }));
  const appItems = rustItems.filter((i) => !i.command.startsWith(PAGE));

  beforeEach(() => installCommands());

  it("reads the menu items out of the Rust source", () => {
    expect(rustItems.length).toBeGreaterThan(50);
    expect(rustItems.map((i) => i.command)).toEqual(expect.arrayContaining(["app:settings", "page:view.day", "page:recording.create"]));
  });

  it("lists the page: items of exactly the commands marked as menu items", () => {
    const marked = allCommands()
      .filter((c) => c.menu)
      .map((c) => c.id);
    expect(pageItems.map((i) => i.id).sort()).toEqual(marked.sort());
  });

  it("makes exactly the page commands with a checkmark checkbox items", () => {
    const checkboxes = allCommands()
      .filter((c) => c.menu && c.checked)
      .map((c) => c.id);
    expect(pageItems.filter((i) => i.kind === "check").map((i) => i.id).sort()).toEqual(checkboxes.sort());
  });

  it("shows the default binding of each page command, Record Selection borrowing the one of recording.open", () => {
    const shown = (id: string) => getCommand(id === "recording.create" ? "recording.open" : id)?.defaultBinding ?? "";
    const mismatches = pageItems.filter((i) => i.binding !== shown(i.id)).map((i) => [i.id, i.binding, shown(i.id)]);
    expect(mismatches).toEqual([]);
  });

  it("lists app items only for bus commands, with the bus command's default binding", () => {
    const bus = new Map(BUS_COMMANDS.map((c) => [c.id, c.defaultBinding ?? ""]));
    const unknown = appItems.filter((i) => !bus.has(i.command)).map((i) => i.command);
    const mismatches = appItems.filter((i) => bus.has(i.command) && bus.get(i.command) !== i.binding).map((i) => [i.command, i.binding, bus.get(i.command)]);
    expect(unknown).toEqual([]);
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

  });

  describe("bus commands", () => {
    it("reports none of them, which Rust enables itself", () => {
      seedQuery(SETTINGS_KEY, settings());
      expect(enabled().filter((id) => id.includes(":"))).toEqual([]);
    });
  });

  describe("checkmarks", () => {
    it.each(VIEWS.map((v) => v.id))("checks only view.%s while that view shows", (view) => {
      resetStore({ view });
      expect(checked().filter((id) => id.startsWith("view."))).toEqual([`view.${view}`]);
    });
  });

  describe("items of one view", () => {
    const ALL: View[] = ["day", "week", "month", "reports", "projects", "tags", "rules"];
    const DATED: View[] = ["day", "week", "month", "reports"];

    it.each<[string, View[]]>([
      ["reports.copyText", ["reports"]],
      ["reports.copyNotes", ["reports"]],
      ["projects.new", ["projects"]],
      ["clients.new", ["projects"]],
      ["rules.new", ["rules"]],
      ["tags.new", ["tags"]],
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
    const ALLOWED = ["shortcuts.show", "palette.toggle", "zoom.in", "zoom.out", "zoom.reset", "edit.undo", "edit.redo"];

    it.each<[string, Parameters<typeof resetStore>[0]]>([
      ["a dialog", { dialog: { type: "shortcuts", scopes: ["global"] } }],
      ["the palette", { paletteOpen: true }],
      ["the recording editor", { editor: { mode: "create", start: at("09:00"), end: at("10:00") } }],
    ])("enables only the items that still run over %s", (_, patch) => {
      seedQuery(SETTINGS_KEY, settings());
      resetStore({ view: "day", ...patch });
      const rest = enabled().filter((id) => !ALLOWED.includes(id));
      expect(rest).toEqual([]);
      expect(enabled()).toEqual(expect.arrayContaining(["zoom.in", "zoom.out", "zoom.reset"]));
    });
  });

  describe("suggestions", () => {
    const proposed = suggestion();

    beforeEach(() => {
      resetStore({ view: "day", day: DAY });
      seedQuery(dayKey(DAY), dayView({ suggestions: [proposed] }));
    });

    const FOCUSED = ["suggestion.approve", "suggestion.edit", "suggestion.dismiss", "suggestion.approveRule"];
    const ALL = ["suggestion.approveAll", "suggestion.dismissAll"];

    it("enables the items of one suggestion only while that suggestion is focused", () => {
      expect(enabled().filter((id) => FOCUSED.includes(id))).toEqual([]);
      state().focusSuggestion(proposed.id);
      expect(enabled()).toEqual(expect.arrayContaining(FOCUSED));
    });

    it("disables Approve all of the rule for a focused stored suggestion and keeps the other items", () => {
      const stored = storedSuggestion({ start: at("12:00"), end: at("12:30") });
      seedQuery(dayKey(DAY), dayView({ suggestions: [proposed, stored] }));
      state().focusSuggestion(stored.id);
      expect(enabled()).not.toContain("suggestion.approveRule");
      expect(enabled()).toEqual(expect.arrayContaining(["suggestion.approve", "suggestion.edit", "suggestion.dismiss"]));
    });

    it("keeps them disabled for a focused suggestion the day does not hold", () => {
      state().focusSuggestion("other:1:2");
      expect(enabled().filter((id) => FOCUSED.includes(id))).toEqual([]);
    });

    it("keeps them disabled once a selection takes the focus", () => {
      state().focusSuggestion(proposed.id);
      state().setSelection({ start: at("09:00"), end: at("10:00"), blockIds: [] });
      expect(enabled().filter((id) => FOCUSED.includes(id))).toEqual([]);
    });

    it("enables Approve all and Dismiss all while the day has suggestions, focused or not", () => {
      expect(enabled()).toEqual(expect.arrayContaining(ALL));
    });

    it("disables Approve all and Dismiss all on a day without suggestions", () => {
      seedQuery(dayKey(DAY), dayView());
      expect(enabled().filter((id) => ALL.includes(id))).toEqual([]);
    });

    it("disables all of them outside the Day view", () => {
      state().focusSuggestion(proposed.id);
      state().setView("week");
      expect(enabled().filter((id) => id.startsWith("suggestion."))).toEqual([]);
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

  it("follows the focus onto and off a suggestion", () => {
    const proposed = suggestion();
    resetStore({ view: "day", day: DAY });
    seedQuery(dayKey(DAY), dayView({ suggestions: [proposed] }));
    stop =installMenuSync();
    mocked.menuSync.mockClear();
    state().focusSuggestion(proposed.id);
    expect(mocked.menuSync).toHaveBeenLastCalledWith(expect.objectContaining({ enabled: expect.arrayContaining(["suggestion.approve"]) }));
    state().focusSuggestion(null);
    expect(mocked.menuSync.mock.lastCall?.[0].enabled).not.toContain("suggestion.approve");
  });

  it("stops following once uninstalled", () => {
    installMenuSync()();
    mocked.menuSync.mockClear();
    state().setView("rules");
    expect(mocked.menuSync).not.toHaveBeenCalled();
  });
});
