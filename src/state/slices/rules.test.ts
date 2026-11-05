import { beforeEach, describe, expect, it, vi } from "vitest";
import { waitFor } from "@testing-library/react";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Rule, type RuleInput } from "../../api";
import { useRules } from "../../queries/rules";
import { queryData, renderHookWithClient, RULES_KEY, seedQuery } from "../../test/queries";
import { rule } from "../../test/ruleFixtures";
import { resetStore, useStore } from "../store";

const mocked = vi.mocked(api, true);
const state = () => useStore.getState();
const toastMessages = () => state().toasts.map((t) => t.message);
const ids = () => (queryData<Rule[]>(RULES_KEY) ?? []).map((r) => r.id);
const fieldError = { kind: "invalid_input", message: "Invalid", code: null, field: { path: "conditions.0.value", message: "Needs a value" } };
const input: RuleInput = { name: "Slack", projectId: "project-1", enabled: true, note: "", tagId: null, conditions: [{ field: "app", op: "is", value: "Slack" }] };

const a = rule({ id: "a", name: "A", position: 0 });
const b = rule({ id: "b", name: "B", position: 1 });
const c = rule({ id: "c", name: "C", position: 2 });

describe("rules slice", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ day: "2026-03-10" });
  });

  describe("saveRule", () => {
    it("sends the id and input, shows the refetched rules and returns the saved rule", async () => {
      seedQuery(RULES_KEY, [a]);
      const view = renderHookWithClient(useRules);
      mocked.rulesList.mockResolvedValue([a, b]);
      const saved = await state().saveRule("b", input);
      expect(mocked.ruleSave).toHaveBeenCalledWith("b", input);
      expect(saved.id).toBe("b");
      await waitFor(() => expect(view.result.current.map((r) => r.id)).toEqual(["a", "b"]));
    });

    it("rejects with a field error without a toast", async () => {
      mocked.ruleSave.mockRejectedValueOnce(fieldError);
      await expect(state().saveRule(null, input)).rejects.toMatchObject({ kind: "invalid_input" });
      expect(toastMessages()).toEqual([]);
    });

    it("rejects and toasts any other error", async () => {
      mocked.ruleSave.mockRejectedValueOnce({ kind: "db", message: "Locked", code: null });
      await expect(state().saveRule(null, input)).rejects.toMatchObject({ kind: "db" });
      expect(toastMessages()).toEqual(["Could not save the rule: Locked"]);
    });
  });

  describe("deleteRule", () => {
    it("removes the rule from the list", async () => {
      seedQuery(RULES_KEY, [a, b]);
      await state().deleteRule("a");
      expect(mocked.ruleDelete).toHaveBeenCalledWith("a");
      expect(ids()).toEqual(["b"]);
    });

    it("keeps the rule and toasts when the delete fails", async () => {
      seedQuery(RULES_KEY, [a, b]);
      mocked.ruleDelete.mockRejectedValueOnce({ kind: "not_found", message: "Gone", code: null });
      await state().deleteRule("a");
      expect(ids()).toEqual(["a", "b"]);
      expect(toastMessages()).toEqual(["Could not delete the rule: Gone"]);
    });
  });

  describe("reorderRules", () => {
    it("shows the new order at once and keeps the order the backend answers", async () => {
      seedQuery(RULES_KEY, [a, b, c]);
      let answer: (rules: Rule[]) => void = () => {};
      mocked.rulesReorder.mockReturnValueOnce(new Promise((resolve) => (answer = resolve)));
      const pending = state().reorderRules(["c", "a", "b"]);
      expect(ids()).toEqual(["c", "a", "b"]);
      answer([rule({ id: "c", position: 0 }), rule({ id: "a", position: 1 }), rule({ id: "b", position: 2 })]);
      await pending;
      expect(mocked.rulesReorder).toHaveBeenCalledWith(["c", "a", "b"]);
      expect((queryData<Rule[]>(RULES_KEY) ?? []).map((r) => r.position)).toEqual([0, 1, 2]);
    });

    it("restores the old order and toasts when the write fails", async () => {
      seedQuery(RULES_KEY, [a, b, c]);
      mocked.rulesReorder.mockRejectedValueOnce({ kind: "invalid_input", message: "Unknown rule", code: null });
      await state().reorderRules(["c", "a", "b"]);
      expect(ids()).toEqual(["a", "b", "c"]);
      expect(toastMessages()).toEqual(["Could not reorder the rules: Unknown rule"]);
    });
  });

  describe("moveRule", () => {
    it.each([
      ["down swaps a rule with the next", "a", 1, ["b", "a", "c"]],
      ["up swaps a rule with the previous", "c", -1, ["a", "c", "b"]],
    ] as const)("%s", async (_name, id, by, expected) => {
      seedQuery(RULES_KEY, [a, b, c]);
      await state().moveRule(id, by);
      expect(mocked.rulesReorder).toHaveBeenCalledWith(expected);
    });

    it.each([
      ["up from the first rule", "a", -1],
      ["down from the last rule", "c", 1],
      ["an unknown rule", "x", 1],
    ] as const)("does nothing for %s", async (_name, id, by) => {
      seedQuery(RULES_KEY, [a, b, c]);
      await state().moveRule(id, by);
      expect(mocked.rulesReorder).not.toHaveBeenCalled();
      expect(ids()).toEqual(["a", "b", "c"]);
    });
  });

  describe("setRuleEnabled", () => {
    it("replaces the rule with the saved one", async () => {
      seedQuery(RULES_KEY, [a, b]);
      mocked.ruleSetEnabled.mockResolvedValueOnce({ ...b, enabled: false });
      await state().setRuleEnabled("b", false);
      expect(mocked.ruleSetEnabled).toHaveBeenCalledWith("b", false);
      expect((queryData<Rule[]>(RULES_KEY) ?? []).map((r) => r.enabled)).toEqual([true, false]);
    });

    it("toasts when the change fails", async () => {
      seedQuery(RULES_KEY, [a]);
      mocked.ruleSetEnabled.mockRejectedValueOnce({ kind: "not_found", message: "Gone", code: null });
      await state().setRuleEnabled("a", false);
      expect(queryData<Rule[]>(RULES_KEY)?.[0].enabled).toBe(true);
      expect(toastMessages()).toEqual(["Could not change the rule: Gone"]);
    });
  });
});
