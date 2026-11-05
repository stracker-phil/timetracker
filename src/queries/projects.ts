import { queryOptions, useQuery } from "@tanstack/react-query";
import { api, type Client, type Project, type ProjectListItem, type TagListItem } from "../api";
import { queryClient } from "./queryClient";

const RECENT_LIMIT = 5;

/** The four lists the pickers, sheets and filters read, loaded together. */
export interface ProjectLists {
  /** Archived ones included; views filter. */
  projects: ProjectListItem[];
  /** Archived ones included. */
  clients: Client[];
  tags: TagListItem[];
  /** Active projects by latest recording. */
  recent: Project[];
}

export const PROJECT_LISTS_KEY = ["projectLists"] as const;

const EMPTY: ProjectLists = { projects: [], clients: [], tags: [], recent: [] };

export const projectListsQuery = queryOptions({
  queryKey: PROJECT_LISTS_KEY,
  queryFn: async (): Promise<ProjectLists> => {
    const [projects, clients, tags, recent] = await Promise.all([api.projectsList(true), api.clientsList(true), api.tagsList(), api.projectsRecent(RECENT_LIMIT)]);
    return { projects, clients, tags, recent };
  },
  // Read outside components too (`projectLists`), so kept while nothing shows them.
  gcTime: Infinity,
  meta: { failure: "Could not load the projects" },
});

/** The lists, empty until they have loaded; `loaded` once they have. */
export function useProjectLists(): ProjectLists & { loaded: boolean } {
  const { data } = useQuery(projectListsQuery);
  return { ...(data ?? EMPTY), loaded: data !== undefined };
}

export const useProjects = () => useProjectLists().projects;
export const useClients = () => useProjectLists().clients;
export const useTags = () => useProjectLists().tags;

/** The lists as loaded, for code outside components; empty before the first load. */
export const projectLists = (): ProjectLists => queryClient.getQueryData(PROJECT_LISTS_KEY) ?? EMPTY;
