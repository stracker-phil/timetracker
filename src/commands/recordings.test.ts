import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import userEvent from "@testing-library/user-event";

import { api } from "../api";
import { resetStore, useStore } from "../state/store";
import { at, DAY, dayView, recordingBlock, timesheetCard } from "../test/dayFixtures";
import { suggestion } from "../test/ruleFixtures";
import { settings } from "../test/settings";
import { conflicts, getCommand, handleKeydown, installCommands, runCommand, runMenuCommand } from "./index";

const mocked = vi.mocked(api, true);
const state = () => useStore.getState();

describe("recording commands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ view: "day" });
    installCommands();
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  it.each([
    "edit.undo",
    "edit.redo",
    "recording.create",
    "recording.edit",
    "recording.delete",
    "recording.open",
    "recording.nudgeEarlier",
    "recording.nudgeLater",
    "recording.extend",
    "recording.shrink",
    "recording.next",
    "recording.prev",
    "recording.moveToPrevDay",
    "recording.moveToNextDay",
    "recording.copyToPrevDay",
    "recording.copyToNextDay",
    "day.clearFocus",
    "day.selectBlock",
    "day.extendNext",
    "day.extendPrev",
  ])("registers %s", (id) => {
    expect(getCommand(id)).toBeDefined();
  });

  it.each([
    ["edit.undo", "Mod+Z"],
    ["edit.redo", "Mod+Shift+Z"],
    ["recording.delete", "Backspace"],
    ["recording.open", "Enter"],
    ["day.details", "Space"],
    ["day.clearFocus", "Escape"],
    ["day.selectBlock", "V"],
    ["day.extendNext", "Shift+J"],
    ["day.extendPrev", "Shift+K"],
    ["recording.next", "]"],
    ["recording.prev", "["],
    ["recording.moveToPrevDay", "Alt+ArrowLeft"],
    ["recording.moveToNextDay", "Alt+ArrowRight"],
    ["recording.copyToPrevDay", "Alt+Shift+ArrowLeft"],
    ["recording.copyToNextDay", "Alt+Shift+ArrowRight"],
  ])("binds %s to %s", (id, binding) => {
    expect(getCommand(id)?.defaultBinding).toBe(binding);
  });

  it("has no default binding that two commands of overlapping scopes share", () => {
    expect(conflicts().map((c) => [c.binding, c.a.id, c.b.id])).toEqual([]);
  });

  describe("availability", () => {
    it.each(["day.selectBlock", "recording.moveToPrevDay", "recording.moveToNextDay", "recording.copyToPrevDay", "recording.copyToNextDay"])("%s needs a focused block or recording", (id) => {
      expect(runCommand(id)).toBe(false);
    });

    it.each(["day.selectBlock", "day.extendNext", "day.extendPrev", "recording.next", "recording.prev"])("%s is unavailable in the List mode", (id) => {
      state().focusBlock("a1");
      state().setDayMode("list");
      expect(runCommand(id)).toBe(false);
    });

    it.each(["day.extendNext", "day.extendPrev", "recording.next", "recording.prev"])("%s is available on the timeline without a focus", (id) => {
      expect(runCommand(id)).toBe(true);
    });
  });

  describe("stepping recordings with ] and [", () => {
    beforeEach(() => {
      resetStore({
        view: "day",
        day: DAY,
        dayView: dayView({ timesheet: [timesheetCard({ id: "r1" }), timesheetCard({ id: "r2", start: at("11:00"), end: at("12:00") })] }),
      });
    });

    async function press(keys: string) {
      window.addEventListener("keydown", handleKeydown, true);
      try {
        await userEvent.keyboard(keys);
      } finally {
        window.removeEventListener("keydown", handleKeydown, true);
      }
    }

    it("] focuses the first recording of the day, then the next one", async () => {
      await press("]");
      expect(state().focusedRecording).toBe("r1");
      await press("]");
      expect(state().focusedRecording).toBe("r2");
    });

    it("[ focuses the last recording of the day, then the previous one", async () => {
      await press("[[");
      expect(state().focusedRecording).toBe("r2");
      await press("[[");
      expect(state().focusedRecording).toBe("r1");
    });

    describe("with suggestions between the recordings", () => {
      const between = suggestion({ start: at("10:00"), end: at("10:30"), nextCard: "r2" });
      const last = suggestion({ ruleId: "rule-2", start: at("13:00"), end: at("13:30"), nextCard: null });

      beforeEach(() => {
        resetStore({
          view: "day",
          day: DAY,
          dayView: dayView({ timesheet: [timesheetCard({ id: "r1" }), timesheetCard({ id: "r2", start: at("11:00"), end: at("12:00") })], suggestions: [between, last] }),
        });
      });

      it("] visits recordings and suggestions in the order of the timesheet", async () => {
        const visited: (string | null)[] = [];
        for (let i = 0; i < 4; i++) {
          await press("]");
          visited.push(state().focusedRecording ?? state().focusedSuggestion);
        }
        expect(visited).toEqual(["r1", between.id, "r2", last.id]);
      });

      it("[ walks back from the last suggestion", async () => {
        await press("[[");
        expect(state().focusedSuggestion).toBe(last.id);
        await press("[[");
        expect(state().focusedRecording).toBe("r2");
      });

      it("leaves only one of the two focused", async () => {
        await press("]]");
        expect(state().focusedSuggestion).toBe(between.id);
        expect(state().focusedRecording).toBeNull();
        await press("]");
        expect(state().focusedSuggestion).toBeNull();
        expect(state().focusedRecording).toBe("r2");
      });
    });

    it("N steps the day and leaves the focused recording alone", async () => {
      state().focusRecording("r1");
      await press("n");
      expect(state().focusedRecording).toBe("r1");
      expect(state().day).toBe("2026-03-11");
    });
  });

  describe("a recording with several spans on the day", () => {
    const morning = recordingBlock({ id: "r1", spanId: "r1-s1", spanCount: 2, start: at("09:00"), end: at("09:30") });
    const evening = recordingBlock({ id: "r1", spanId: "r1-s2", spanCount: 2, start: at("13:00"), end: at("13:30") });
    const between = recordingBlock({ id: "r2", start: at("11:00"), end: at("12:00") });

    beforeEach(() => {
      const cards = [timesheetCard({ id: "r1", start: at("09:00"), end: at("13:30") }), timesheetCard({ id: "r2", start: at("11:00"), end: at("12:00") })];
      resetStore({ view: "day", day: DAY, settings: settings(), dayView: dayView({ recordings: [morning, between, evening], timesheet: cards }) });
    });

    async function press(keys: string) {
      window.addEventListener("keydown", handleKeydown, true);
      try {
        await userEvent.keyboard(keys);
      } finally {
        window.removeEventListener("keydown", handleKeydown, true);
      }
    }

    it("] visits it once and stops at the last recording", async () => {
      await press("]");
      await press("]");
      await press("]");
      expect(state().focusedRecording).toBe("r2");
    });

    it("[ starts from the last recording, not from its last span", async () => {
      await press("[[");
      expect(state().focusedRecording).toBe("r2");
      await press("[[");
      expect(state().focusedRecording).toBe("r1");
    });

    it.each([
      ["recording.nudgeLater", 1, 1],
      ["recording.nudgeEarlier", -1, -1],
      ["recording.extend", 0, 1],
      ["recording.shrink", 0, -1],
    ] as const)("%s nudges its first span of the day by %i and %i steps", async (id, startSteps, endSteps) => {
      state().focusRecording("r1");
      expect(runCommand(id)).toBe(true);
      await vi.waitFor(() => expect(mocked.recordingNudge).toHaveBeenCalledWith("r1", "r1-s1", startSteps, endSteps, "cancel"));
      expect(mocked.recordingUpdateSpan).not.toHaveBeenCalled();
    });

    it.each([
      ["recording.moveToNextDay", "recordingMove", "2026-03-11"],
      ["recording.copyToPrevDay", "recordingCopy", "2026-03-09"],
    ] as const)("%s acts on the whole recording", async (id, call, day) => {
      state().focusRecording("r1");
      expect(runCommand(id)).toBe(true);
      await vi.waitFor(() => expect(mocked[call]).toHaveBeenCalledWith("r1", null, day, "cancel"));
    });
  });

  describe("edit.undo and edit.redo", () => {
    it("undo calls the api", () => {
      expect(runCommand("edit.undo")).toBe(true);
      expect(mocked.undo).toHaveBeenCalled();
    });

    it("redo calls the api", () => {
      expect(runCommand("edit.redo")).toBe(true);
      expect(mocked.redo).toHaveBeenCalled();
    });

    it("run in every view", () => {
      resetStore({ view: "projects" });
      expect(runMenuCommand("edit.undo")).toBe(true);
    });

    it.each([
      ["edit.undo", "undo"],
      ["edit.redo", "redo"],
    ])("%s undoes the typing of a focused text field instead of a recording change", (id, native) => {
      const execCommand = vi.fn<(command: string) => boolean>(() => true);
      document.execCommand = execCommand;
      try {
        document.body.appendChild(document.createElement("input")).focus();
        expect(runCommand(id)).toBe(true);
        expect(execCommand).toHaveBeenCalledWith(native);
      } finally {
        Reflect.deleteProperty(document, "execCommand");
      }
      expect(mocked.undo).not.toHaveBeenCalled();
      expect(mocked.redo).not.toHaveBeenCalled();
    });
  });

  describe("recording.open", () => {
    it("is unavailable without a selection or a focused recording", () => {
      expect(runCommand("recording.open")).toBe(false);
      expect(runCommand("recording.delete")).toBe(false);
    });

    it("opens the editor for the selection as Rust snaps it, asking api.recordingSnap", async () => {
      mocked.recordingSnap.mockResolvedValueOnce({ start: at("09:00"), end: at("10:30") });
      state().setSelection({ start: at("09:02"), end: at("10:28"), blockIds: [] });
      expect(runCommand("recording.open")).toBe(true);
      await vi.waitFor(() => expect(state().editor).toEqual({ mode: "create", start: at("09:00"), end: at("10:30") }));
      expect(mocked.recordingSnap).toHaveBeenCalledWith(at("09:02"), at("10:28"), null);
      await vi.waitFor(() => expect(state().draft?.spans).toEqual([{ start: at("09:00"), end: at("10:30") }]));
    });

    it("passes the end a drag moves to Rust's snap", async () => {
      state().setSelection({ start: at("09:02"), end: at("10:28"), blockIds: [], moving: "start" });
      runCommand("recording.open");
      await vi.waitFor(() => expect(state().editor).not.toBeNull());
      expect(mocked.recordingSnap).toHaveBeenCalledWith(at("09:02"), at("10:28"), "start");
    });

    it("edits the focused recording rather than the selection", () => {
      state().setSelection({ start: at("09:00"), end: at("10:00"), blockIds: [] });
      state().focusRecording("r1");
      runCommand("recording.open");
      expect(state().editor).toEqual({ mode: "edit", id: "r1" });
      expect(mocked.recordingSnap).not.toHaveBeenCalled();
    });

    it("creates from the selection even while a recording is focused", async () => {
      state().setSelection({ start: at("09:00"), end: at("10:00"), blockIds: [] });
      state().focusRecording("r1");
      runCommand("recording.create");
      await vi.waitFor(() => expect(state().editor).toMatchObject({ mode: "create" }));
    });

    it("opens no editor and toasts when Rust cannot snap the selection", async () => {
      mocked.recordingSnap.mockRejectedValueOnce({ kind: "io", message: "no snap", code: null });
      state().setSelection({ start: at("09:00"), end: at("10:00"), blockIds: [] });
      runCommand("recording.open");
      await vi.waitFor(() => expect(state().toasts.map((t) => t.message)).toEqual(["Could not open the editor: no snap"]));
      expect(state().editor).toBeNull();
    });
  });

  describe("recording.edit", () => {
    it("opens the editor on the focused recording", () => {
      state().focusRecording("r1");
      expect(runCommand("recording.edit")).toBe(true);
      expect(state().editor).toEqual({ mode: "edit", id: "r1" });
    });
  });

  describe("day.clearFocus", () => {
    it("clears the block focus, the selection and the focused recording", () => {
      state().focusBlock("a1");
      state().setSelection({ start: 1, end: 2, blockIds: [] });
      state().focusRecording("r1");
      expect(runCommand("day.clearFocus")).toBe(true);
      expect([state().focusedBlock, state().selection, state().focusedRecording]).toEqual([null, null, null]);
    });
  });

  describe("while the editor is open", () => {
    beforeEach(() => {
      state().focusRecording("r1");
      state().openEditor({ mode: "edit", id: "r1" });
    });

    it.each(["recording.delete", "edit.undo", "nav.today"])("%s does not run from a menu or key", (id) => {
      expect(runMenuCommand(id)).toBe(false);
      expect(mocked.recordingDelete).not.toHaveBeenCalled();
      expect(mocked.undo).not.toHaveBeenCalled();
    });
  });
});
