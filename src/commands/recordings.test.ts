import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { resetStore, useStore } from "../state/store";
import { at } from "../test/dayFixtures";
import { getCommand, installCommands, runCommand, runMenuCommand } from "./index";

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

  it.each(["edit.undo", "edit.redo", "recording.create", "recording.edit", "recording.delete", "recording.open", "recording.nudgeEarlier", "recording.nudgeLater", "recording.extend", "recording.shrink", "day.clearFocus"])(
    "registers %s",
    (id) => {
      expect(getCommand(id)).toBeDefined();
    },
  );

  it.each([
    ["edit.undo", "Mod+Z"],
    ["edit.redo", "Mod+Shift+Z"],
    ["recording.delete", "Backspace"],
    ["recording.open", "Enter"],
    ["day.details", "Space"],
    ["day.clearFocus", "Escape"],
  ])("binds %s to %s", (id, binding) => {
    expect(getCommand(id)?.defaultBinding).toBe(binding);
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

    it.each(["edit.undo", "edit.redo"])("%s leaves ⌘Z to a focused text field", (id) => {
      const input = document.body.appendChild(document.createElement("input"));
      input.focus();
      expect(runCommand(id)).toBe(false);
      expect(mocked.undo).not.toHaveBeenCalled();
      expect(mocked.redo).not.toHaveBeenCalled();
    });
  });

  describe("recording.open", () => {
    it("is unavailable without a selection or a focused recording", () => {
      expect(runCommand("recording.open")).toBe(false);
      expect(runCommand("recording.delete")).toBe(false);
    });

    it("opens the editor for the snapped selection", () => {
      state().setSelection({ start: at("09:02"), end: at("10:28"), blockIds: [] });
      state().setSelectionSnap({ start: at("09:00"), end: at("10:30") });
      expect(runCommand("recording.open")).toBe(true);
      expect(state().editor).toEqual({ mode: "create", start: at("09:00"), end: at("10:30") });
    });

    it("edits the focused recording rather than the selection", () => {
      state().setSelection({ start: at("09:00"), end: at("10:00"), blockIds: [] });
      state().focusRecording("r1");
      runCommand("recording.open");
      expect(state().editor).toEqual({ mode: "edit", id: "r1" });
    });

    it("creates from the selection even while a recording is focused", () => {
      state().setSelection({ start: at("09:00"), end: at("10:00"), blockIds: [] });
      state().focusRecording("r1");
      runCommand("recording.create");
      expect(state().editor).toMatchObject({ mode: "create" });
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
