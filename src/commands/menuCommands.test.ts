import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api, EMPTY_UI_STATE } from "../api";
import { resetStore, useStore } from "../state/store";
import { dataInfo } from "../test/data";
import { at, DAY, dayView } from "../test/dayFixtures";
import { suggestion } from "../test/ruleFixtures";
import { settings } from "../test/settings";
import { allCommands, installCommands, runCommand, runMenuCommand } from "./index";
import { menuState } from "./menu";

const mocked = vi.mocked(api, true);
const state = () => useStore.getState();

describe("recording.new", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers({ toFake: ["Date"], now: new Date(2026, 9, 21, 12, 10) });
    resetStore({ view: "projects", day: DAY });
    installCommands();
  });

  afterEach(() => vi.useRealTimers());

  it("opens the editor on the Day view's selection as Rust snaps it", async () => {
    mocked.recordingSnap.mockResolvedValueOnce({ start: at("09:00"), end: at("10:30") });
    resetStore({ view: "day", day: DAY });
    state().setSelection({ start: at("09:02"), end: at("10:28"), blockIds: [] });
    expect(runCommand("recording.new")).toBe(true);
    await vi.waitFor(() => expect(state().editor).toEqual({ mode: "create", start: at("09:00"), end: at("10:30") }));
    expect(mocked.recordingSnap).toHaveBeenCalledWith(at("09:02"), at("10:28"), null);
  });

  it("switches to today's Day view and opens the editor on Rust's default span", async () => {
    const span = { start: new Date(2026, 9, 21, 11, 30).getTime(), end: new Date(2026, 9, 21, 12, 0).getTime() };
    mocked.recordingDefaultSpan.mockResolvedValueOnce(span);
    runCommand("recording.new");
    await vi.waitFor(() => expect(state().editor).toEqual({ mode: "create", ...span }));
    expect(state().view).toBe("day");
    expect(state().day).toBe("2026-10-21");
  });

  it("stays in the Week view and goes to today", async () => {
    resetStore({ view: "week", day: DAY });
    runCommand("recording.new");
    await vi.waitFor(() => expect(state().editor).toMatchObject({ mode: "create" }));
    expect(state().view).toBe("week");
    expect(state().day).toBe("2026-10-21");
  });

  it("ignores a selection that is not on the Day view", async () => {
    resetStore({ view: "week", day: DAY });
    state().setSelection({ start: at("09:00"), end: at("10:00"), blockIds: [] });
    runCommand("recording.new");
    await vi.waitFor(() => expect(state().editor).toMatchObject({ mode: "create" }));
    expect(state().editor).not.toMatchObject({ start: at("09:00") });
  });

  it("shows an error and opens no editor when the default span cannot be fetched", async () => {
    mocked.recordingDefaultSpan.mockRejectedValueOnce({ kind: "io", message: "no snap", code: null });
    runCommand("recording.new");
    await vi.waitFor(() => expect(state().toasts.map((t) => t.message)).toEqual(["Could not open a new recording: no snap"]));
    expect(state().editor).toBeNull();
  });
});

describe("nav.goToDate", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ view: "week" });
    installCommands();
  });

  it("opens the date picker", () => {
    expect(runMenuCommand("nav.goToDate")).toBe(true);
    expect(state().dialog).toEqual({ type: "datePicker" });
  });

  it("does not run outside the views with a period", () => {
    resetStore({ view: "projects" });
    expect(runMenuCommand("nav.goToDate")).toBe(false);
    expect(state().dialog).toBeNull();
  });
});

describe("suggestion commands", () => {
  const proposed = suggestion({ ruleId: "rule-slack" });

  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ view: "day", day: DAY, dayView: dayView({ suggestions: [proposed] }) });
    installCommands();
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  function focusSuggestion() {
    const button = document.body.appendChild(document.createElement("button"));
    button.dataset.suggestionId = proposed.id;
    button.focus();
  }

  it("suggestion.accept records the focused suggestion on the shown day", async () => {
    focusSuggestion();
    expect(runMenuCommand("suggestion.accept")).toBe(true);
    await vi.waitFor(() => expect(mocked.suggestionAccept).toHaveBeenCalledWith(DAY, proposed.id));
    expect(mocked.suggestionsAcceptRule).not.toHaveBeenCalled();
  });

  it("suggestion.acceptRule records every suggestion of the focused one's rule", async () => {
    focusSuggestion();
    expect(runMenuCommand("suggestion.acceptRule")).toBe(true);
    await vi.waitFor(() => expect(mocked.suggestionsAcceptRule).toHaveBeenCalledWith(DAY, "rule-slack"));
    expect(mocked.suggestionAccept).not.toHaveBeenCalled();
  });

  it.each(["suggestion.accept", "suggestion.acceptRule"])("%s does not run without a focused suggestion", (id) => {
    expect(runMenuCommand(id)).toBe(false);
  });
});

describe("app.launchAtLogin", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    installCommands();
  });

  it.each([
    [false, true],
    [true, false],
  ])("turns launch at login from %s to %s", async (current, next) => {
    resetStore({ settings: settings({ launchAtLogin: current }) });
    expect(runCommand("app.launchAtLogin")).toBe(true);
    await vi.waitFor(() => expect(mocked.autostartSet).toHaveBeenCalledWith(next));
    await vi.waitFor(() => expect(state().settings?.launchAtLogin).toBe(next));
  });

  it("does not run before the settings are loaded", () => {
    expect(runCommand("app.launchAtLogin")).toBe(false);
    expect(mocked.autostartSet).not.toHaveBeenCalled();
  });
});

