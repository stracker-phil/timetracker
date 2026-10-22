import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type DraftSpans, type Span } from "../../api";
import { at, dayView, recordingBlock } from "../../test/dayFixtures";
import { recording } from "../../test/recordingFixtures";
import { resetStore, useStore } from "../store";

const mocked = vi.mocked(api, true);
const state = () => useStore.getState();
const toastMessages = () => state().toasts.map((t) => t.message);

const MINUTE_MS = 60_000;
const morning = { start: at("09:00"), end: at("09:30") };
const noon = { start: at("11:00"), end: at("11:45") };
const evening = { start: at("16:00"), end: at("17:00") };

/** Opens a create editor and waits until Rust has answered for its span. */
async function openCreate(span: Span, projectId?: string) {
  state().openEditor({ mode: "create", ...span, ...(projectId ? { projectId } : {}) });
  await vi.waitFor(() => expect(state().draft).not.toBeNull());
}

/** An answer that arrives when `answer` is called. */
function deferred() {
  let answer!: (spans: DraftSpans) => void;
  const promise = new Promise<DraftSpans>((resolve) => (answer = resolve));
  return { promise, answer };
}

describe("editor slice", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.recordingSnap.mockImplementation(async (start, end) => ({ start, end }));
    mocked.recordingsList.mockResolvedValue([]);
    resetStore();
  });

  describe("openEditor", () => {
    it("opens a create editor and fills a draft of its span once Rust has answered for it", async () => {
      state().openEditor({ mode: "create", ...morning });
      expect(state().editor).toEqual({ mode: "create", ...morning });
      expect(state().draft).toBeNull();
      await vi.waitFor(() => expect(state().draft).toEqual({ projectId: null, spans: [morning], totalMs: 30 * MINUTE_MS, note: "", tagId: null }));
      expect(mocked.recordingDraftSpans).toHaveBeenCalledWith([morning]);
      expect(state().editing).toBeNull();
    });

    it("preselects the project the opener names", async () => {
      await openCreate(morning, "p-site");
      expect(state().draft?.projectId).toBe("p-site");
    });

    it("takes the spans and total Rust answers with", async () => {
      mocked.recordingDraftSpans.mockResolvedValueOnce({ spans: [{ start: at("09:00"), end: at("09:35") }], totalMs: 35 * MINUTE_MS });
      await openCreate(morning);
      expect(state().draft).toMatchObject({ spans: [{ start: at("09:00"), end: at("09:35") }], totalMs: 35 * MINUTE_MS });
    });

    it("toasts and shows no draft when Rust cannot answer for the span", async () => {
      mocked.recordingDraftSpans.mockRejectedValueOnce({ kind: "io", message: "no answer", code: null });
      state().openEditor({ mode: "create", ...morning });
      await vi.waitFor(() => expect(toastMessages()).toEqual(["Could not open the recording: no answer"]));
      expect(state().draft).toBeNull();
    });

    describe("for an existing recording", () => {
      const stored = recording({
        id: "r1",
        projectId: "p-site",
        note: "standup",
        tagId: "tag-research",
        spans: [
          { id: "r1-s1", ...morning },
          { id: "r1-s2", ...evening },
          { id: "r1-s3", start: at("18:00"), end: null },
        ],
      });

      it("shows no draft until the recording has loaded and Rust answered, then its closed spans", async () => {
        mocked.recordingsList.mockResolvedValue([stored]);
        resetStore({ dayView: dayView({ recordings: [recordingBlock({ id: "r1", ...morning })] }) });
        state().openEditor({ mode: "edit", id: "r1" });
        expect(state().draft).toBeNull();
        await vi.waitFor(() =>
          expect(state().draft).toEqual({ projectId: "p-site", spans: [morning, evening], totalMs: 90 * MINUTE_MS, note: "standup", tagId: "tag-research" }),
        );
        expect(state().editing).toEqual(stored);
        expect(mocked.recordingsList).toHaveBeenCalledWith(morning.start, morning.end);
        expect(mocked.recordingDraftSpans).toHaveBeenCalledWith([morning, evening]);
      });

      it("finds the recording by the span the opener names when the day does not show it", async () => {
        mocked.recordingsList.mockResolvedValue([stored]);
        state().openEditor({ mode: "edit", id: "r1", at: noon });
        await vi.waitFor(() => expect(state().draft).not.toBeNull());
        expect(mocked.recordingsList).toHaveBeenCalledWith(noon.start, noon.end);
      });

      it("loads nothing when neither the day nor the opener knows where the recording is", () => {
        state().openEditor({ mode: "edit", id: "r1" });
        expect(mocked.recordingsList).not.toHaveBeenCalled();
        expect(state().draft).toBeNull();
      });

      it("toasts and shows no draft when the recording cannot be loaded", async () => {
        mocked.recordingsList.mockRejectedValueOnce({ kind: "db", message: "Database locked", code: null });
        state().openEditor({ mode: "edit", id: "r1", at: noon });
        await vi.waitFor(() => expect(toastMessages()).toEqual(["Could not open the recording: Database locked"]));
        expect(state().draft).toBeNull();
      });

      it("drops the answer when another editor opened meanwhile", async () => {
        let answer!: (rows: (typeof stored)[]) => void;
        mocked.recordingsList.mockReturnValueOnce(new Promise((resolve) => (answer = resolve)));
        state().openEditor({ mode: "edit", id: "r1", at: noon });
        state().openEditor({ mode: "create", ...morning });
        answer([stored]);
        await vi.waitFor(() => expect(state().draft?.spans).toEqual([morning]));
        await new Promise((resolve) => setTimeout(resolve, 10));
        expect(state().editing).toBeNull();
        expect(state().draft?.spans).toEqual([morning]);
      });
    });

    it("declines the overlap question of the editor it replaces", () => {
      const resolve = vi.fn();
      state().openEditor({ mode: "create", ...morning });
      useStore.setState({ editorOverlap: { recordings: [], resolve } });
      state().openEditor({ mode: "create", ...noon });
      expect(resolve).toHaveBeenCalledWith(false);
      expect(state().editorOverlap).toBeNull();
    });
  });

  describe("closeEditor", () => {
    it("drops the editor, its draft and the selection with its snap", async () => {
      await openCreate(morning);
      state().setSelection({ ...morning, blockIds: ["a1"] });
      state().setSelectionSnap(morning);
      state().closeEditor();
      expect([state().editor, state().draft, state().editing, state().selection, state().selectionSnap]).toEqual([null, null, null, null, null]);
    });

    it("declines a pending overlap question", () => {
      const resolve = vi.fn();
      state().openEditor({ mode: "create", ...morning });
      useStore.setState({ editorOverlap: { recordings: [], resolve } });
      state().closeEditor();
      expect(resolve).toHaveBeenCalledWith(false);
      expect(state().editorOverlap).toBeNull();
    });

    it("leaves no draft behind when Rust answers after it closed", async () => {
      const slow = deferred();
      mocked.recordingDraftSpans.mockReturnValueOnce(slow.promise);
      state().openEditor({ mode: "create", ...morning });
      state().closeEditor();
      slow.answer({ spans: [morning], totalMs: 30 * MINUTE_MS });
      await new Promise((resolve) => setTimeout(resolve, 10));
      expect(state().draft).toBeNull();
    });
  });

  describe("editing the draft", () => {
    beforeEach(() => openCreate(noon));

    it("patchDraft changes only the given fields", () => {
      state().patchDraft({ note: "fix", tagId: "tag-research" });
      expect(state().draft).toEqual({ projectId: null, spans: [noon], totalMs: 45 * MINUTE_MS, note: "fix", tagId: "tag-research" });
    });

    it("patchDraft does nothing without an editor", () => {
      state().closeEditor();
      state().patchDraft({ note: "fix" });
      expect(state().draft).toBeNull();
    });

    describe("setDraftSpan", () => {
      it("shows the new times at once and asks Rust for them", async () => {
        const slow = deferred();
        mocked.recordingDraftSpans.mockReturnValueOnce(slow.promise);
        state().setDraftSpan(0, { start: noon.start, end: at("12:00") });
        expect(state().draft?.spans).toEqual([{ start: noon.start, end: at("12:00") }]);
        expect(mocked.recordingDraftSpans).toHaveBeenLastCalledWith([{ start: noon.start, end: at("12:00") }]);
        slow.answer({ spans: [{ start: noon.start, end: at("12:00") }], totalMs: 60 * MINUTE_MS });
        await vi.waitFor(() => expect(state().draft?.totalMs).toBe(60 * MINUTE_MS));
      });

      it("takes the times as Rust answers, e.g. an end rolled past midnight", async () => {
        const rolled = { start: noon.start, end: at("08:00", "2026-03-11") };
        mocked.recordingDraftSpans.mockResolvedValueOnce({ spans: [rolled], totalMs: rolled.end - rolled.start });
        state().setDraftSpan(0, { start: noon.start, end: at("08:00") });
        await vi.waitFor(() => expect(state().draft?.spans).toEqual([rolled]));
        expect(state().draft?.totalMs).toBe(rolled.end - rolled.start);
      });

      it("keeps the latest ask when an older answer arrives after it", async () => {
        const first = deferred();
        mocked.recordingDraftSpans.mockReturnValueOnce(first.promise);
        state().setDraftSpan(0, { start: noon.start, end: at("12:00") });
        state().setDraftSpan(0, { start: noon.start, end: at("12:30") });
        await vi.waitFor(() => expect(state().draft?.totalMs).toBe(90 * MINUTE_MS));
        first.answer({ spans: [{ start: noon.start, end: at("12:00") }], totalMs: 60 * MINUTE_MS });
        await new Promise((resolve) => setTimeout(resolve, 10));
        expect(state().draft?.spans).toEqual([{ start: noon.start, end: at("12:30") }]);
        expect(state().draft?.totalMs).toBe(90 * MINUTE_MS);
      });

      it("toasts when Rust cannot answer and keeps the times shown", async () => {
        mocked.recordingDraftSpans.mockRejectedValueOnce({ kind: "io", message: "no answer", code: null });
        state().setDraftSpan(0, { start: noon.start, end: at("12:00") });
        await vi.waitFor(() => expect(toastMessages()).toEqual(["Could not change the times: no answer"]));
        expect(state().draft?.spans).toEqual([{ start: noon.start, end: at("12:00") }]);
      });
    });

    it("removeDraftSpan asks Rust about the remaining spans", async () => {
      state().addDraftSpan();
      await vi.waitFor(() => expect(state().draft?.spans).toHaveLength(2));
      state().removeDraftSpan(0);
      expect(state().draft?.spans).toHaveLength(1);
      expect(mocked.recordingDraftSpans).toHaveBeenLastCalledWith([{ start: noon.end, end: noon.end + 30 * MINUTE_MS }]);
    });

    it("addDraftSpan takes the spans Rust answers with, a span after the last", async () => {
      state().addDraftSpan();
      expect(mocked.recordingDraftAdd).toHaveBeenCalledWith([noon]);
      await vi.waitFor(() => expect(state().draft?.spans).toEqual([noon, { start: noon.end, end: noon.end + 30 * MINUTE_MS }]));
      expect(state().draft?.totalMs).toBe(75 * MINUTE_MS);
    });

    it("setDraftTotal asks Rust to move the last end and takes its answer", async () => {
      state().setDraftTotal(60 * MINUTE_MS);
      expect(mocked.recordingDraftTotal).toHaveBeenCalledWith([noon], 60 * MINUTE_MS);
      await vi.waitFor(() => expect(state().draft).toMatchObject({ spans: [{ start: noon.start, end: at("12:00") }], totalMs: 60 * MINUTE_MS }));
    });

    it("keeps the fields while the times change", async () => {
      state().patchDraft({ note: "keep", projectId: "p-site" });
      state().setDraftTotal(60 * MINUTE_MS);
      await vi.waitFor(() => expect(state().draft?.totalMs).toBe(60 * MINUTE_MS));
      expect(state().draft).toMatchObject({ note: "keep", projectId: "p-site" });
    });
  });

  describe("recordSelection", () => {
    it("does nothing without a selection", async () => {
      await state().recordSelection();
      expect(mocked.recordingSnap).not.toHaveBeenCalled();
      expect(state().editor).toBeNull();
    });

    it("opens a create editor on the selection as Rust snaps it", async () => {
      mocked.recordingSnap.mockResolvedValueOnce(morning);
      state().setSelection({ start: at("09:02"), end: at("09:28"), blockIds: [] });
      await state().recordSelection();
      expect(mocked.recordingSnap).toHaveBeenCalledWith(at("09:02"), at("09:28"), null);
      expect(state().editor).toEqual({ mode: "create", ...morning });
      await vi.waitFor(() => expect(state().draft?.spans).toEqual([morning]));
    });

    it("hands Rust the end a drag moves", async () => {
      state().setSelection({ start: at("09:02"), end: at("09:28"), blockIds: [], moving: "start" });
      await state().recordSelection();
      expect(mocked.recordingSnap).toHaveBeenCalledWith(at("09:02"), at("09:28"), "start");
    });

    it("moves an open create editor to the selection and keeps its fields", async () => {
      await openCreate(morning);
      state().patchDraft({ note: "keep me", projectId: "p-site" });
      state().setSelection({ ...noon, blockIds: [] });
      await state().recordSelection();
      expect(state().editor).toEqual({ mode: "create", ...noon });
      await vi.waitFor(() => expect(state().draft).toEqual({ projectId: "p-site", spans: [noon], totalMs: 45 * MINUTE_MS, note: "keep me", tagId: null }));
    });

    it("adds the selection as another span of the open editor with add, after the others", async () => {
      await openCreate(noon);
      state().setSelection({ ...morning, blockIds: [] });
      await state().recordSelection(true);
      await vi.waitFor(() => expect(state().draft?.spans).toEqual([noon, morning]));
      expect(mocked.recordingDraftSpans).toHaveBeenLastCalledWith([noon, morning]);
      expect(state().editor).toEqual({ mode: "create", ...noon });
    });

    it("opens a new editor with add when none is open", async () => {
      state().setSelection({ ...morning, blockIds: [] });
      await state().recordSelection(true);
      expect(state().editor).toEqual({ mode: "create", ...morning });
    });

    it("toasts and opens no editor when Rust cannot snap", async () => {
      mocked.recordingSnap.mockRejectedValueOnce({ kind: "io", message: "no snap", code: null });
      state().setSelection({ ...morning, blockIds: [] });
      await state().recordSelection();
      expect(toastMessages()).toEqual(["Could not open the editor: no snap"]);
      expect(state().editor).toBeNull();
    });
  });

  describe("answerOverlap", () => {
    it.each([true, false])("passes %s to the waiting save and clears the question", (replace) => {
      const resolve = vi.fn();
      state().openEditor({ mode: "create", ...morning });
      useStore.setState({ editorOverlap: { recordings: [recording()], resolve } });
      state().answerOverlap(replace);
      expect(resolve).toHaveBeenCalledWith(replace);
      expect(state().editorOverlap).toBeNull();
    });

    it("does nothing when no question is open", () => {
      expect(() => state().answerOverlap(true)).not.toThrow();
      expect(state().editorOverlap).toBeNull();
    });
  });
});
