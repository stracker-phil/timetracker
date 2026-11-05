import { useStore } from "../../state/store";
import { shadeColor } from "../Accent";
import { useProjects } from "../../queries/projects";

/** The open editor's project as its shade (`shadeColor`); null while none is picked. */
export function useDraftColor(): string | null {
  const projectId = useStore((s) => s.draft?.projectId);
  const project = useProjects().find((p) => p.project.id === projectId)?.project;
  return project ? shadeColor(project.color, project.shade) : null;
}
