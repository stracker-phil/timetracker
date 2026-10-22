// Must not import the store or "../api" at runtime: apiMock.ts imports this file while the api module is mocked.
import type { Client, Project, ProjectListItem, TagListItem } from "../api/types";
import type { Store } from "../state/store";

/** An active project without a client; `patch` overrides fields. */
export function project(patch: Partial<Project> = {}): Project {
  return { id: "project-1", clientId: null, name: "Website", color: "blue", shade: 0, billable: false, archivedAt: null, createdAt: 0, ...patch };
}

/** An active client; `patch` overrides fields. */
export function client(patch: Partial<Client> = {}): Client {
  return { id: "client-1", name: "Acme", color: "blue", archivedAt: null, createdAt: 0, ...patch };
}

/** A listed project; `project` patches the project itself, the rest the list fields. */
export function projectItem(patch: Omit<Partial<ProjectListItem>, "project"> & { project?: Partial<Project> } = {}): ProjectListItem {
  const { project: projectPatch, ...rest } = patch;
  return { project: project(projectPatch), client: null, lastUsed: null, weekMs: 0, monthMs: 0, ...rest };
}

/** A tag used by no recording; `patch` overrides fields. */
export function tag(patch: Partial<TagListItem> = {}): TagListItem {
  const name = patch.name ?? "admin";
  return { id: `tag-${name}`, name, emoji: null, recordings: 0, createdAt: 0, ...patch };
}

export const DEFAULT_TAGS: TagListItem[] = ["admin", "research", "development", "maintenance", "internal meeting", "external meeting", "support"].map((name) => tag({ name }));

/** A store patch for `resetStore` with the projects slice loaded. */
export function projectsStore({ projects = [], clients = [], tags = DEFAULT_TAGS, recent = [] }: { projects?: ProjectListItem[]; clients?: Client[]; tags?: TagListItem[]; recent?: Project[] } = {}): Partial<Store> {
  return { projects, clients, tags, recent, projectsLoaded: true };
}
