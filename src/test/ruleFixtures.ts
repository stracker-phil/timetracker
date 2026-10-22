// Must not import the store or "../api" at runtime: apiMock.ts imports this file while the api module is mocked.
import type { Condition, Rule, RuleDay, RulePreview, Suggestion } from "../api/types";
import { at, block } from "./dayFixtures";

/** An enabled rule `app is Slack` for the default project; `patch` overrides fields. */
export function rule(patch: Partial<Rule> = {}): Rule {
  const id = patch.id ?? "rule-1";
  const condition: Condition = { id: `${id}-c1`, field: "app", op: "is", value: "Slack" };
  return { id, name: "Slack", projectId: "project-1", position: 0, enabled: true, note: "", tagId: null, conditions: [condition], ...patch };
}

/** A suggestion for the default project from 09:00 to 09:45 of `DAY`; `patch` overrides fields. */
export function suggestion(patch: Partial<Suggestion> = {}): Suggestion {
  const start = patch.start ?? at("09:00");
  const end = patch.end ?? at("09:45");
  const ruleId = patch.ruleId ?? "rule-1";
  return {
    id: `${ruleId}:${start}:${end}`,
    ruleId,
    ruleName: "Slack",
    projectId: "project-1",
    projectName: "Website",
    clientName: null,
    color: "blue",
    shade: 0,
    start,
    end,
    durationMs: end - start,
    note: "",
    tagId: null,
    tag: null,
    nextCard: null,
    ...patch,
  };
}

/** Today of the default rule without matches; `patch` overrides fields. */
export function ruleDay(patch: Partial<RuleDay> = {}): RuleDay {
  return { ruleId: "rule-1", matchedMs: 0, pending: 0, pendingMs: 0, ...patch };
}

/** A preview without hits; `patch` overrides fields. */
export function rulePreview(patch: Partial<RulePreview> = {}): RulePreview {
  return { hits: [], totalMs: 0, days: 0, ...patch };
}

/** A preview hit on `day`. */
export function previewHit(day: string, patch: Parameters<typeof block>[0] = {}) {
  return { day, block: block(patch) };
}