describe("welcome.show", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ dialog: { type: "settings" }, uiState: { ...EMPTY_UI_STATE, onboardingDone: true } });
    installCommands();
  });

  it("closes the dialog and brings the welcome back", () => {
    expect(runCommand("welcome.show")).toBe(true);
    expect(state().dialog).toBeNull();
    expect(state().uiState.onboardingDone).toBe(false);
  });
});

describe("data and help commands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    installCommands();
  });

  it("data.exportBackup writes the backup to the chosen path and confirms with a toast", async () => {
    mocked.chooseSavePath.mockResolvedValue("/tmp/backup.json");
    mocked.backupExport.mockResolvedValue({ bytes: 10, recordings: 3, activities: 40 });
    expect(runCommand("data.exportBackup")).toBe(true);
    await vi.waitFor(() => expect(mocked.backupExport).toHaveBeenCalledWith("/tmp/backup.json"));
    await vi.waitFor(() => expect(state().toasts.map((t) => t.message)).toEqual(["Backup written: 3 recordings, 40 activity spans"]));
  });

  it("data.exportBackup writes nothing when the save panel is cancelled", async () => {
    mocked.chooseSavePath.mockResolvedValue(null);
    runCommand("data.exportBackup");
    await vi.waitFor(() => expect(mocked.chooseSavePath).toHaveBeenCalled());
    expect(mocked.backupExport).not.toHaveBeenCalled();
  });

  it("data.reveal shows the database file in Finder", async () => {
    mocked.dataInfo.mockResolvedValue(dataInfo({ file: "/data/dir/timetracker.sqlite" }));
    expect(runCommand("data.reveal")).toBe(true);
    await vi.waitFor(() => expect(mocked.revealPath).toHaveBeenCalledWith("/data/dir/timetracker.sqlite"));
  });

  it("data.reveal reports a failure", async () => {
    mocked.revealPath.mockRejectedValueOnce({ kind: "io", message: "no Finder", code: null });
    runCommand("data.reveal");
    await vi.waitFor(() => expect(state().toasts.map((t) => t.message)).toEqual(["Couldn't reveal the folder: no Finder"]));
  });

  it("help.readme opens the README", async () => {
    expect(runCommand("help.readme")).toBe(true);
    await vi.waitFor(() => expect(mocked.openReadme).toHaveBeenCalled());
  });

  it("help.readme reports a failure", async () => {
    mocked.openReadme.mockRejectedValueOnce({ kind: "not_found", message: "README is missing", code: null });
    runCommand("help.readme");
    await vi.waitFor(() => expect(state().toasts.map((t) => t.message)).toEqual(["Couldn't open the README: README is missing"]));
  });
});

describe("edit.undo and edit.redo", () => {
  const execCommand = vi.fn<(command: string) => boolean>(() => true);

  function focusInput() {
    const input = document.body.appendChild(document.createElement("input"));
    input.focus();
  }

  beforeEach(() => {
    vi.clearAllMocks();
    document.execCommand = execCommand;
    resetStore({ view: "day" });
    installCommands();
  });

  afterEach(() => {
    Reflect.deleteProperty(document, "execCommand");
    document.body.innerHTML = "";
  });

  it.each([
    ["edit.undo", "undo"],
    ["edit.redo", "redo"],
  ])("%s undoes the typing of a focused text field, not the last recording change", (id, native) => {
    focusInput();
    expect(runCommand(id)).toBe(true);
    expect(execCommand).toHaveBeenCalledWith(native);
    expect(mocked.undo).not.toHaveBeenCalled();
    expect(mocked.redo).not.toHaveBeenCalled();
  });

  it("undoes the typing of a text field inside the recording editor", () => {
    resetStore({ editor: { mode: "create", start: at("09:00"), end: at("10:00") } });
    focusInput();
    expect(runMenuCommand("edit.undo")).toBe(true);
    expect(execCommand).toHaveBeenCalledWith("undo");
  });

  it("does nothing in the recording editor without a text field", () => {
    resetStore({ editor: { mode: "create", start: at("09:00"), end: at("10:00") } });
    expect(runMenuCommand("edit.undo")).toBe(false);
    expect(execCommand).not.toHaveBeenCalled();
    expect(mocked.undo).not.toHaveBeenCalled();
  });

  it("leaves the palette's search field alone", () => {
    resetStore({ paletteOpen: true });
    focusInput();
    expect(runMenuCommand("edit.undo")).toBe(false);
    expect(execCommand).not.toHaveBeenCalled();
  });
});

describe("runMenuCommand", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    installCommands();
  });

  it.each<[string, Parameters<typeof resetStore>[0]]>([
    ["the Day view", { view: "day", settings: settings() }],
    ["the Projects view", { view: "projects" }],
    ["the Reports view", { view: "reports" }],
    ["a Day view with a focused recording", { view: "day", focusedRecording: "r1", dayView: dayView(), day: DAY }],
    ["an open dialog", { view: "day", dialog: { type: "settings" } }],
    ["an open recording editor", { view: "day", editor: { mode: "create", start: at("09:00"), end: at("10:00") } }],
  ])("runs exactly the items the menu enables in %s", (_, patch) => {
    resetStore(patch);
    const enabled = menuState().enabled;
    const ran = allCommands()
      .filter((c) => c.menu)
      .filter((c) => {
        resetStore(patch);
        return runMenuCommand(c.id);
      })
      .map((c) => c.id);
    expect(ran).toEqual(allCommands().filter((c) => enabled.includes(c.id)).map((c) => c.id));
  });

  it("refuses an unknown id", () => {
    expect(runMenuCommand("nothing.here")).toBe(false);
  });
});
