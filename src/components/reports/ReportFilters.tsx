import { Building2, FolderKanban, Tag } from "lucide-react";
import type { Group, Range, ReportFilter, Rounding } from "../../api";
import { today } from "../../lib/day";
import { projectLabel } from "../../state/slices/projects";
import { useStore } from "../../state/store";
import { SegmentedButtons } from "../settings/controls";
import { MultiSelect } from "./MultiSelect";

type RangeKind = Range["kind"];

const PRESETS: { kind: RangeKind; label: string }[] = [
  { kind: "today", label: "Today" },
  { kind: "yesterday", label: "Yesterday" },
  { kind: "thisWeek", label: "This week" },
  { kind: "lastWeek", label: "Last week" },
  { kind: "thisMonth", label: "This month" },
  { kind: "lastMonth", label: "Last month" },
  { kind: "custom", label: "Custom" },
];

const GROUPS: readonly Group[] = ["project", "tag", "day", "week", "month"];

const ROUNDING_STEPS = [5, 6, 10, 15, 30] as const;
const ROUNDINGS: { value: Rounding; label: string }[] = [
  { value: "none", label: "No rounding" },
  ...ROUNDING_STEPS.map((m) => ({ value: `nearest${m}` satisfies Rounding, label: `Nearest ${m} min` })),
  ...ROUNDING_STEPS.map((m) => ({ value: `up${m}` satisfies Rounding, label: `Up to ${m} min` })),
];

interface Props {
  filter: ReportFilter;
  /** The days the report shows; a switch to Custom starts from them. */
  shown: { from: string; to: string } | null;
  onChange: (patch: Partial<ReportFilter>) => void;
}

function RangePicker({ range, shown, onChange }: { range: Range; shown: Props["shown"]; onChange: (range: Range) => void }) {
  const pick = (kind: RangeKind) => {
    if (kind !== "custom") return onChange({ kind });
    const day = today();
    onChange({ kind: "custom", from: shown?.from ?? day, to: shown?.to ?? day });
  };
  const setBound = (bound: "from" | "to", day: string) => range.kind === "custom" && day && onChange({ ...range, [bound]: day });
  return (
    <div className="report-range">
      <div className="segmented" role="group" aria-label="Range">
        {PRESETS.map((p) => (
          <button key={p.kind} type="button" aria-pressed={range.kind === p.kind} onClick={() => pick(p.kind)}>
            {p.label}
          </button>
        ))}
      </div>
      {range.kind === "custom" && (
        <>
          <input className="input" type="date" aria-label="From" value={range.from} max={range.to} onChange={(e) => setBound("from", e.target.value)} />
          <span className="o3">–</span>
          <input className="input" type="date" aria-label="To" value={range.to} min={range.from} onChange={(e) => setBound("to", e.target.value)} />
        </>
      )}
    </div>
  );
}

export function ReportFilters({ filter, shown, onChange }: Props) {
  const projects = useStore((s) => s.projects);
  const clients = useStore((s) => s.clients);
  const tags = useStore((s) => s.tags);
  return (
    <div className="report-filters">
      <RangePicker range={filter.range} shown={shown} onChange={(range) => onChange({ range })} />
      <div className="report-pickers">
        <MultiSelect
          label="Clients"
          icon={<Building2 size="1.077rem" />}
          items={clients.map((c) => ({ id: c.id, label: c.name, accent: c.color }))}
          value={filter.clientIds}
          onChange={(clientIds) => onChange({ clientIds })}
        />
        <MultiSelect
          label="Projects"
          icon={<FolderKanban size="1.077rem" />}
          items={projects.map((p) => ({ id: p.project.id, label: projectLabel(p), accent: p.project.color, shade: p.project.shade }))}
          value={filter.projectIds}
          onChange={(projectIds) => onChange({ projectIds })}
        />
        <MultiSelect label="Tags" icon={<Tag size="1.077rem" />} items={tags.map((t) => ({ id: t.id, label: t.name }))} value={filter.tagIds} onChange={(tagIds) => onChange({ tagIds })} />
      </div>
      <div className="report-options">
        <span className="o3">Group by</span>
        <SegmentedButtons label="Group by" options={GROUPS} value={filter.groupBy} onChange={(groupBy) => onChange({ groupBy })} />
        <select className="input" aria-label="Rounding" value={filter.rounding} onChange={(e) => onChange({ rounding: e.target.value as Rounding })}>
          {ROUNDINGS.map((r) => (
            <option key={r.value} value={r.value}>
              {r.label}
            </option>
          ))}
        </select>
        <label className="inline-switch">
          <input className="switch" type="checkbox" role="switch" checked={filter.includeArchived} onChange={(e) => onChange({ includeArchived: e.target.checked })} />
          Archived projects
        </label>
      </div>
    </div>
  );
}
