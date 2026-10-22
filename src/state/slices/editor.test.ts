import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type DraftSpans, type Span } from "../../api";
import { at, block, dayView, lane, recordingBlock } from "../../test/dayFixtures";
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
        const done = state().setDraftSpan(0, { start: noon.start, end: at("12:00") });
        expect(state().draft?.spans).toEqual([{ start: noon.start, end: at("12:00") }]);
        expect(mocked.recordingDraftSpans).toHaveBeenLastCalledWith([{ start: noon.start, end: at("12:00") }]);
        slow.answer({ spans: [{ start: noon.start, end: at("12:00") }], totalMs: 60 * MINUTE_MS });
        await done;
        expect(state().draft?.totalMs).toBe(60 * MINUTE_MS);
      });

      it("resolves only once Rust has answered", async () => {
        const slow = deferred();
        mocked.recordingDraftSpans.mockReturnValueOnce(slow.promise);
        let resolved = false;
        const done = state()
          .setDraftSpan(0, { start: noon.start, end: at("12:00") })
          .then(() => void (resolved = true));
        await new Promise((resolve) => setTimeout(resolve, 10));
        expect(resolved).toBe(false);
        slow.answer({ spans: [{ start: noon.start, end: at("12:00") }], totalMs: 60 * MINUTE_MS });
        await done;
        expect(resolved).toBe(true);
        expect(state().draft?.totalMs).toBe(60 * MINUTE_MS);
      });

      it("takes the times as Rust answers, e.g. an end rolled past midnight", async () => {
        const rolled = { start: noon.start, end: at("08:00", "2026-03-11") };
        mocked.recordingDraftSpans.mockResolvedValueOnce({ spans: [rolled], totalMs: rolled.end - rolled.start });
        await state().setDraftSpan(0, { start: noon.start, end: at("08:00") });
        expect(state().draft?.spans).toEqual([rolled]);
        expect(state().draft?.totalMs).toBe(rolled.end - rolled.start);
      });

      it("keeps the latest ask when an older answer arrives after it", async () => {
        const first = deferred();
        mocked.recordingDraftSpans.mockReturnValueOnce(first.promise);
        void state().setDraftSpan(0, { start: noon.start, end: at("12:00") });
        await state().setDraftSpan(0, { start: noon.start, end: at("12:30") });
        expect(state().draft?.totalMs).toBe(90 * MINUTE_MS);
        first.answer({ spans: [{ start: noon.start, end: at("12:00") }], totalMs: 60 * MINUTE_MS });
        await new Promise((resolve) => setTimeout(resolve, 10));
        expect(state().draft?.spans).toEqual([{ start: noon.start, end: at("12:30") }]);
        expect(state().draft?.totalMs).toBe(90 * MINUTE_MS);
      });

      it("toasts when Rust cannot answer and keeps the times shown", async () => {
        mocked.recordingDraftSpans.mockRejectedValueOnce({ kind: "io", message: "no answer", code: null });
        await state().setDraftSpan(0, { start: noon.start, end: at("12:00") });
        await vi.waitFor(() => expect(toastMessages()).toEqual(["Could not change the times: no answer"]));
        expect(state().draft?.spans).toEqual([{ start: noon.start, end: at("12:00") }]);
      });
    });

    describe("resizeDraftSpan", () => {
      const dragged = { start: noon.start, end: at("12:03") };

      it("keeps the times until Rust has answered, then takes its snapped answer", async () => {
        const slow = deferred();
        mocked.recordingDraftResize.mockReturnValueOnce(slow.promise);
        const done = state().resizeDraftSpan(0, dragged, "end");
        expect(state().draft?.spans).toEqual([noon]);
        expect(mocked.recordingDraftResize).toHaveBeenCalledWith([noon], 0, dragged, "end");
        slow.answer({ spans: [{ start: noon.start, end: at("12:00") }], totalMs: 60 * MINUTE_MS });
        await done;
        expect(state().draft).toMatchObject({ spans: [{ start: noon.start, end: at("12:00") }], totalMs: 60 * MINUTE_MS });
      });

      it("takes the merged spans Rust answers with", async () => {
        state().addDraftSpan();
        await vi.waitFor(() => expect(state().draft?.spans).toHaveLength(2));
        const merged = { start: noon.start, end: noon.end + 30 * MINUTE_MS };
        mocked.recordingDraftResize.mockResolvedValueOnce({ spans: [merged], totalMs: merged.end - merged.start });
        await state().resizeDraftSpan(0, { start: noon.start, end: noon.end + 20 * MINUTE_MS }, "end");
        expect(state().draft?.spans).toEqual([merged]);
      });

      it("keeps the latest ask when an older answer arrives after it", async () => {
        const first = deferred();
        mocked.recordingDraftResize.mockReturnValueOnce(first.promise);
        void state().resizeDraftSpan(0, dragged, "end");
        await state().resizeDraftSpan(0, { start: noon.start, end: at("12:30") }, "end");
        first.answer({ spans: [{ start: noon.start, end: at("12:00") }], totalMs: 60 * MINUTE_MS });
        await new Promise((resolve) => setTimeout(resolve, 10));
        expect(state().draft?.spans).toEqual([{ start: noon.start, end: at("12:30") }]);
      });

      it("keeps the fields and toasts when Rust cannot answer", async () => {
        state().patchDraft({ note: "keep" });
        mocked.recordingDraftResize.mockRejectedValueOnce({ kind: "io", message: "no answer", code: null });
        await state().resizeDraftSpan(0, dragged, "end");
        expect(toastMessages()).toEqual(["Could not change the times: no answer"]);
        expect(state().draft).toMatchObject({ note: "keep", spans: [noon] });
      });

      it("shows the dragged span until Rust has answered, then drops it", async () => {
        const slow = deferred();
        mocked.recordingDraftResize.mockReturnValueOnce(slow.promise);
        state().setDraftDrag({ index: 0, span: dragged });
        const done = state().resizeDraftSpan(0, dragged, "end");
        expect(state().draftDrag).toEqual({ index: 0, span: dragged });
        slow.answer({ spans: [dragged], totalMs: 48 * MINUTE_MS });
        await done;
        expect(state().draftDrag).toBeNull();
      });

      it("drops the dragged span when Rust cannot answer", async () => {
        mocked.recordingDraftResize.mockRejectedValueOnce({ kind: "io", message: "no answer", code: null });
        state().setDraftDrag({ index: 0, span: dragged });
        await state().resizeDraftSpan(0, dragged, "end");
        expect(state().draftDrag).toBeNull();
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
      expect(mocked.recordingSnap).toHaveBeenCalledWith(at("09:02"), at("09:28"));
      expect(state().editor).toEqual({ mode: "create", ...morning });
      await vi.waitFor(() => expect(state().draft?.spans).toEqual([morning]));
    });

    it("remembers the clicked blocks of the selection a new editor opens on", async () => {
      state().setSelection({ ...morning, blockIds: ["a1"] });
      await state().recordSelection();
      expect(state().draftBlockIds).toEqual(["a1"]);
    });

    it("includes the selection as another span of an open editor and keeps its fields", async () => {
      await openCreate(noon);
      state().patchDraft({ note: "keep me", projectId: "p-site" });
      state().setSelection({ ...morning, blockIds: [] });
      await state().recordSelection();
      await vi.waitFor(() => expect(state().draft).toEqual({ projectId: "p-site", spans: [morning, noon], totalMs: 75 * MINUTE_MS, note: "keep me", tagId: null }));
      expect(mocked.recordingDraftInclude).toHaveBeenCalledWith([noon], morning);
      expect(state().editor).toEqual({ mode: "create", ...noon });
    });

    it("clears the selection once an open editor has taken it", async () => {
      await openCreate(noon);
      state().setSelection({ ...morning, blockIds: [] });
      await state().recordSelection();
      expect(state().selection).toBeNull();
    });

    it("toasts and opens no editor when Rust cannot snap", async () => {
      mocked.recordingSnap.mockRejectedValueOnce({ kind: "io", message: "no snap", code: null });
      state().setSelection({ ...morning, blockIds: [] });
      await state().recordSelection();
      expect(toastMessages()).toEqual(["Could not open the editor: no snap"]);
      expect(state().editor).toBeNull();
    });
  });

  describe("toggleBlock", () => {
    const docs = block({ id: "a1", ...morning });
    const todo = block({ id: "a2", ...noon });
    beforeEach(() => resetStore({ dayView: dayView({ lanes: [lane({ blocks: [docs, todo] })] }) }));

    it("opens an editor on the block's span and remembers the block", async () => {
      await state().toggleBlock("a1");
      expect(state().editor).toEqual({ mode: "create", ...morning });
      expect(state().draftBlockIds).toEqual(["a1"]);
      await vi.waitFor(() => expect(state().draft?.spans).toEqual([morning]));
    });

    it("adds a second block as another span and leaves the time between unassigned", async () => {
      await state().toggleBlock("a1");
      await vi.waitFor(() => expect(state().draft).not.toBeNull());
      await state().toggleBlock("a2");
      expect(state().draft?.spans).toEqual([morning, noon]);
      expect(state().draft?.totalMs).toBe(75 * MINUTE_MS);
      expect(state().draftBlockIds).toEqual(["a1", "a2"]);
    });

    it("removes the span of a block clicked again", async () => {
      await state().toggleBlock("a1");
      await vi.waitFor(() => expect(state().draft).not.toBeNull());
      await state().toggleBlock("a2");
      await state().toggleBlock("a1");
      expect(state().draft?.spans).toEqual([noon]);
      expect(state().draftBlockIds).toEqual(["a2"]);
    });

    it("closes the editor without saving when the last span goes", async () => {
      await state().toggleBlock("a1");
      await vi.waitFor(() => expect(state().draft).not.toBeNull());
      await state().toggleBlock("a1");
      expect(state().editor).toBeNull();
      expect(state().draft).toBeNull();
      expect(state().draftBlockIds).toEqual([]);
      expect(mocked.recordingCreate).not.toHaveBeenCalled();
    });

    it("ignores a block that is not in the view", async () => {
      await state().toggleBlock("missing");
      expect(state().editor).toBeNull();
    });

    it("does nothing while the running timer's recording is edited", async () => {
      const running = recording({ id: "r1", spans: [{ id: "r1-s1", ...evening }, { id: "r1-s2", start: at("18:00"), end: null }] });
      mocked.recordingsList.mockResolvedValue([running]);
      resetStore({ dayView: dayView({ lanes: [lane({ blocks: [docs, todo] })], recordings: [recordingBlock({ id: "r1", ...evening })] }) });
      state().openEditor({ mode: "edit", id: "r1" });
      await vi.waitFor(() => expect(state().draft).not.toBeNull());
      await state().toggleBlock("a1");
      expect(state().draft?.spans).toEqual([evening]);
      expect(state().draftBlockIds).toEqual([]);
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
