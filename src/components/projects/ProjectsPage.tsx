import { Plus } from "lucide-react";
import { useEffect, useState, type MouseEvent } from "react";
import type { Client, ProjectListItem } from "../../api";
import { copyText } from "../../lib/clipboard";
import { formatHm, formatHours, plural, relativeTime } from "../../lib/format";
import { groupProjects } from "../../state/slices/projects";
import { useStore } from "../../state/store";
import { AccentDot } from "../Accent";
import { useContextMenu, type MenuEntry } from "../ContextMenu";
import { Toolbar } from "../Toolbar";
import { TagsSection } from "./TagsSection";

function Hours({ ms, label }: { ms: number; label: string }) {
  return (
    <span className={ms > 0 ? "project-hours" : "project-hours o3"} title={`${label}: ${formatHours(ms)}`}>
      {formatHm(ms)}
    </span>
  );
}

function ProjectRow({ item, onMenu }: { item: ProjectListItem; onMenu: (e: MouseEvent, item: ProjectListItem) => void }) {
  const { project } = item;
  const archived = project.archivedAt !== null;
  const open = () => useStore.getState().openDialog({ type: "project", id: project.id });
  return (
    <li className={archived ? "project-row dimmed" : "project-row"} onContextMenu={(e) => onMenu(e, item)}>
      <span className="project-name">
        <button type="button" className="project-open" onClick={open}>
          <AccentDot accent={project.color} shade={project.shade} />
          <span className="grow-text">{project.name}</span>
        </button>
        {project.billable && <span className="secondary-label">billable</span>}
        {item.lastUsed !== null && <span className="secondary-label project-last-used">last used {relativeTime(item.lastUsed)}</span>}
      </span>
      <Hours ms={item.weekMs} label="This week" />
      <Hours ms={item.monthMs} label="This month" />
      <span className="project-row-end">
        {archived && (
          <button type="button" className="btn" onClick={() => void useStore.getState().archiveProject(project.id, false).catch(() => {})}>
            Restore
          </button>
        )}
      </span>
    </li>
  );
}

function AddProjectButton({ client }: { client: Client | null }) {
  const label = client ? `New project for ${client.name}` : "New project without a client";
  return (
    <button type="button" className="icon-btn" aria-label={label} title={label} onClick={() => useStore.getState().openDialog({ type: "project", clientId: client?.id })}>
      <Plus size="1.077rem" />
    </button>
  );
}

function ClientHead({ client, count }: { client: Client | null; count: number }) {
  const archived = client !== null && client.archivedAt !== null;
  return (
    <header className="client-head">
      <span className="client-title">
        <h2 className="client-name">
          {client ? (
            <>
              <AccentDot accent={client.color} />
              <button type="button" className="client-edit" title="Edit client" onClick={() => useStore.getState().openDialog({ type: "client", id: client.id })}>
                {client.name}
              </button>
            </>
          ) : (
            "No client"
          )}
        </h2>
        {archived && <span className="secondary-label">archived</span>}
        <span className="o3">{plural(count, "project", "projects")}</span>
      </span>
      <span className="client-column">This week</span>
      <span className="client-column">This month</span>
      <span className="project-row-end">{!archived && <AddProjectButton client={client} />}</span>
    </header>
  );
}

/** Projects as a card per client; then the tags. */
export function ProjectsPage() {
  const projects = useStore((s) => s.projects);
  const clients = useStore((s) => s.clients);
  const loaded = useStore((s) => s.projectsLoaded);
  const changes = useStore((s) => s.changeCount);
  // The hours change with every recording.
  useEffect(() => void useStore.getState().loadProjects(), [changes]);
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
    <section className="view-page" aria-label="Projects">
      <Toolbar title="Projects">
        <label className="projects-archived">
          <input type="checkbox" className="switch" role="switch" checked={showArchived} onChange={(e) => setShowArchived(e.target.checked)} />
          Show archived
        </label>
        <button type="button" className="btn" onClick={() => openDialog({ type: "client" })}>
          <Plus size="1.154rem" /> New client
        </button>
        <button type="button" className="btn" onClick={() => openDialog({ type: "project" })}>
          <Plus size="1.154rem" /> New project
        </button>
      </Toolbar>
      <div className="projects-page">
        {loaded && groups.length === 0 && <p className="o3">No projects yet. A project is where recorded time goes.</p>}
        {groups.map((group) => (
          <section key={group.client?.id ?? ""} className="projects-card" aria-label={group.client?.name ?? "No client"}>
            <ClientHead client={group.client} count={group.projects.length} />
            {group.projects.length === 0 && <p className="o3 project-none">No projects</p>}
            <ul className="project-rows">
              {group.projects.map((item) => (
                <ProjectRow key={item.project.id} item={item} onMenu={openMenu} />
              ))}
            </ul>
          </section>
        ))}
        <TagsSection />
      </div>
      {menu.element}
    </section>
  );
}
