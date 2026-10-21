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

const fieldLabel = (field: RuleField) => FIELDS.find((f) => f.id === field)?.label.toLowerCase() ?? field;
const opLabel = (op: RuleOp) => OPS.find((o) => o.id === op)?.label ?? op;

/** `app is "Slack" · title contains "standup"`. */
export function conditionSummary(conditions: (Condition | ConditionInput)[]): string {
  return conditions.map((c) => `${fieldLabel(c.field)} ${opLabel(c.op)} "${c.value}"`).join(" · ");
}

export const emptyCondition = (): ConditionInput => ({ field: "app", op: "is", value: "" });

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
