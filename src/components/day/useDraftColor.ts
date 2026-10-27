import { useStore } from "../../state/store";
import { shadeColor } from "../Accent";

/** The open editor's project as its shade (`shadeColor`); null while none is picked. */
export function useDraftColor(): string | null {
  return useStore((s) => {
    const project = s.projects.find((p) => p.project.id === s.draft?.projectId)?.project;
    return project ? shadeColor(project.color, project.shade) : null;
  });
}
