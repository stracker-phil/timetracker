import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api, EMPTY_UI_STATE } from "../api";
import { resetStore, useStore } from "../state/store";
import { at, DAY, dayView } from "../test/dayFixtures";
import { dayKey, seedQuery, SETTINGS_KEY } from "../test/queries";
import { storedSuggestion, suggestion } from "../test/ruleFixtures";
import { settings } from "../test/settings";
import { allCommands, conflicts, installCommands, runCommand, runMenuCommand } from "./index";
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
    expect(mocked.recordingSnap).toHaveBeenCalledWith(at("09:02"), at("10:28"));
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

  it("stays in the Month view, goes to today and opens the editor", async () => {
    resetStore({ view: "month", day: DAY });
    runCommand("recording.new");
    await vi.waitFor(() => expect(state().editor).toMatchObject({ mode: "create" }));
    expect(state().view).toBe("month");
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
  const proposed = suggestion({ ruleIds: ["rule-slack"] });

  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ view: "day", day: DAY });
    seedQuery(dayKey(DAY), dayView({ suggestions: [proposed] }));
    installCommands();
  });

  it("suggestion.approve records the focused suggestion on the shown day", async () => {
    state().focusSuggestion(proposed.id);
    expect(runMenuCommand("suggestion.approve")).toBe(true);
    await vi.waitFor(() => expect(mocked.suggestionApprove).toHaveBeenCalledWith(DAY, proposed.id));
    expect(mocked.suggestionApproveRule).not.toHaveBeenCalled();
  });

  it("suggestion.approveRule records every suggestion of the focused one's rule", async () => {
    state().focusSuggestion(proposed.id);
    expect(runMenuCommand("suggestion.approveRule")).toBe(true);
    await vi.waitFor(() => expect(mocked.suggestionApproveRule).toHaveBeenCalledWith(DAY, "rule-slack"));
    expect(mocked.suggestionApprove).not.toHaveBeenCalled();
  });

  it("suggestion.approveRule does not run on a stored suggestion, which has no rule", () => {
    const stored = storedSuggestion({ start: at("12:00"), end: at("12:30") });
    seedQuery(dayKey(DAY), dayView({ suggestions: [proposed, stored] }));
    state().focusSuggestion(stored.id);
    expect(runMenuCommand("suggestion.approveRule")).toBe(false);
    expect(mocked.suggestionApproveRule).not.toHaveBeenCalled();
    expect(runMenuCommand("suggestion.approve")).toBe(true);
  });

  it("suggestion.edit opens the editor on the focused suggestion", () => {
    state().focusSuggestion(proposed.id);
    expect(runMenuCommand("suggestion.edit")).toBe(true);
    expect(state().editor).toMatchObject({ mode: "create", suggestion: { id: proposed.id, by: proposed.by } });
  });

  it("suggestion.dismiss dismisses the focused suggestion", async () => {
    state().focusSuggestion(proposed.id);
    expect(runMenuCommand("suggestion.dismiss")).toBe(true);
    await vi.waitFor(() => expect(mocked.suggestionsDismiss).toHaveBeenCalledWith(DAY, [proposed.id]));
  });

  it("suggestion.approveAll approves every suggestion of the shown day without a focus", async () => {
    const other = suggestion({ ruleIds: ["rule-other"], start: at("11:00"), end: at("11:30") });
    seedQuery(dayKey(DAY), dayView({ suggestions: [proposed, other] }));
    expect(runMenuCommand("suggestion.approveAll")).toBe(true);
    await vi.waitFor(() => expect(mocked.suggestionsApprove).toHaveBeenCalledWith(DAY, [proposed.id, other.id]));
  });

  it("suggestion.approveAll approves only the suggestions that have a project", async () => {
    const unassigned = suggestion({ ruleIds: ["rule-none"], projectId: null, projectName: null, color: "graphite", start: at("11:00"), end: at("11:30") });
    seedQuery(dayKey(DAY), dayView({ suggestions: [proposed, unassigned] }));
    expect(runMenuCommand("suggestion.approveAll")).toBe(true);
    await vi.waitFor(() => expect(mocked.suggestionsApprove).toHaveBeenCalledWith(DAY, [proposed.id]));
  });

  it("suggestion.approveAll does not run when no suggestion has a project, but suggestion.dismissAll does", () => {
    const unassigned = suggestion({ projectId: null, projectName: null, color: "graphite" });
    seedQuery(dayKey(DAY), dayView({ suggestions: [unassigned] }));
    expect(runMenuCommand("suggestion.approveAll")).toBe(false);
    expect(runMenuCommand("suggestion.dismissAll")).toBe(true);
  });

  it("suggestion.approve on a focused suggestion without a project opens the editor", () => {
    const unassigned = suggestion({ projectId: null, projectName: null, color: "graphite" });
    seedQuery(dayKey(DAY), dayView({ suggestions: [unassigned] }));
    state().focusSuggestion(unassigned.id);
    expect(runMenuCommand("suggestion.approve")).toBe(true);
    expect(mocked.suggestionApprove).not.toHaveBeenCalled();
    expect(state().editor).toMatchObject({ mode: "create", suggestion: { id: unassigned.id } });
  });

  describe("with a running suggestion", () => {
    const running = suggestion({ running: true, ruleIds: [], start: at("13:00"), end: at("13:30") });

    beforeEach(() => seedQuery(dayKey(DAY), dayView({ suggestions: [proposed, running] })));

    it.each(["suggestion.approve", "suggestion.dismiss"])("%s does not run on the focused running suggestion", (id) => {
      state().focusSuggestion(running.id);
      expect(runMenuCommand(id)).toBe(false);
      expect(mocked.suggestionApprove).not.toHaveBeenCalled();
      expect(mocked.suggestionsDismiss).not.toHaveBeenCalled();
    });

    it("suggestion.edit still opens the editor on it", () => {
      state().focusSuggestion(running.id);
      expect(runMenuCommand("suggestion.edit")).toBe(true);
      expect(state().editor).toMatchObject({ mode: "create", suggestion: { id: running.id } });
    });

    it("suggestion.approveAll and dismissAll leave it out", async () => {
      expect(runMenuCommand("suggestion.approveAll")).toBe(true);
      await vi.waitFor(() => expect(mocked.suggestionsApprove).toHaveBeenCalledWith(DAY, [proposed.id]));
      expect(runMenuCommand("suggestion.dismissAll")).toBe(true);
      await vi.waitFor(() => expect(mocked.suggestionsDismiss).toHaveBeenCalledWith(DAY, [proposed.id]));
    });

    it("suggestion.approveAll and dismissAll do not run when only it is shown", () => {
      seedQuery(dayKey(DAY), dayView({ suggestions: [running] }));
      expect(runMenuCommand("suggestion.approveAll")).toBe(false);
      expect(runMenuCommand("suggestion.dismissAll")).toBe(false);
    });
  });

  it("suggestion.dismissAll dismisses every suggestion of the shown day", async () => {
    expect(runMenuCommand("suggestion.dismissAll")).toBe(true);
    await vi.waitFor(() => expect(mocked.suggestionsDismiss).toHaveBeenCalledWith(DAY, [proposed.id]));
  });

  it.each(["suggestion.approve", "suggestion.edit", "suggestion.dismiss", "suggestion.approveRule"])("%s does not run without a focused suggestion", (id) => {
    expect(runMenuCommand(id)).toBe(false);
  });

  it.each(["suggestion.approveAll", "suggestion.dismissAll"])("%s does not run on a day without suggestions", (id) => {
    seedQuery(dayKey(DAY), dayView());
    expect(runMenuCommand(id)).toBe(false);
  });

  it("shares no default binding with the commands of overlapping scopes", () => {
    expect(conflicts().map((c) => [c.binding, c.a.id, c.b.id])).toEqual([]);
  });
});

