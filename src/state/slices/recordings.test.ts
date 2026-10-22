import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { at, block, dayView, lane } from "../../test/dayFixtures";
import { projectItem, projectsStore } from "../../test/projectFixtures";
import { recording } from "../../test/recordingFixtures";
import { resetStore, useStore } from "../store";

const mocked = vi.mocked(api, true);
const state = () => useStore.getState();
const toastMessages = () => state().toasts.map((t) => t.message);

const input = { projectId: "p-site", spans: [{ start: at("09:00"), end: at("09:45") }], note: "", tagId: null };
const inTheWay = recording({ id: "r9", start: at("09:15"), end: at("09:30") });
const overlapError = { kind: "overlap", message: "Overlaps", code: null, details: [inTheWay] };

/** Waits for the overlap dialog, which `createRecording` is blocked on. */
async function overlapDialog() {
  await vi.waitFor(() => expect(state().dialog?.type).toBe("overlap"));
  const dialog = state().dialog;
  if (dialog?.type !== "overlap") throw new Error("no overlap dialog");
  return dialog;
}

describe("recordings slice", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ day: "2026-03-10", ...projectsStore({ projects: [projectItem({ project: { id: "p-site", name: "Website" } })] }) });
  });

  describe("selection", () => {
    const docs = block({ id: "a1", start: at("09:00"), end: at("09:30") });
    const todo = block({ id: "a2", start: at("10:00"), end: at("10:30") });
    beforeEach(() => resetStore({ dayView: dayView({ lanes: [lane({ blocks: [docs, todo] })] }) }));

    it("starts a selection from the block's span", () => {
      state().extendSelection("a2");
      expect(state().selection).toEqual({ start: todo.start, end: todo.end, blockIds: ["a2"] });
    });

    it("grows to the earliest start and the latest end", () => {
      state().extendSelection("a2");
      state().extendSelection("a1");
      expect(state().selection).toEqual({ start: docs.start, end: todo.end, blockIds: ["a2", "a1"] });
    });

    it("ignores a block that is not in the view", () => {
      state().extendSelection("missing");
      expect(state().selection).toBeNull();
    });

    it("shows the span Rust snapped a block selection to", async () => {
      mocked.recordingSnap.mockResolvedValueOnce({ start: at("08:55"), end: at("09:35") });
      state().extendSelection("a1");
      await vi.waitFor(() => expect(state().selectionSnap).toEqual({ start: at("08:55"), end: at("09:35") }));
    });

    it("toasts when Rust cannot snap a block selection and keeps the selection", async () => {
      mocked.recordingSnap.mockRejectedValueOnce({ kind: "io", message: "no snap", code: null });
      state().extendSelection("a1");
      await vi.waitFor(() => expect(toastMessages()).toEqual(["Could not select the time: no snap"]));
      expect(state().selection).toMatchObject({ start: docs.start, end: docs.end });
    });

    it("forgets the snapped span when the selection is cleared", () => {
      state().setSelection({ start: 1, end: 2, blockIds: [] });
      expect(state().selectionSnap).toEqual({ start: 1, end: 2 });
      state().setSelection(null);
      expect(state().selectionSnap).toBeNull();
    });
  });

  describe("hoverRecording", () => {
    it("marks the recording as hovered and clears it with null", () => {
      expect(state().hoveredRecording).toBeNull();
      state().hoverRecording("r1");
      expect(state().hoveredRecording).toBe("r1");
      state().hoverRecording(null);
      expect(state().hoveredRecording).toBeNull();
    });

    it("moves the mark to another recording", () => {
      state().hoverRecording("r1");
      state().hoverRecording("r2");
      expect(state().hoveredRecording).toBe("r2");
    });

    it("leaves the focused recording alone", () => {
      state().focusRecording("r1");
      state().hoverRecording("r2");
      expect(state().focusedRecording).toBe("r1");
      state().hoverRecording(null);
      expect(state().focusedRecording).toBe("r1");
    });
  });

  describe("createRecording", () => {
    it("saves with overlap cancel, toasts with Undo and clears the selection", async () => {
      state().setSelection({ ...input.spans[0], blockIds: [] });
      await state().createRecording(input);
      expect(mocked.recordingCreate).toHaveBeenCalledWith(input, "cancel");
      expect(toastMessages()).toContain("Recorded 45m for Website");
      expect(state().toasts[0].action?.label).toBe("Undo");
      expect(state().selection).toBeNull();
    });

    it("runs the Undo action of the toast through the api", async () => {
      mocked.undo.mockResolvedValueOnce("Record 45m for Website");
      await state().createRecording(input);
      state().toasts[0].action?.run();
      await vi.waitFor(() => expect(toastMessages()).toContain("Undid “Record 45m for Website”"));
    });

    it("asks before replacing on an overlap and retries with replace on Replace", async () => {
      mocked.recordingCreate.mockRejectedValueOnce(overlapError);
      const saving = state().createRecording(input);
      (await overlapDialog()).onReplace();
      await expect(saving).resolves.toMatchObject({ id: "recording-new" });
      expect(mocked.recordingCreate).toHaveBeenLastCalledWith(input, "replace");
    });

    it("saves nothing and resolves null on an overlap that is cancelled", async () => {
      mocked.recordingCreate.mockRejectedValueOnce(overlapError);
      const saving = state().createRecording(input);
      (await overlapDialog()).onCancel();
      await expect(saving).resolves.toBeNull();
      expect(mocked.recordingCreate).not.toHaveBeenCalledWith(input, "replace");
      expect(toastMessages()).toEqual([]);
    });

    it("rejects with the error when a form field is to blame", async () => {
      const error = { kind: "invalid_input", message: "x", code: null, field: { path: "note", message: "Too long" } };
      mocked.recordingCreate.mockRejectedValueOnce(error);
      await expect(state().createRecording(input)).rejects.toEqual(error);
      expect(toastMessages()).toEqual([]);
    });

    it("toasts any other error and resolves null", async () => {
      mocked.recordingCreate.mockRejectedValueOnce({ kind: "db", message: "Database locked", code: null });
      await expect(state().createRecording(input)).resolves.toBeNull();
      expect(toastMessages()).toEqual(["Could not save the recording: Database locked"]);
    });

    it("ignores a second save while the first is in flight", async () => {
      let finish!: (r: ReturnType<typeof recording>) => void;
      mocked.recordingCreate.mockReturnValueOnce(new Promise((r) => (finish = r)));
      const first = state().createRecording(input);
      await expect(state().createRecording(input)).resolves.toBeNull();
      finish(recording());
      await first;
      expect(mocked.recordingCreate).toHaveBeenCalledTimes(1);
    });
  });

  describe("an overlap while the editor is open", () => {
    beforeEach(() => state().openEditor({ mode: "create", start: input.spans[0].start, end: input.spans[0].end }));

    it("is asked inside the editor, not in a dialog", async () => {
      mocked.recordingCreate.mockRejectedValueOnce(overlapError);
      const saving = state().createRecording(input);
      await vi.waitFor(() => expect(state().editorOverlap?.recordings).toEqual([inTheWay]));
      expect(state().dialog).toBeNull();
      state().answerOverlap(false);
      await saving;
    });

    it("retries with replace on answerOverlap(true) and clears the question", async () => {
      mocked.recordingCreate.mockRejectedValueOnce(overlapError);
      const saving = state().createRecording(input);
      await vi.waitFor(() => expect(state().editorOverlap).not.toBeNull());
      state().answerOverlap(true);
      await expect(saving).resolves.toMatchObject({ id: "recording-new" });
      expect(mocked.recordingCreate).toHaveBeenLastCalledWith(input, "replace");
      expect(state().editorOverlap).toBeNull();
    });

    it("saves nothing on answerOverlap(false) and keeps the editor open", async () => {
      mocked.recordingCreate.mockRejectedValueOnce(overlapError);
      const saving = state().createRecording(input);
      await vi.waitFor(() => expect(state().editorOverlap).not.toBeNull());
      state().answerOverlap(false);
      await expect(saving).resolves.toBeNull();
      expect(mocked.recordingCreate).not.toHaveBeenCalledWith(input, "replace");
      expect(state().editor).not.toBeNull();
      expect(state().editorOverlap).toBeNull();
    });

    it("is declined when the editor closes meanwhile", async () => {
      mocked.recordingUpdate.mockRejectedValueOnce(overlapError);
      const saving = state().updateRecording("r1", { note: "x" });
      await vi.waitFor(() => expect(state().editorOverlap).not.toBeNull());
      state().closeEditor();
      await expect(saving).resolves.toBeNull();
      expect(mocked.recordingUpdate).not.toHaveBeenCalledWith("r1", { note: "x" }, "replace");
    });
  });

  describe("updateRecording and deleteRecording", () => {
    it("sends the patch with overlap cancel and toasts", async () => {
      const patch = { spans: [{ start: at("09:00"), end: at("10:00") }] };
      await state().updateRecording("r1", patch);
      expect(mocked.recordingUpdate).toHaveBeenCalledWith("r1", patch, "cancel");
      expect(toastMessages()).toContain("Recording changed");
    });

    it("deletes, toasts with Undo and drops the focus on it", async () => {
      state().focusRecording("r1");
      await state().deleteRecording("r1");
      expect(mocked.recordingDelete).toHaveBeenCalledWith("r1");
      expect(toastMessages()).toContain("Recording deleted");
      expect(state().focusedRecording).toBeNull();
    });

    it("keeps the focus and toasts the error when deleting fails", async () => {
      mocked.recordingDelete.mockRejectedValueOnce({ kind: "not_found", message: "Gone", code: null });
      state().focusRecording("r1");
      await state().deleteRecording("r1");
      expect(toastMessages()).toEqual(["Could not delete the recording: Gone"]);
      expect(state().focusedRecording).toBe("r1");
    });
  });

  describe("updateSpan", () => {
    const span = { start: at("09:00"), end: at("10:00") };

    it("sends the span with overlap cancel and toasts with Undo", async () => {
      await state().updateSpan("r1", "r1-s2", span);
      expect(mocked.recordingUpdateSpan).toHaveBeenCalledWith("r1", "r1-s2", span, null, "cancel");
      expect(toastMessages()).toEqual(["Recording changed"]);
      expect(state().toasts[0].action?.label).toBe("Undo");
    });

    it.each(["start", "end"] as const)("forwards the dragged edge %s for Rust to snap", async (moving) => {
      await state().updateSpan("r1", "r1-s2", span, moving);
      expect(mocked.recordingUpdateSpan).toHaveBeenCalledWith("r1", "r1-s2", span, moving, "cancel");
    });

    it("keeps the moving edge when it retries with replace", async () => {
      mocked.recordingUpdateSpan.mockRejectedValueOnce(overlapError);
      const writing = state().updateSpan("r1", "r1-s2", span, "end");
      (await overlapDialog()).onReplace();
      await writing;
      expect(mocked.recordingUpdateSpan).toHaveBeenLastCalledWith("r1", "r1-s2", span, "end", "replace");
    });

    it("asks before replacing on an overlap and retries with replace on Replace", async () => {
      mocked.recordingUpdateSpan.mockRejectedValueOnce(overlapError);
      const writing = state().updateSpan("r1", "r1-s2", span);
      (await overlapDialog()).onReplace();
      await writing;
      expect(mocked.recordingUpdateSpan).toHaveBeenLastCalledWith("r1", "r1-s2", span, null, "replace");
      expect(toastMessages()).toEqual(["Recording changed"]);
    });

    it("changes nothing and says nothing when the overlap is cancelled", async () => {
      mocked.recordingUpdateSpan.mockRejectedValueOnce(overlapError);
      const writing = state().updateSpan("r1", "r1-s2", span);
      (await overlapDialog()).onCancel();
      await expect(writing).resolves.toBeNull();
      expect(toastMessages()).toEqual([]);
    });

    it("toasts any other error", async () => {
      mocked.recordingUpdateSpan.mockRejectedValueOnce({ kind: "db", message: "Database locked", code: null });
      await state().updateSpan("r1", "r1-s2", span);
      expect(toastMessages()).toEqual(["Could not change the recording: Database locked"]);
    });
  });

  describe("nudgeSpan", () => {
    it("sends the step counts with overlap cancel and toasts with Undo", async () => {
      await state().nudgeSpan("r1", "r1-s2", 1, 1);
      expect(mocked.recordingNudge).toHaveBeenCalledWith("r1", "r1-s2", 1, 1, "cancel");
      expect(toastMessages()).toEqual(["Recording changed"]);
      expect(state().toasts[0].action?.label).toBe("Undo");
    });

    it("asks before replacing on an overlap and retries with replace on Replace", async () => {
      mocked.recordingNudge.mockRejectedValueOnce(overlapError);
      const writing = state().nudgeSpan("r1", "r1-s2", 0, 1);
      (await overlapDialog()).onReplace();
      await writing;
      expect(mocked.recordingNudge).toHaveBeenLastCalledWith("r1", "r1-s2", 0, 1, "replace");
      expect(toastMessages()).toEqual(["Recording changed"]);
    });

    it("changes nothing and says nothing when the overlap is cancelled", async () => {
      mocked.recordingNudge.mockRejectedValueOnce(overlapError);
      const writing = state().nudgeSpan("r1", "r1-s2", 0, 1);
      (await overlapDialog()).onCancel();
      await expect(writing).resolves.toBeNull();
      expect(toastMessages()).toEqual([]);
    });

    it("toasts any other error", async () => {
      mocked.recordingNudge.mockRejectedValueOnce({ kind: "db", message: "Database locked", code: null });
      await state().nudgeSpan("r1", "r1-s2", -1, -1);
      expect(toastMessages()).toEqual(["Could not change the recording: Database locked"]);
    });
  });

  describe.each([
    { name: "moveRecording", call: "recordingMove", done: "Recording moved", failed: "Could not move the recording" },
    { name: "copyRecording", call: "recordingCopy", done: "Recording copied", failed: "Could not copy the recording" },
  ] as const)("$name", ({ name, call, done, failed }) => {
    it("sends the day with overlap cancel and toasts with Undo", async () => {
      await state()[name]("r1", null, "2026-03-12");
      expect(mocked[call]).toHaveBeenCalledWith("r1", null, "2026-03-12", "cancel");
      expect(toastMessages()).toEqual([done]);
      expect(state().toasts[0].action?.label).toBe("Undo");
    });

    it("sends the source day when one day's time is meant", async () => {
      await state()[name]("r1", "2026-03-10", "2026-03-12");
      expect(mocked[call]).toHaveBeenCalledWith("r1", "2026-03-10", "2026-03-12", "cancel");
    });

    it("asks before replacing on an overlap and retries with replace on Replace", async () => {
      mocked[call].mockRejectedValueOnce(overlapError);
      const writing = state()[name]("r1", null, "2026-03-12");
      (await overlapDialog()).onReplace();
      await writing;
      expect(mocked[call]).toHaveBeenLastCalledWith("r1", null, "2026-03-12", "replace");
      expect(toastMessages()).toEqual([done]);
    });

    it("changes nothing and says nothing when the overlap is cancelled", async () => {
      mocked[call].mockRejectedValueOnce(overlapError);
      const writing = state()[name]("r1", null, "2026-03-12");
      (await overlapDialog()).onCancel();
      await writing;
      expect(mocked[call]).not.toHaveBeenCalledWith("r1", null, "2026-03-12", "replace");
      expect(toastMessages()).toEqual([]);
    });

    it("toasts any other error", async () => {
      mocked[call].mockRejectedValueOnce({ kind: "db", message: "Database locked", code: null });
      await state()[name]("r1", null, "2026-03-12");
      expect(toastMessages()).toEqual([`${failed}: Database locked`]);
    });
  });

  describe("undo and redo", () => {
    it("names what was undone and redone", async () => {
      mocked.undo.mockResolvedValueOnce("Move recording");
      mocked.redo.mockResolvedValueOnce("Move recording");
      await state().undo();
      await state().redo();
      expect(toastMessages()).toEqual(["Undid “Move recording”", "Redid “Move recording”"]);
    });

    it("says nothing when there is nothing to undo", async () => {
      await state().undo();
      expect(toastMessages()).toEqual([]);
    });

    it("toasts the error of a failed undo", async () => {
      mocked.undo.mockRejectedValueOnce({ kind: "db", message: "Database locked", code: null });
      await state().undo();
      expect(toastMessages()).toEqual(["Could not undo: Database locked"]);
    });

    it.each([
      ["undo", "Could not undo: Overlaps a newer recording"],
      ["redo", "Could not redo: Overlaps a newer recording"],
    ] as const)("toasts an overlap on %s instead of asking to replace", async (name, message) => {
      mocked[name].mockRejectedValueOnce({ kind: "overlap", message: "Overlaps a newer recording", code: null, details: [inTheWay] });
      await state()[name]();
      expect(toastMessages()).toEqual([message]);
      expect(state().dialog).toBeNull();
    });
  });

  describe("loadUndoStatus", () => {
    it("stores the labels", async () => {
      mocked.undoStatus.mockResolvedValueOnce({ undoLabel: "Move recording", redoLabel: null });
      await state().loadUndoStatus();
      expect(state().undoStatus).toEqual({ undoLabel: "Move recording", redoLabel: null });
    });

    it("falls back to nothing to undo when the status cannot be read", async () => {
      mocked.undoStatus.mockResolvedValueOnce({ undoLabel: "Move recording", redoLabel: null });
      await state().loadUndoStatus();
      mocked.undoStatus.mockRejectedValueOnce({ kind: "io", message: "x", code: null });
      await state().loadUndoStatus();
      expect(state().undoStatus).toEqual({ undoLabel: null, redoLabel: null });
    });
  });
});
