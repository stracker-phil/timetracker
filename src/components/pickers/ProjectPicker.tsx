import { Plus } from "lucide-react";
import { useState } from "react";
import type { Client, Project, ProjectListItem } from "../../api";
import { groupProjects, projectLabel, recentProjects, searchProjects } from "../../state/slices/projects";
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
  const rows: PickerRow<Choice>[] = [];
  if (name) {
    rows.push(...searchProjects(projects, name).map((p) => projectRow(p, "match")));
  } else {
    const recentItems = recentProjects(projects, recent).slice(0, RECENT_SHOWN);
    if (recentItems.length > 0) rows.push({ kind: "section", label: "Recent" }, ...recentItems.map((p) => projectRow(p, "recent")));
    for (const group of groupProjects(projects, clients, false)) {
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
  /** Lists the recent projects under the field while none is picked. */
  showRecent?: boolean;
}

/** Rows to click for an empty `ProjectPicker`; out of the tab order, as focus opens its list. */
function RecentProjects({ items, onPick }: { items: ProjectListItem[]; onPick: (projectId: string) => void }) {
  if (items.length === 0) return null;
  return (
    <ul className="recent-projects" aria-label="Recent projects">
      {items.map((item) => (
        <li key={item.project.id}>
          <button type="button" tabIndex={-1} className="recent-project" aria-label={projectLabel(item)} onClick={() => onPick(item.project.id)}>
            <AccentDot accent={item.project.color} shade={item.project.shade} />
            <span className="grow-text">{item.project.name}</span>
            {item.client && <span className="o3">{item.client.name}</span>}
          </button>
        </li>
      ))}
    </ul>
  );
}

/** Pick an active project, or create one (see `PickerField` for the keys). */
export function ProjectPicker({ value, onChange, autoFocus, showRecent }: Props) {
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
        openOnFocus={value === null}
        autoFocus={autoFocus}
      />
      {showRecent && value === null && <RecentProjects items={recentProjects(projects, recent).slice(0, RECENT_SHOWN)} onPick={onChange} />}
      {creating !== null && <ProjectSheet initialName={creating} onClose={() => setCreating(null)} onSaved={(project) => onChange(project.id)} />}
    </>
  );
}
