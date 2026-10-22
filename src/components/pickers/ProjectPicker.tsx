import { Plus } from "lucide-react";
import { useState } from "react";
import type { Client, Project, ProjectListItem } from "../../api";
import { groupProjects, projectLabel, searchProjects } from "../../state/slices/projects";
import { useStore } from "../../state/store";
import { AccentDot } from "../Accent";
import { ProjectSheet } from "../projects/ProjectSheet";
import { PickerField } from "./PickerField";
import type { PickerRow } from "./PickerList";

const RECENT_SHOWN = 5;

type Choice = { kind: "project"; id: string } | { kind: "new" };

function projectRow(item: ProjectListItem, keyPrefix: string): PickerRow<Choice> {
  return {
    kind: "option",
    key: `${keyPrefix}:${item.project.id}`,
    value: { kind: "project", id: item.project.id },
    content: (
      <>
        <AccentDot accent={item.project.color} shade={item.project.shade} />
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

/** Pick an active project, or create one (see `PickerField` for the keys). */
export function ProjectPicker({ value, onChange, autoFocus }: Props) {
  const projects = useStore((s) => s.projects);
  const clients = useStore((s) => s.clients);
  const recent = useStore((s) => s.recent);
  const chosen = projects.find((p) => p.project.id === value);
  const [creating, setCreating] = useState<string | null>(null);

  const pick = (choice: Choice, query: string) => {
    if (choice.kind === "new") setCreating(query.trim());
    else onChange(choice.id);
  };

  return (
    <>
      <PickerField
        label="Project"
        listLabel="Projects"
        icon={chosen ? <AccentDot accent={chosen.project.color} shade={chosen.project.shade} /> : <span className="accent-dot accent-dot--none" />}
        text={chosen ? projectLabel(chosen) : null}
        placeholder="No project"
        rowsFor={(query) => rowsFor(query, projects, recent, clients)}
        isCurrent={(choice) => choice.kind === "project" && choice.id === value}
        onPick={pick}
        autoFocus={autoFocus}
      />
      {creating !== null && <ProjectSheet initialName={creating} onClose={() => setCreating(null)} onSaved={(project) => onChange(project.id)} />}
    </>
  );
}
