import type { ReactElement } from "react";
import type { QueryKey } from "@tanstack/react-query";
import { render, renderHook, type RenderOptions } from "@testing-library/react";
import type { Client, Project, ProjectListItem, TagListItem } from "../api/types";
import { QueryRoot } from "../queries/QueryRoot";
import { PROJECT_LISTS_KEY, type ProjectLists } from "../queries/projects";
import { queryClient } from "../queries/queryClient";
import { DEFAULT_TAGS } from "./projectFixtures";

export { dayKey } from "../queries/day";
export { PROJECT_LISTS_KEY } from "../queries/projects";
export { RULES_KEY } from "../queries/rules";
export { SAVED_REPORTS_KEY } from "../queries/reports";
export { SETTINGS_KEY } from "../queries/settings";
export { TIMER_KEY, TRACKER_KEY } from "../queries/status";

/** `render` inside the window's `QueryRoot`, as the window roots do. */
export function renderWithClient(ui: ReactElement, options?: Omit<RenderOptions, "wrapper">) {
  return render(ui, { ...options, wrapper: QueryRoot });
}

/** `renderHook` inside the `QueryRoot`: keeps a query active, so an invalidation refetches it (an unobserved query is only marked stale). */
export function renderHookWithClient<T>(hook: () => T) {
  return renderHook(hook, { wrapper: QueryRoot });
}

/** Puts `data` into the shared query client under `key`, as if the read had loaded it. */
export function seedQuery(key: QueryKey, data: unknown) {
  queryClient.setQueryData(key, data);
}

/** Seeds the project lists (`PROJECT_LISTS_KEY`); the tags default to `DEFAULT_TAGS`. */
export function seedProjects({ projects = [], clients = [], tags = DEFAULT_TAGS, recent = [] }: { projects?: ProjectListItem[]; clients?: Client[]; tags?: TagListItem[]; recent?: Project[] } = {}) {
  const lists: ProjectLists = { projects, clients, tags, recent };
  seedQuery(PROJECT_LISTS_KEY, lists);
}

/** What the shared client holds under `key`. */
export const queryData = <T,>(key: QueryKey) => queryClient.getQueryData<T>(key);
