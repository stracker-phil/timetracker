import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { at, DAY, dayView } from "../../test/dayFixtures";
import { recording } from "../../test/recordingFixtures";
import { suggestion } from "../../test/ruleFixtures";
import { resetStore, useStore } from "../store";

const mocked = vi.mocked(api, true);
const state = () => useStore.getState();
const toastMessages = () => state().toasts.map((t) => t.message);

const first = suggestion({ ruleId: "rule-1", projectName: "Website", start: at("09:00"), end: at("09:30") });
const second = suggestion({ ruleId: "rule-2", start: at("10:00"), end: at("10:30") });
const third = suggestion({ ruleId: "rule-1", start: at("11:00"), end: at("11:30") });

describe("suggestions slice", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ day: DAY, dayView: dayView({ suggestions: [first, second, third] }) });
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
      expect(toastMessages()).toEqual(["Recorded 45 min for Website"]);
      expect(state().toasts[0].action?.label).toBe("Undo");
    });

    it("runs the Undo action of the toast through the api", async () => {
      mocked.undo.mockResolvedValueOnce("Record 45 min for Website");
      await state().approveSuggestion(first);
      state().toasts[0].action?.run();
      await vi.waitFor(() => expect(toastMessages()).toContain("Undid “Record 45 min for Website”"));
    });

    it("toasts the error and offers no Undo when the suggestion is gone", async () => {
      mocked.suggestionApprove.mockRejectedValueOnce({ kind: "not_found", message: "Suggestion is gone", code: null });
      await state().approveSuggestion(first);
      expect(toastMessages()).toEqual(["Could not approve the suggestion: Suggestion is gone"]);
      expect(state().toasts[0].action).toBeUndefined();
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
      resetStore({ day: DAY, dayView: dayView({ suggestions: [first] }), focusedSuggestion: first.id });
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
      const proposed = suggestion({ ruleName: "Slack", projectId: "p-site", note: "standup", tagId: "tag-1", start: at("09:00"), end: at("09:30") });
      state().editSuggestion(proposed);
      expect(state().focusedSuggestion).toBe(proposed.id);
      expect(state().editor).toEqual({
        mode: "create",
        start: at("09:00"),
        end: at("09:30"),
        projectId: "p-site",
        note: "standup",
        tagId: "tag-1",
        suggestion: { id: proposed.id, ruleName: "Slack" },
      });
      await vi.waitFor(() => expect(state().draft).toMatchObject({ projectId: "p-site", note: "standup", tagId: "tag-1", spans: [{ start: at("09:00"), end: at("09:30") }] }));
    });
  });

  describe("approving an edited suggestion", () => {
    it("saves the editor's input as the suggestion's approval instead of a plain recording", async () => {
      const input = { projectId: first.projectId, spans: [{ start: first.start, end: first.end }], note: "changed", tagId: null };
      await state().createRecording(input, first.id);
      expect(mocked.suggestionApproveEdited).toHaveBeenCalledWith(DAY, first.id, input, "cancel");
      expect(mocked.recordingCreate).not.toHaveBeenCalled();
    });
  });
});
