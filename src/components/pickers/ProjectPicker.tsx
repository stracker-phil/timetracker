import { Plus, Search } from "lucide-react";
import { useRef, useState } from "react";
import type { Client, Project, ProjectListItem } from "../../api";
import { groupProjects, projectLabel, searchProjects } from "../../state/slices/projects";
import { useStore } from "../../state/store";
import { AccentDot } from "../Accent";
import { InsetField } from "../Field";
import { ProjectSheet } from "../projects/ProjectSheet";
import { listKeyDown, optionsOf, PickerList, usePickerIndex, type PickerRow } from "./PickerList";

const RECENT_SHOWN = 5;

type Choice = { kind: "project"; id: string } | { kind: "new" };

function projectRow(item: ProjectListItem, keyPrefix: string): PickerRow<Choice> {
  return {
    kind: "option",
    key: `${keyPrefix}:${item.project.id}`,
    value: { kind: "project", id: item.project.id },
    content: (
      <>
        <AccentDot accent={item.project.color} />
        <span className="grow-text">{item.project.name}</span>
        {item.client && <span className="o3">{item.client.name}</span>}
      </>
    ),
  };
}

/** Recent and all projects by client while the query is empty; else the matches, best first. Always ends with "New project". */
function rowsFor(query: string, projects: ProjectListItem[], recent: Project[], clients: Client[]): PickerRow<Choice>[] {
  const name = query.trim();
  const active = projects.filter((p) => p.project.archivedAt === null);
  const rows: PickerRow<Choice>[] = [];
  if (name) {
    rows.push(...searchProjects(projects, name).map((p) => projectRow(p, "match")));
  } else {
    const recentItems = recent.flatMap((r) => active.filter((p) => p.project.id === r.id)).slice(0, RECENT_SHOWN);
    if (recentItems.length > 0) rows.push({ kind: "section", label: "Recent" }, ...recentItems.map((p) => projectRow(p, "recent")));
    for (const group of groupProjects(active, clients, false)) {
      if (group.projects.length === 0) continue;
      rows.push({ kind: "section", label: group.client?.name ?? "No client" }, ...group.projects.map((p) => projectRow(p, "all")));
    }
  }
  rows.push({
    kind: "option",
    key: "new",
    value: { kind: "new" },
    content: (
      <>
        <Plus size="1rem" aria-hidden="true" />
        <span className="grow-text">{name ? `New project "${name}"…` : "New project…"}</span>
      </>
    ),
  });
  return rows;
}

interface Props {
  value: string | null;
  onChange: (projectId: string) => void;
  autoFocus?: boolean;
}

/** Search and pick an active project, or create one; ↑ ↓ move, Enter picks, Esc closes the list. */
export function ProjectPicker({ value, onChange, autoFocus }: Props) {
  const projects = useStore((s) => s.projects);
  const clients = useStore((s) => s.clients);
  const recent = useStore((s) => s.recent);
  const chosen = projects.find((p) => p.project.id === value);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [creating, setCreating] = useState<string | null>(null);
  const fieldRef = useRef<HTMLDivElement>(null);
  const rows = rowsFor(query, projects, recent, clients);
  const options = optionsOf(rows);
  const [current, setIndex] = usePickerIndex(options.length);

  const close = () => {
    setOpen(false);
    setQuery("");
    setIndex(0);
  };
  const pick = (choice: Choice) => {
    if (choice.kind === "new") setCreating(query.trim());
    else onChange(choice.id);
    close();
  };

  return (
    <div className="picker" ref={fieldRef}>
      <InsetField icon={chosen && !open ? <AccentDot accent={chosen.project.color} /> : <Search size="1.077rem" />}>
        <input
          aria-label="Project"
          role="combobox"
          aria-expanded={open}
          autoFocus={autoFocus}
          placeholder="Project"
          value={open ? query : chosen ? projectLabel(chosen) : ""}
          onFocus={() => setOpen(true)}
          onBlur={close}
          onChange={(e) => {
            setOpen(true);
            setQuery(e.target.value);
            setIndex(0);
          }}
          onKeyDown={(e) => {
            if (listKeyDown(e, { current, count: options.length, open, setIndex, setOpen: (o) => (o ? setOpen(true) : close()) })) return;
            if (e.key === "Enter" && open) {
              e.preventDefault();
              pick(options[current]);
            }
          }}
        />
      </InsetField>
      {open && <PickerList anchor={fieldRef} label="Projects" rows={rows} active={current} onHover={setIndex} onPick={pick} />}
      {creating !== null && <ProjectSheet initialName={creating} onClose={() => setCreating(null)} onSaved={(project) => onChange(project.id)} />}
    </div>
  );
}
