import { Building2, FolderKanban, Tag } from "lucide-react";
import type { Range, ReportFilter } from "../../api";
import { today } from "../../lib/day";
import { projectLabel } from "../../state/slices/projects";
import { useStore } from "../../state/store";
import { PopUpButton } from "../menu";
import { MultiSelect } from "./MultiSelect";

type RangeKind = Range["kind"];

const PRESETS: { value: RangeKind; label: string }[] = [
  { value: "today", label: "Today" },
  { value: "yesterday", label: "Yesterday" },
  { value: "thisWeek", label: "This week" },
  { value: "lastWeek", label: "Last week" },
  { value: "thisMonth", label: "This month" },
  { value: "lastMonth", label: "Last month" },
  { value: "custom", label: "Custom" },
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
      <PopUpButton aria-label="Period" value={range.kind} options={PRESETS} onChange={pick} />
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

/** The slim filter bar above the report. */
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
    </div>
  );
}
