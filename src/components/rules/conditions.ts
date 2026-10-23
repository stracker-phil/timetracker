import type { ActivityBlock, Condition, ConditionInput, RuleField, RuleInput, RuleOp } from "../../api";

export const FIELDS: { id: RuleField; label: string }[] = [
  { id: "app", label: "App" },
  { id: "title", label: "Title" },
  { id: "url", label: "URL" },
  { id: "domain", label: "Domain" },
  { id: "path", label: "Path" },
  { id: "source", label: "Source" },
];

export const OPS: { id: RuleOp; label: string }[] = [
  { id: "contains", label: "contains" },
  { id: "is", label: "is" },
  { id: "begins_with", label: "begins with" },
  { id: "pattern", label: "matches" },
];

export const fieldLabel = (field: RuleField) => FIELDS.find((f) => f.id === field)?.label ?? field;
const opLabel = (op: RuleOp) => OPS.find((o) => o.id === op)?.label ?? op;

export interface ConditionGroup {
  field: RuleField;
  /** Each condition with its index in the rule's list. */
  items: { condition: ConditionInput; index: number }[];
}

/** The conditions by field, fields in order of first appearance: a field's conditions are alternatives. */
export function groupConditions(conditions: ConditionInput[]): ConditionGroup[] {
  const groups: ConditionGroup[] = [];
  conditions.forEach((condition, index) => {
    const group = groups.find((g) => g.field === condition.field);
    if (group) group.items.push({ condition, index });
    else groups.push({ field: condition.field, items: [{ condition, index }] });
  });
  return groups;
}

/** `title contains "a" or is "b"`. */
const groupText = ({ field, items }: ConditionGroup) => `${fieldLabel(field).toLowerCase()} ${items.map(({ condition: c }) => `${opLabel(c.op)} "${c.value}"`).join(" or ")}`;

/** `app is "Chrome" or is "Safari" · title contains "standup"`. */
export const conditionSummary = (conditions: (Condition | ConditionInput)[]) => groupConditions(conditions).map(groupText).join(" · ");

const EXACT_FIELDS: RuleField[] = ["app", "domain"];

export const emptyCondition = (field: RuleField = "app"): ConditionInput => ({
  field,
  op: EXACT_FIELDS.includes(field) ? "is" : "contains",
  value: "",
});

export const emptyRule = (): RuleInput => ({ name: "", projectId: "", enabled: true, note: "", tagId: null, conditions: [emptyCondition()] });

/** The host without `www.`, as rules match the domain. */
function domainOf(url: string): string | null {
  try {
    const host = new URL(url).hostname.toLowerCase();
    return host.replace(/^www\./, "") || null;
  } catch {
    return null;
  }
}

/** A rule for the block's activity: its app, and its domain or else its title; without a project, the sheet asks for one. */
export function draftFromBlock(block: ActivityBlock, project: { id: string; name: string } | null): RuleInput {
  const conditions: ConditionInput[] = [{ field: "app", op: "is", value: block.app }];
  const domain = block.url ? domainOf(block.url) : null;
  if (domain) conditions.push({ field: "domain", op: "is", value: domain });
  else if (block.title && !block.private) conditions.push({ field: "title", op: "contains", value: block.title });
  return { ...emptyRule(), name: project ? `${block.app} → ${project.name}` : block.app, projectId: project?.id ?? "", conditions };
}
