import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { at, DAY, dayView } from "../../test/dayFixtures";
import { dayKey, seedQuery } from "../../test/queries";
import { recording } from "../../test/recordingFixtures";
import { storedSuggestion, suggestion } from "../../test/ruleFixtures";
import { resetStore, useStore } from "../store";
import { isApprovable, isSettleable } from "./suggestions";

const mocked = vi.mocked(api, true);
const state = () => useStore.getState();
const toastMessages = () => state().toasts.map((t) => t.message);

const first = suggestion({ ruleIds: ["rule-1"], projectName: "Website", start: at("09:00"), end: at("09:30") });
const second = suggestion({ ruleIds: ["rule-2"], start: at("10:00"), end: at("10:30") });
const third = suggestion({ ruleIds: ["rule-1"], start: at("11:00"), end: at("11:30") });
const unassigned = suggestion({ ruleIds: ["rule-3"], projectId: null, projectName: null, color: "graphite", start: at("12:00"), end: at("12:30") });

/** The shown day's view, as the day query holds it, with these suggestions. */
const show = (...suggestions: ReturnType<typeof suggestion>[]) => seedQuery(dayKey(DAY), dayView({ suggestions }));

describe("suggestions slice", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ day: DAY });
    show(first, second, third);
  });

  describe("focusSuggestion", () => {
    it("takes the focus off recordings, blocks and the selection", () => {
      resetStore({ day: DAY, focusedRecording: "r1", focusedBlock: "a1", selection: { start: 1, end: 2, blockIds: [] } });
      state().focusSuggestion(first.id);
      expect([state().focusedSuggestion, state().focusedRecording, state().focusedBlock, state().selection]).toEqual([first.id, null, null, null]);
    });

    it("leaves the other focus alone when it clears the suggestion", () => {
      resetStore({ day: DAY, focusedSuggestion: first.id, focusedRecording: "r1" });
      state().focusSuggestion(null);
      expect([state().focusedSuggestion, state().focusedRecording]).toEqual([null, "r1"]);
    });

    it("is dropped again when a recording gets the focus", () => {
      state().focusSuggestion(first.id);
      state().focusRecording("r1");
      expect(state().focusedSuggestion).toBeNull();
    });
  });

  describe("approveSuggestion", () => {
    it("approves the suggestion on the shown day and offers Undo", async () => {
      mocked.suggestionApprove.mockResolvedValueOnce({ recording: recording(), durationMs: 45 * 60_000 });
      await state().approveSuggestion(first);
      expect(mocked.suggestionApprove).toHaveBeenCalledWith(DAY, first.id);
      expect(toastMessages()).toEqual(["Recorded 45m for Website"]);
      expect(state().toasts[0].action?.label).toBe("Undo");
    });

    it("runs the Undo action of the toast through the api", async () => {
      mocked.undo.mockResolvedValueOnce("Record 45m for Website");
      await state().approveSuggestion(first);
      state().toasts[0].action?.run();
      await vi.waitFor(() => expect(toastMessages()).toContain("Undid “Record 45m for Website”"));
    });

    it("toasts the error and offers no Undo when the suggestion is gone", async () => {
      mocked.suggestionApprove.mockRejectedValueOnce({ kind: "not_found", message: "Suggestion is gone", code: null });
      await state().approveSuggestion(first);
      expect(toastMessages()).toEqual(["Could not approve the suggestion: Suggestion is gone"]);
      expect(state().toasts[0].action).toBeUndefined();
    });

    it("keeps the focus and records nothing before the shown day has loaded", async () => {
      resetStore({ day: "2026-03-11", focusedSuggestion: first.id });
      await state().approveSuggestions([first.id]);
      expect(mocked.suggestionsApprove).not.toHaveBeenCalled();
      expect(state().focusedSuggestion).toBe(first.id);
    });

    it("moves the focus to the next suggestion", async () => {
      state().focusSuggestion(first.id);
      await state().approveSuggestion(first);
      expect(state().focusedSuggestion).toBe(second.id);
    });

    it("moves the focus back to the previous suggestion from the last one", async () => {
      state().focusSuggestion(third.id);
      await state().approveSuggestion(third);
      expect(state().focusedSuggestion).toBe(first.id);
    });

    it("clears the focus when it was the only suggestion", async () => {
      show(first);
      state().focusSuggestion(first.id);
      await state().approveSuggestion(first);
      expect(state().focusedSuggestion).toBeNull();
    });

    it("keeps the focus on another suggestion", async () => {
      state().focusSuggestion(second.id);
      await state().approveSuggestion(first);
      expect(state().focusedSuggestion).toBe(second.id);
    });

    it("keeps the focus where it was when the approval fails", async () => {
      mocked.suggestionApprove.mockRejectedValueOnce({ kind: "db", message: "Locked", code: null });
      state().focusSuggestion(first.id);
      await state().approveSuggestion(first);
      expect(state().focusedSuggestion).toBe(first.id);
    });
  });

  describe("approveSuggestion without a project", () => {
    it("opens the editor without a project instead of approving", async () => {
      await state().approveSuggestion(unassigned);
      expect(mocked.suggestionApprove).not.toHaveBeenCalled();
      expect(state().focusedSuggestion).toBe(unassigned.id);
      expect(state().editor).toMatchObject({ mode: "create", start: unassigned.start, end: unassigned.end, suggestion: { id: unassigned.id, by: unassigned.by } });
      expect(state().editor).toHaveProperty("projectId", undefined);
      expect(toastMessages()).toEqual([]);
    });
  });

  describe("a running suggestion", () => {
    const running = suggestion({ running: true, ruleIds: [], start: at("13:00"), end: at("13:30") });

    beforeEach(() => show(first, running));

    it("is not approved", async () => {
      await state().approveSuggestion(running);
      expect(mocked.suggestionApprove).not.toHaveBeenCalled();
      expect(toastMessages()).toEqual([]);
    });

    it("does not open the editor when it has no project either", async () => {
      const unassignedRunning = suggestion({ running: true, projectId: null, projectName: null });
      await state().approveSuggestion(unassignedRunning);
      expect(state().editor).toBeNull();
    });

    it("is skipped when approving several", async () => {
      mocked.suggestionsApprove.mockResolvedValueOnce([recording({ id: "r1" })]);
      await state().approveSuggestions([first.id, running.id]);
      expect(mocked.suggestionsApprove).toHaveBeenCalledWith(DAY, [first.id]);
    });

    it("asks for nothing when it is the only one to approve", async () => {
      await state().approveSuggestions([running.id]);
      expect(mocked.suggestionsApprove).not.toHaveBeenCalled();
    });

    it("is left out when dismissing several", async () => {
      await state().dismissSuggestions([first.id, running.id]);
      expect(mocked.suggestionsDismiss).toHaveBeenCalledWith(DAY, [first.id]);
    });

    it("asks for nothing when it is the only one to dismiss", async () => {
      await state().dismissSuggestions([running.id]);
      expect(mocked.suggestionsDismiss).not.toHaveBeenCalled();
      expect(toastMessages()).toEqual([]);
    });
  });

  describe("isSettleable", () => {
    it.each([
      ["finished", false, true],
      ["running", true, false],
    ])("is decided by running: a %s suggestion", (_, running, expected) => {
      expect(isSettleable(suggestion({ running }))).toBe(expected);
    });
  });

  describe("isApprovable", () => {
    it.each([
      ["has a project", first, true],
      ["has none", unassigned, false],
    ])("is decided by the project: a suggestion that %s", (_, s, expected) => {
      expect(isApprovable(s)).toBe(expected);
    });
  });

  describe("approveRuleSuggestions", () => {
    it("approves every suggestion of the rule on the shown day", async () => {
      mocked.suggestionApproveRule.mockResolvedValueOnce([recording({ id: "r1" }), recording({ id: "r2" })]);
      await state().approveRuleSuggestions("rule-1");
      expect(mocked.suggestionApproveRule).toHaveBeenCalledWith(DAY, "rule-1");
      expect(toastMessages()).toEqual(["Recorded 2 suggestions"]);
    });

    it("says 1 suggestion for a single recording", async () => {
      mocked.suggestionApproveRule.mockResolvedValueOnce([recording()]);
      await state().approveRuleSuggestions("rule-2");
      expect(toastMessages()).toEqual(["Recorded 1 suggestion"]);
    });

    it("toasts the error", async () => {
      mocked.suggestionApproveRule.mockRejectedValueOnce({ kind: "db", message: "Locked", code: null });
      await state().approveRuleSuggestions("rule-1");
      expect(toastMessages()).toEqual(["Could not approve the suggestions: Locked"]);
    });

    it("moves the focus to the first suggestion that is left", async () => {
      state().focusSuggestion(first.id);
      await state().approveRuleSuggestions("rule-1");
      expect(state().focusedSuggestion).toBe(second.id);
    });

    it("counts a suggestion joined from several rules as the rule's", async () => {
      const joined = suggestion({ ruleIds: ["rule-0", "rule-1"], start: at("08:00"), end: at("08:30") });
      show(joined, second, third);
      state().focusSuggestion(joined.id);
      await state().approveRuleSuggestions("rule-1");
      expect(state().focusedSuggestion).toBe(second.id);
    });
  });

  describe("approveSuggestions", () => {
    it("approves the given suggestions of the shown day as one change", async () => {
      mocked.suggestionsApprove.mockResolvedValueOnce([recording({ id: "r1" }), recording({ id: "r2" })]);
      await state().approveSuggestions([first.id, second.id]);
      expect(mocked.suggestionsApprove).toHaveBeenCalledWith(DAY, [first.id, second.id]);
      expect(toastMessages()).toEqual(["Recorded 2 suggestions"]);
      expect(state().toasts[0].action?.label).toBe("Undo");
    });

    it("asks for nothing without suggestions", async () => {
      await state().approveSuggestions([]);
      expect(mocked.suggestionsApprove).not.toHaveBeenCalled();
      expect(toastMessages()).toEqual([]);
    });

    it("sends only the suggestions that have a project", async () => {
      show(first, unassigned, second);
      mocked.suggestionsApprove.mockResolvedValueOnce([recording({ id: "r1" }), recording({ id: "r2" })]);
      await state().approveSuggestions([first.id, unassigned.id, second.id]);
      expect(mocked.suggestionsApprove).toHaveBeenCalledWith(DAY, [first.id, second.id]);
    });

    it("asks for nothing when none of the suggestions has a project", async () => {
      show(unassigned);
      await state().approveSuggestions([unassigned.id]);
      expect(mocked.suggestionsApprove).not.toHaveBeenCalled();
      expect(toastMessages()).toEqual([]);
    });

    it("toasts the error", async () => {
      mocked.suggestionsApprove.mockRejectedValueOnce({ kind: "db", message: "Locked", code: null });
      await state().approveSuggestions([first.id]);
      expect(toastMessages()).toEqual(["Could not approve the suggestions: Locked"]);
    });
  });

  describe("dismissSuggestions", () => {
    it("dismisses the given suggestions of the shown day and says how many", async () => {
      mocked.suggestionsDismiss.mockResolvedValueOnce(2);
      await state().dismissSuggestions([first.id, second.id]);
      expect(mocked.suggestionsDismiss).toHaveBeenCalledWith(DAY, [first.id, second.id]);
      expect(toastMessages()).toEqual(["Dismissed 2 suggestions"]);
      expect(state().toasts[0].action?.label).toBe("Undo");
    });

    it("says 1 suggestion for a single one", async () => {
      await state().dismissSuggestions([first.id]);
      expect(toastMessages()).toEqual(["Dismissed 1 suggestion"]);
    });

    it("asks for nothing without suggestions", async () => {
      await state().dismissSuggestions([]);
      expect(mocked.suggestionsDismiss).not.toHaveBeenCalled();
    });

    it("toasts the error", async () => {
      mocked.suggestionsDismiss.mockRejectedValueOnce({ kind: "db", message: "Locked", code: null });
      await state().dismissSuggestions([first.id]);
      expect(toastMessages()).toEqual(["Could not dismiss the suggestions: Locked"]);
    });

    it("moves the focus to the next suggestion that is left", async () => {
      state().focusSuggestion(first.id);
      await state().dismissSuggestions([first.id, second.id]);
      expect(state().focusedSuggestion).toBe(third.id);
    });
  });

  describe("editSuggestion", () => {
    it("focuses the suggestion and opens the editor prefilled with it", async () => {
      const proposed = suggestion({ by: "Slack", projectId: "p-site", note: "standup", tagId: "tag-1", start: at("09:00"), end: at("09:30") });
      state().editSuggestion(proposed);
      expect(state().focusedSuggestion).toBe(proposed.id);
      expect(state().editor).toEqual({
        mode: "create",
        start: at("09:00"),
        end: at("09:30"),
        projectId: "p-site",
        note: "standup",
        tagId: "tag-1",
        suggestion: { id: proposed.id, by: "Slack" },
      });
      await vi.waitFor(() => expect(state().draft).toMatchObject({ projectId: "p-site", note: "standup", tagId: "tag-1", spans: [{ start: at("09:00"), end: at("09:30") }] }));
    });

    it("opens a running stored suggestion on its start as the running span, with no closed spans", async () => {
      const timer = storedSuggestion({ running: true, projectId: "p-site", note: "build", start: at("09:00"), end: at("09:30") });
      state().editSuggestion(timer);
      expect(state().editor).toMatchObject({ mode: "create", openStart: at("09:00"), suggestion: { id: timer.id, by: "cli", running: { storedId: "st-1" } } });
      await vi.waitFor(() => expect(state().draft).toMatchObject({ projectId: "p-site", note: "build", spans: [], openStart: at("09:00") }));
    });

    it("toasts and opens nothing while the open editor has unsaved changes, leaving the focus alone", async () => {
      state().openEditor({ mode: "create", start: at("13:00"), end: at("13:30") });
      await vi.waitFor(() => expect(state().draft).not.toBeNull());
      state().patchDraft({ note: "half written" });
      state().focusSuggestion(second.id);
      state().editSuggestion(first);
      expect(toastMessages()).toEqual(["Save or cancel the open recording first"]);
      expect(state().dialog).toBeNull();
      expect(state().editor).toEqual({ mode: "create", start: at("13:00"), end: at("13:30") });
      expect(state().focusedSuggestion).toBe(second.id);
    });

    it("replaces an unchanged open editor", async () => {
      state().openEditor({ mode: "create", start: at("13:00"), end: at("13:30") });
      await vi.waitFor(() => expect(state().draft).not.toBeNull());
      state().editSuggestion(first);
      expect(state().editor).toMatchObject({ mode: "create", suggestion: { id: first.id } });
      expect(state().focusedSuggestion).toBe(first.id);
    });

    it("keeps the editor of the same suggestion with its typed note", async () => {
      state().editSuggestion(first);
      await vi.waitFor(() => expect(state().draft).not.toBeNull());
      state().patchDraft({ note: "typed" });
      state().editSuggestion(first);
      expect(state().dialog).toBeNull();
      expect(state().draft?.note).toBe("typed");
    });
  });

  describe("updateSuggestion", () => {
    const patch = { projectId: "p-site", note: "changed", tagId: null };

    it("saves the patch to the stored suggestion, resolves true and offers Undo", async () => {
      await expect(state().updateSuggestion("st-1", patch)).resolves.toBe(true);
      expect(mocked.suggestionUpdate).toHaveBeenCalledWith("st-1", patch);
      expect(toastMessages()).toEqual(["Suggestion changed"]);
      expect(state().toasts[0].action?.label).toBe("Undo");
    });

    it("toasts the error and resolves false when the save fails", async () => {
      mocked.suggestionUpdate.mockRejectedValueOnce({ kind: "db", message: "Locked", code: null });
      await expect(state().updateSuggestion("st-1", patch)).resolves.toBe(false);
      expect(toastMessages()).toEqual(["Could not change the suggestion: Locked"]);
    });

    it("rejects with the error a form field shows, without a toast", async () => {
      const error = { kind: "invalid_input", message: "`start`: in the future", code: null, field: { path: "start", message: "The start is in the future" } };
      mocked.suggestionUpdate.mockRejectedValueOnce(error);
      await expect(state().updateSuggestion("st-1", patch)).rejects.toEqual(error);
      expect(toastMessages()).toEqual([]);
    });
  });

  describe("clickSuggestion", () => {
    const openElsewhere = async () => {
      state().openEditor({ mode: "create", start: at("13:00"), end: at("13:30") });
      await vi.waitFor(() => expect(state().draft).not.toBeNull());
    };

    it("opens the suggestion in the editor without one open", () => {
      state().clickSuggestion(first);
      expect(state().editor).toMatchObject({ mode: "create", start: first.start, end: first.end, suggestion: { id: first.id } });
      expect(state().focusedSuggestion).toBe(first.id);
    });

    it("joins all of the suggestion's time into the open draft", async () => {
      await openElsewhere();
      state().clickSuggestion(first);
      await vi.waitFor(() => expect(state().draft?.spans).toEqual([{ start: first.start, end: first.end }, { start: at("13:00"), end: at("13:30") }]));
      expect(state().editor).toEqual({ mode: "create", start: at("13:00"), end: at("13:30") });
    });

    it("joins only the part given into the open draft", async () => {
      await openElsewhere();
      const part = { start: at("09:10"), end: at("09:20") };
      state().clickSuggestion(first, part);
      await vi.waitFor(() => expect(state().draft?.spans).toEqual([part, { start: at("13:00"), end: at("13:30") }]));
    });

    it("keeps the open editor's typed fields when it takes the time", async () => {
      await openElsewhere();
      state().patchDraft({ note: "typed" });
      state().clickSuggestion(first);
      await vi.waitFor(() => expect(state().draft?.spans).toHaveLength(2));
      expect(state().draft?.note).toBe("typed");
      expect(toastMessages()).toEqual([]);
    });
  });

  describe("approving an edited suggestion", () => {
    it("saves the editor's input as the suggestion's approval instead of a plain recording", async () => {
      const input = { projectId: "project-1", spans: [{ start: first.start, end: first.end }], note: "changed", tagId: null };
      await state().createRecording(input, first.id);
      expect(mocked.suggestionApproveEdited).toHaveBeenCalledWith(DAY, first.id, input, "cancel");
      expect(mocked.recordingCreate).not.toHaveBeenCalled();
    });
  });
});
