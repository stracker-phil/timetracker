import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type DraftSpans, type Span } from "../../api";
import { at, block, DAY, dayView, lane, recordingBlock } from "../../test/dayFixtures";
import { dayKey, seedQuery } from "../../test/queries";
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
    resetStore({ day: DAY });
  });

  describe("openEditor", () => {
    it("opens a create editor and fills a draft of its span once Rust has answered for it", async () => {
      state().openEditor({ mode: "create", ...morning });
      expect(state().editor).toEqual({ mode: "create", ...morning });
      expect(state().draft).toBeNull();
      await vi.waitFor(() => expect(state().draft).toEqual({ projectId: null, spans: [morning], totalMs: 30 * MINUTE_MS, openStart: null, note: "", tagId: null }));
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
        seedQuery(dayKey(DAY), dayView({ recordings: [recordingBlock({ id: "r1", ...morning })] }));
        state().openEditor({ mode: "edit", id: "r1" });
        expect(state().draft).toBeNull();
        await vi.waitFor(() =>
          expect(state().draft).toEqual({ projectId: "p-site", spans: [morning, evening], totalMs: 90 * MINUTE_MS, openStart: at("18:00"), note: "standup", tagId: "tag-research" }),
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

      it("loads nothing when the shown day has not loaded and the opener names no span", () => {
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
      expect(state().draft).toEqual({ projectId: null, spans: [noon], totalMs: 45 * MINUTE_MS, openStart: null, note: "fix", tagId: "tag-research" });
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

    it("writes nothing for a new recording while its times change", async () => {
      state().setDraftTotal(60 * MINUTE_MS);
      await state().setDraftSpan(0, { start: noon.start, end: at("12:30") });
      await state().spansSettled();
      expect(mocked.recordingUpdate).not.toHaveBeenCalled();
      expect(mocked.recordingCreate).not.toHaveBeenCalled();
    });
  });

  describe("for an edited recording", () => {
    const stored = recording({ id: "r1", projectId: "p-site", note: "standup", tagId: "tag-research", spans: [{ id: "r1-s1", ...morning }, { id: "r1-s2", ...noon }] });
    const single = recording({ ...stored, spans: [{ id: "r1-s1", ...morning }] });
    const docs = block({ id: "a1", ...morning });
    const late = block({ id: "a3", ...evening });
    const longer = { start: morning.start, end: at("09:45") };

    async function openEdited(row = stored) {
      mocked.recordingsList.mockResolvedValue([row]);
      seedQuery(dayKey(DAY), dayView({ lanes: [lane({ blocks: [docs, late] })], recordings: [recordingBlock({ id: "r1", ...morning })] }));
      state().openEditor({ mode: "edit", id: "r1" });
      await vi.waitFor(() => expect(state().draft).not.toBeNull());
    }

    it.each([
      ["a typed span", () => state().setDraftSpan(0, longer), [longer, noon]],
      ["a removed span", () => state().removeDraftSpan(0), [noon]],
      ["an added span", () => state().addDraftSpan(), [morning, noon, { start: noon.end, end: noon.end + 30 * MINUTE_MS }]],
      ["a new total", () => state().setDraftTotal(90 * MINUTE_MS), [morning, { start: noon.start, end: at("12:00") }]],
      ["a dragged edge", () => state().resizeDraftSpan(1, { start: noon.start, end: at("12:00") }, "end"), [morning, { start: noon.start, end: at("12:00") }]],
    ])("saves %s as the recording's spans at once and shows them", async (_name, change, spans) => {
      await openEdited();
      void change();
      await state().spansSettled();
      expect(mocked.recordingUpdate).toHaveBeenLastCalledWith("r1", { spans }, "cancel");
      expect(state().draft?.spans).toEqual(spans);
      expect(toastMessages()).toEqual(["Recording changed"]);
    });

    it("writes only the spans and leaves the fields typed meanwhile in the draft", async () => {
      await openEdited();
      state().patchDraft({ note: "typed" });
      await state().setDraftSpan(0, longer);
      expect(mocked.recordingUpdate).toHaveBeenLastCalledWith("r1", { spans: [longer, noon] }, "cancel");
      expect(state().draft?.note).toBe("typed");
    });

    it("shows the spans of the recording Rust saved", async () => {
      await openEdited();
      const rolled = { start: morning.start, end: at("09:35") };
      mocked.recordingUpdate.mockResolvedValueOnce(recording({ ...stored, spans: [{ id: "r1-s1", ...rolled }, { id: "r1-s2", ...noon }] }));
      await state().setDraftSpan(0, { start: morning.start, end: at("09:32") });
      await state().spansSettled();
      expect(state().draft?.spans).toEqual([rolled, noon]);
      expect(state().editing?.spans.map((s) => s.end)).toEqual([rolled.end, noon.end]);
    });

    it("saves the newest of two quick changes", async () => {
      await openEdited();
      void state().setDraftSpan(0, { start: morning.start, end: at("09:40") });
      await state().setDraftSpan(0, longer);
      await state().spansSettled();
      expect(mocked.recordingUpdate).toHaveBeenLastCalledWith("r1", { spans: [longer, noon] }, "cancel");
      expect(state().draft?.spans).toEqual([longer, noon]);
    });

    it("includes a selection as another span and saves it", async () => {
      await openEdited();
      state().setSelection({ ...evening, blockIds: [] });
      await state().recordSelection();
      expect(mocked.recordingUpdate).toHaveBeenLastCalledWith("r1", { spans: [morning, noon, evening] }, "cancel");
      expect(state().draft?.spans).toEqual([morning, noon, evening]);
    });

    it("saves the span of a clicked activity block and marks the block", async () => {
      await openEdited();
      await state().toggleBlock("a3");
      expect(mocked.recordingUpdate).toHaveBeenLastCalledWith("r1", { spans: [morning, noon, evening] }, "cancel");
      expect(state().draftBlockIds).toEqual(["a3"]);
    });

    it("keeps the editor and writes nothing when the only span is clicked out", async () => {
      await openEdited(single);
      await state().toggleBlock("a1");
      expect(state().editor).toEqual({ mode: "edit", id: "r1" });
      expect(state().draft?.spans).toEqual([morning]);
      expect(mocked.recordingUpdate).not.toHaveBeenCalled();
    });

    it("shows the saved spans again and toasts when the write fails", async () => {
      await openEdited();
      mocked.recordingUpdate.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      await state().setDraftSpan(0, longer);
      await state().spansSettled();
      expect(state().draft?.spans).toEqual([morning, noon]);
      expect(toastMessages()).toEqual(["Could not change the recording: Disk full"]);
    });

    describe("on an overlap", () => {
      const inTheWay = recording({ id: "r9", start: at("09:15"), end: at("09:50") });

      async function writeIntoOverlap() {
        await openEdited();
        mocked.recordingUpdate.mockRejectedValueOnce({ kind: "overlap", message: "Overlaps a recording", code: null, details: [inTheWay] });
        const done = state().setDraftSpan(0, longer);
        await vi.waitFor(() => expect(state().editorOverlap?.recordings).toEqual([inTheWay]));
        return { done };
      }

      it("shows the saved spans again when the question is declined", async () => {
        const { done } = await writeIntoOverlap();
        state().answerOverlap(false);
        await done;
        expect(state().draft?.spans).toEqual([morning, noon]);
        expect(mocked.recordingUpdate).not.toHaveBeenCalledWith(expect.anything(), expect.anything(), "replace");
      });

      it("writes the spans again with replace when the question is accepted", async () => {
        const { done } = await writeIntoOverlap();
        state().answerOverlap(true);
        await done;
        expect(mocked.recordingUpdate).toHaveBeenLastCalledWith("r1", { spans: [longer, noon] }, "replace");
        expect(state().draft?.spans).toEqual([longer, noon]);
      });
    });

    it("ignores a new open start for a recording the timer does not run on", async () => {
      await openEdited();
      await state().setOpenStart(at("09:10"));
      expect(state().draft?.openStart).toBeNull();
      expect(mocked.recordingUpdate).not.toHaveBeenCalled();
    });

    describe("while the timer runs on it", () => {
      const started = at("18:00");
      const running = (closed: Span[], openStart = started) =>
        recording({ ...stored, spans: [...closed.map((s, i) => ({ id: `r1-s${i + 1}`, ...s })), { id: `r1-s${closed.length + 1}`, start: openStart, end: null }] });
      const original = mocked.recordingUpdate.getMockImplementation();

      // Rust keeps the times a patch leaves out, and the timer running.
      beforeEach(() => {
        let closed = [morning, noon];
        let openStart = started;
        mocked.recordingUpdate.mockImplementation(async (_id, patch) => {
          closed = patch.spans ?? closed;
          openStart = patch.openStart ?? openStart;
          return running(closed, openStart);
        });
      });

      afterEach(() => void (original && mocked.recordingUpdate.mockImplementation(original)));

      it("loads the open span's start into the draft apart from the closed spans", async () => {
        await openEdited(running([morning, noon]));
        expect(state().draft).toMatchObject({ spans: [morning, noon], openStart: started });
      });

      it("writes only the closed spans for a change of them", async () => {
        await openEdited(running([morning, noon]));
        await state().setDraftSpan(0, longer);
        await state().spansSettled();
        expect(mocked.recordingUpdate).toHaveBeenLastCalledWith("r1", { spans: [longer, noon] }, "cancel");
        expect(mocked.recordingUpdate.mock.lastCall?.[1]).not.toHaveProperty("openStart");
        expect(state().draft).toMatchObject({ spans: [longer, noon], openStart: started });
      });

      it("shows a new open start at once and writes only it", async () => {
        await openEdited(running([morning, noon]));
        const done = state().setOpenStart(at("18:10"));
        expect(state().draft?.openStart).toBe(at("18:10"));
        await done;
        expect(mocked.recordingUpdate).toHaveBeenLastCalledWith("r1", { openStart: at("18:10") }, "cancel");
        expect(mocked.recordingUpdate.mock.lastCall?.[1]).not.toHaveProperty("spans");
      });

      it("shows the open start Rust saved", async () => {
        await openEdited(running([morning, noon]));
        mocked.recordingUpdate.mockResolvedValueOnce(running([morning, noon], at("18:15")));
        await state().setOpenStart(at("18:12"));
        expect(state().draft?.openStart).toBe(at("18:15"));
        expect(state().editing?.spans.at(-1)?.start).toBe(at("18:15"));
      });

      it("shows the saved open start again and toasts when the write fails", async () => {
        await openEdited(running([morning, noon]));
        mocked.recordingUpdate.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
        await state().setOpenStart(at("18:10"));
        expect(state().draft?.openStart).toBe(started);
        expect(toastMessages()).toEqual(["Could not change the recording: Disk full"]);
      });

      it("merges changes asked during a write into one later write of the spans and the open start", async () => {
        await openEdited(running([morning, noon]));
        let finish!: (saved: ReturnType<typeof recording>) => void;
        mocked.recordingUpdate.mockReturnValueOnce(new Promise((resolve) => (finish = resolve)));
        void state().setDraftSpan(0, longer);
        await vi.waitFor(() => expect(mocked.recordingUpdate).toHaveBeenLastCalledWith("r1", { spans: [longer, noon] }, "cancel"));
        const shorter = { start: noon.start, end: at("11:30") };
        void state().setOpenStart(at("18:10"));
        void state().setDraftSpan(1, shorter);
        await new Promise((resolve) => setTimeout(resolve, 10));
        finish(running([longer, noon]));
        await state().spansSettled();
        expect(mocked.recordingUpdate).toHaveBeenLastCalledWith("r1", { spans: [longer, shorter], openStart: at("18:10") }, "cancel");
        expect(state().draft).toMatchObject({ spans: [longer, shorter], openStart: at("18:10") });
      });

      it("saves the span of a clicked activity block and keeps the timer running", async () => {
        await openEdited(running([morning, noon]));
        await state().toggleBlock("a3");
        expect(mocked.recordingUpdate).toHaveBeenLastCalledWith("r1", { spans: [morning, noon, evening] }, "cancel");
        expect(state().draft).toMatchObject({ spans: [morning, noon, evening], openStart: started });
        expect(state().draftBlockIds).toEqual(["a3"]);
      });

      it("saves an included selection as another closed span", async () => {
        await openEdited(running([morning, noon]));
        state().setSelection({ ...evening, blockIds: [] });
        await state().recordSelection();
        expect(mocked.recordingUpdate).toHaveBeenLastCalledWith("r1", { spans: [morning, noon, evening] }, "cancel");
        expect(state().draft?.openStart).toBe(started);
      });

      it("toasts and keeps the saved spans when Rust refuses a clicked block", async () => {
        await openEdited(running([morning, noon]));
        mocked.recordingDraftToggle.mockRejectedValueOnce({ kind: "io", message: "no answer", code: null });
        await state().toggleBlock("a3");
        expect(toastMessages()).toEqual(["Could not change the times: no answer"]);
        expect(state().draft?.spans).toEqual([morning, noon]);
        expect(mocked.recordingUpdate).not.toHaveBeenCalled();
      });

      it("toasts and keeps the saved spans when Rust refuses a selection", async () => {
        await openEdited(running([morning, noon]));
        mocked.recordingDraftInclude.mockRejectedValueOnce({ kind: "io", message: "no answer", code: null });
        state().setSelection({ ...evening, blockIds: [] });
        await state().recordSelection();
        expect(toastMessages()).toEqual(["Could not change the times: no answer"]);
        expect(state().draft?.spans).toEqual([morning, noon]);
        expect(mocked.recordingUpdate).not.toHaveBeenCalled();
      });

      describe("syncEdited", () => {
        it("takes an open start that moved alone", async () => {
          await openEdited(running([morning, noon]));
          mocked.recordingsList.mockResolvedValue([running([morning, noon], at("18:05"))]);
          await state().syncEdited();
          expect(state().draft).toMatchObject({ spans: [morning, noon], openStart: at("18:05") });
        });

        it("takes a stop: the open span becomes a closed one and no open start is left", async () => {
          await openEdited(running([morning, noon]));
          const stopped = { start: started, end: at("18:40") };
          mocked.recordingsList.mockResolvedValue([recording({ ...stored, spans: [{ id: "r1-s1", ...morning }, { id: "r1-s2", ...noon }, { id: "r1-s3", ...stopped }] })]);
          await state().syncEdited();
          expect(state().draft).toMatchObject({ spans: [morning, noon, stopped], openStart: null });
        });
      });
    });

    describe("spansSettled", () => {
      it("resolves only once the span write has finished", async () => {
        await openEdited();
        let finish!: (saved: ReturnType<typeof recording>) => void;
        mocked.recordingUpdate.mockReturnValueOnce(new Promise((resolve) => (finish = resolve)));
        void state().setDraftSpan(0, longer);
        let settled = false;
        const waiting = state()
          .spansSettled()
          .then(() => void (settled = true));
        await new Promise((resolve) => setTimeout(resolve, 10));
        expect(settled).toBe(false);
        finish(recording({ ...stored, spans: [{ id: "r1-s1", ...longer }, { id: "r1-s2", ...noon }] }));
        await waiting;
        expect(settled).toBe(true);
      });

      it("resolves at once without a change in flight", async () => {
        await openEdited();
        await expect(state().spansSettled()).resolves.toBeUndefined();
      });
    });

    describe("syncEdited", () => {
      const nudged = recording({ ...stored, spans: [{ id: "r1-s1", start: at("09:10"), end: morning.end }, { id: "r1-s2", ...noon }] });

      it("takes the spans Rust now holds and keeps the typed fields", async () => {
        await openEdited();
        state().patchDraft({ note: "typed" });
        mocked.recordingsList.mockResolvedValue([nudged]);
        await state().syncEdited();
        expect(state().draft?.spans).toEqual([{ start: at("09:10"), end: morning.end }, noon]);
        expect(state().draft?.totalMs).toBe(75 * MINUTE_MS - 10 * MINUTE_MS);
        expect(state().draft?.note).toBe("typed");
        expect(state().editing).toEqual(nudged);
      });

      it("leaves a new recording alone", async () => {
        await openCreate(noon);
        mocked.recordingsList.mockResolvedValue([nudged]);
        await state().syncEdited();
        expect(state().draft?.spans).toEqual([noon]);
        expect(state().editing).toBeNull();
      });

      it("does not ask to discard when the editor closes after the sync", async () => {
        await openEdited();
        mocked.recordingsList.mockResolvedValue([nudged]);
        await state().syncEdited();
        const proceed = vi.fn();
        state().leaveEditor(proceed);
        expect(proceed).toHaveBeenCalled();
        expect(state().dialog).toBeNull();
      });
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
      await vi.waitFor(() => expect(state().draft).toEqual({ projectId: "p-site", spans: [morning, noon], totalMs: 75 * MINUTE_MS, openStart: null, note: "keep me", tagId: null }));
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
    beforeEach(() => seedQuery(dayKey(DAY), dayView({ lanes: [lane({ blocks: [docs, todo] })] })));

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
