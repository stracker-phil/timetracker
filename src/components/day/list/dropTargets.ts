import type { Project } from "../../../api";
import type { Store } from "../../../state/store";
import type { MenuEntry } from "../../ContextMenu";

/** Projects shown as chips; the rest are in the "More…" menu. */
export const VISIBLE_PROJECTS = 8;

/** Marks the "More…" chip as a drop target; dropping there opens the rest as a menu. */
export const MORE_TARGET = "more";

/** Active projects, recently used first. */
export const selectStripProjects = (s: Store): Project[] => {
  const active = s.projects.filter((p) => p.project.archivedAt === null).map((p) => p.project);
  const recent = s.recent.flatMap((r) => active.find((p) => p.id === r.id) ?? []);
  return [...recent, ...active.filter((p) => !recent.includes(p))];
};

/** The drop target under the pointer: a project id, `MORE_TARGET`, or null. */
export const dropTargetAt = (x: number, y: number): string | null =>
  document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-project-id]")?.dataset.projectId ?? null;

/** The overflow menu's items; disabled without `pick`. */
export const overflowItems = (projects: Project[], pick: ((project: Project) => void) | null): MenuEntry[] =>
  projects.slice(VISIBLE_PROJECTS).map((p) => ({ label: p.name, disabled: !pick, action: () => pick?.(p) }));
