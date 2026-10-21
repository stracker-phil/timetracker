import { Plus } from "lucide-react";
import { useState, type MouseEvent } from "react";
import type { Client, ProjectListItem } from "../../api";
import { copyText } from "../../lib/clipboard";
import { formatHours, relativeTime } from "../../lib/format";
import { groupProjects } from "../../state/slices/projects";
import { useStore } from "../../state/store";
import { AccentDot } from "../Accent";
import { useContextMenu, type MenuEntry } from "../ContextMenu";
import { TagsSection } from "./TagsSection";

function ProjectRow({ item, onMenu }: { item: ProjectListItem; onMenu: (e: MouseEvent, item: ProjectListItem) => void }) {
  const { project } = item;
  const archived = project.archivedAt !== null;
  const open = () => useStore.getState().openDialog({ type: "project", id: project.id });
  return (
    <li className={archived ? "row project-row dimmed" : "row project-row"} onContextMenu={(e) => onMenu(e, item)}>
      <button type="button" className="project-open" onClick={open}>
        <AccentDot accent={project.color} />
        <span className="grow-text">{project.name}</span>
      </button>
      <span className="project-meta mono o2">
        {item.lastUsed !== null && <span>last used {relativeTime(item.lastUsed)}</span>}
        <span>{formatHours(item.recordedMs30d)} / 30 d</span>
      </span>
      {archived && (
        <button type="button" className="btn" onClick={() => void useStore.getState().archiveProject(project.id, false).catch(() => {})}>
          Restore
        </button>
      )}
    </li>
  );
}

function AddProjectButton({ client }: { client: Client | null }) {
  const label = client ? `New project for ${client.name}` : "New project without a client";
  return (
    <button type="button" className="icon-btn band-add" aria-label={label} title={label} onClick={() => useStore.getState().openDialog({ type: "project", clientId: client?.id })}>
      <Plus size="1rem" />
    </button>
  );
}

function ClientBand({ client }: { client: Client | null }) {
  const archived = client !== null && client.archivedAt !== null;
  return (
    <h2 className={archived ? "band dimmed" : "band"}>
      {client ? (
        <button type="button" className="band-button" title="Edit client" onClick={() => useStore.getState().openDialog({ type: "client", id: client.id })}>
          {client.name}
        </button>
      ) : (
        "No client"
      )}
      {archived ? <span className="o3">archived</span> : <AddProjectButton client={client} />}
    </h2>
  );
}

/** Projects grouped by client, archived ones on request, then the tags. */
export function ProjectsPage() {
  const projects = useStore((s) => s.projects);
  const clients = useStore((s) => s.clients);
  const loaded = useStore((s) => s.projectsLoaded);
  const [showArchived, setShowArchived] = useState(false);
  const menu = useContextMenu();
  const groups = groupProjects(projects, clients, showArchived);
  const { openDialog, archiveProject } = useStore.getState();

  const openMenu = (e: MouseEvent, { project }: ProjectListItem) => {
    const archived = project.archivedAt !== null;
    const items: MenuEntry[] = [
      { label: "Edit…", action: () => openDialog({ type: "project", id: project.id }) },
      { label: archived ? "Restore" : "Archive", action: () => void archiveProject(project.id, !archived).catch(() => {}) },
      "separator",
      { label: "Copy id", action: () => void copyText(project.id, "the project id") },
    ];
    menu.open(e, items);
  };

  return (
    <section className="projects-page" aria-label="Projects">
      <header className="projects-head">
        <h1>Projects</h1>
        <button type="button" className="btn" onClick={() => openDialog({ type: "project" })}>
          <Plus size="1.077rem" /> New project
        </button>
        <button type="button" className="btn" onClick={() => openDialog({ type: "client" })}>
          <Plus size="1.077rem" /> New client
        </button>
        <label className="projects-archived">
          <input type="checkbox" className="switch" role="switch" checked={showArchived} onChange={(e) => setShowArchived(e.target.checked)} />
          Show archived
        </label>
      </header>
      {loaded && groups.length === 0 && <p className="o3">No projects yet. A project is where recorded time goes.</p>}
      {groups.map((group) => (
        <section key={group.client?.id ?? ""} className="projects-group" aria-label={group.client?.name ?? "No client"}>
          <ClientBand client={group.client} />
          {group.projects.length === 0 && <p className="o3 project-none">No projects</p>}
          <ul className="project-rows">
            {group.projects.map((item) => (
              <ProjectRow key={item.project.id} item={item} onMenu={openMenu} />
            ))}
          </ul>
        </section>
      ))}
      <TagsSection />
      {menu.element}
    </section>
  );
}