describe("welcome.show", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ uiState: { ...EMPTY_UI_STATE, onboardingDone: true } });
    installCommands();
  });

  it("brings the welcome back", () => {
    expect(runCommand("welcome.show")).toBe(true);
    expect(state().uiState.onboardingDone).toBe(false);
  });
});

describe("data.exportBackup", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    installCommands();
  });

  it("writes the backup to the chosen path and confirms with a toast", async () => {
    mocked.chooseSavePath.mockResolvedValue("/tmp/backup.json");
    mocked.backupExport.mockResolvedValue({ bytes: 10, recordings: 3, activities: 40 });
    expect(runCommand("data.exportBackup")).toBe(true);
    await vi.waitFor(() => expect(mocked.backupExport).toHaveBeenCalledWith("/tmp/backup.json"));
    await vi.waitFor(() => expect(state().toasts.map((t) => t.message)).toEqual(["Backup written: 3 recordings, 40 activities"]));
  });

  it("writes nothing when the save panel is cancelled", async () => {
    mocked.chooseSavePath.mockResolvedValue(null);
    runCommand("data.exportBackup");
    await vi.waitFor(() => expect(mocked.chooseSavePath).toHaveBeenCalled());
    expect(mocked.backupExport).not.toHaveBeenCalled();
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
    seedQuery(SETTINGS_KEY, settings());
    seedQuery(dayKey(DAY), dayView());
    installCommands();
  });

  it.each<[string, Parameters<typeof resetStore>[0]]>([
    ["the Day view", { view: "day" }],
    ["the Projects view", { view: "projects" }],
    ["the Reports view", { view: "reports" }],
    ["a Day view with a focused recording", { view: "day", focusedRecording: "r1", day: DAY }],
    ["an open dialog", { view: "day", dialog: { type: "shortcuts", scopes: ["global"] } }],
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
