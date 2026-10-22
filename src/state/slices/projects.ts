import { api, toAppError, type Client, type ClientInput, type Project, type ProjectInput, type ProjectListItem, type Tag, type TagListItem } from "../../api";
import { fuzzyFilter } from "../../lib/fuzzy";
import { fail } from "../requests";
import type { Slice } from "../store";

const RECENT_LIMIT = 5;

export interface ProjectsSlice {
  /** Archived ones included; views filter. */
  projects: ProjectListItem[];
  /** Archived ones included. */
  clients: Client[];
  tags: TagListItem[];
  /** Active projects by latest recording. */
  recent: Project[];
  projectsLoaded: boolean;
  /** Loads all four lists; runs at boot and on every `data_changed` of projects or tags. */
  loadProjects: () => Promise<void>;
  /** `id: null` creates. Rejects on failure: a field error for the form to show, anything else after a toast. */
  saveProject: (id: string | null, input: ProjectInput) => Promise<Project>;
  archiveProject: (id: string, archived: boolean) => Promise<void>;
  saveClient: (id: string | null, input: ClientInput) => Promise<Client>;
  archiveClient: (id: string, archived: boolean) => Promise<void>;
  /** An existing name returns that tag. */
  saveTag: (id: string | null, name: string) => Promise<Tag>;
  deleteTag: (id: string) => Promise<void>;
}

export const projectsSlice: Slice<ProjectsSlice> = {
  create: (set, get) => {
    /** Runs a write and settles once the lists have reloaded; a failure is toasted under `context` unless a form field shows it. */
    async function write<T>(context: string, run: () => Promise<T>): Promise<T> {
      try {
        return await run();
      } catch (e) {
        if (!toAppError(e).field) fail(e, context);
        throw e;
      } finally {
        // A reload failure is toasted on its own and never replaces the write's result or error.
        await get()
          .loadProjects()
          .catch((e: unknown) => fail(e, "Could not reload the projects"));
      }
    }

    return {
      projects: [],
      clients: [],
      tags: [],
      recent: [],
      projectsLoaded: false,

      async loadProjects() {
        try {
          const [projects, clients, tags, recent] = await Promise.all([api.projectsList(true), api.clientsList(true), api.tagsList(), api.projectsRecent(RECENT_LIMIT)]);
          set({ projects, clients, tags, recent, projectsLoaded: true });
        } catch (e) {
          fail(e, "Could not load the projects");
        }
      },

      saveProject: (id, input) => write("Could not save the project", () => api.projectsSave(id, input)),
      archiveProject: (id, archived) => write(archived ? "Could not archive the project" : "Could not restore the project", () => api.projectsArchive(id, archived)).then(() => {}),
      saveClient: (id, input) => write("Could not save the client", () => api.clientsSave(id, input)),
      archiveClient: (id, archived) => write(archived ? "Could not archive the client" : "Could not restore the client", () => api.clientsArchive(id, archived)).then(() => {}),
      saveTag: (id, name) => write("Could not save the tag", () => api.tagsSave(id, name)),
      deleteTag: (id) => write("Could not delete the tag", () => api.tagsDelete(id)),
    };
  },
};

export interface ProjectGroup {
  /** `null`: projects without a client. */
  client: Client | null;
  projects: ProjectListItem[];
}

/**
 * Projects grouped by client, clients by name, every client listed even without projects; client-less
 * projects come last. `archived` includes archived clients and projects.
 */
export function groupProjects(projects: ProjectListItem[], clients: Client[], archived: boolean): ProjectGroup[] {
  const byName = (a: string, b: string) => a.localeCompare(b, undefined, { sensitivity: "base" });
  const shown = projects.filter((p) => archived || p.project.archivedAt === null).sort((a, b) => byName(a.project.name, b.project.name));
  const groups: ProjectGroup[] = clients
    .filter((c) => archived || c.archivedAt === null)
    .sort((a, b) => byName(a.name, b.name))
    .map((client) => ({ client, projects: shown.filter((p) => p.project.clientId === client.id) }));
  const clientless = shown.filter((p) => p.project.clientId === null);
  if (clientless.length > 0) groups.push({ client: null, projects: clientless });
  return groups;
}

/** `Client / Project`, or the project's name without a client. */
export const projectLabel = (item: ProjectListItem) => (item.client ? `${item.client.name} / ${item.project.name}` : item.project.name);

/** Active projects matching `query` by `Client / Name` or name, best first. */
export function searchProjects(projects: ProjectListItem[], query: string): ProjectListItem[] {
  const active = projects.filter((p) => p.project.archivedAt === null);
  return fuzzyFilter(active, query, (p) => [projectLabel(p), p.project.name]);
}

/** The active projects among `recent`, most recent first. */
export function recentProjects(projects: ProjectListItem[], recent: Project[]): ProjectListItem[] {
  return recent.flatMap((r) => projects.filter((p) => p.project.id === r.id && p.project.archivedAt === null));
}
