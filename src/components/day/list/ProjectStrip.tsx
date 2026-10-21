import type { CSSProperties } from "react";
import { useShallow } from "zustand/react/shallow";
import type { Project } from "../../../api";
import { useStore } from "../../../state/store";
import { shadeColor } from "../../Accent";
import type { MenuAnchor, MenuEntry } from "../../ContextMenu";
import { MORE_TARGET, VISIBLE_PROJECTS, overflowItems, selectStripProjects } from "./dropTargets";

interface Props {
  /** The drop target the dragged row is over. */
  over: string | null;
  /** Records the focused row for a project; null without one. */
  onPick: ((project: Project) => void) | null;
  openMenu: (at: MenuAnchor, items: MenuEntry[]) => void;
}

/** Projects to drop a row on; clicking one records the focused row. */
export function ProjectStrip({ over, onPick, openMenu }: Props) {
  const projects = useStore(useShallow(selectStripProjects));
  if (projects.length === 0) return <p className="list-strip o3">No projects yet</p>;
  const hint = onPick ? undefined : "Drag a row here, or focus one first";
  const chipClass = (target: string, more = false) => ["project-chip", more && "more", over === target && "over"].filter(Boolean).join(" ");
  return (
    <div className="list-strip" role="group" aria-label="Projects">
      {projects.slice(0, VISIBLE_PROJECTS).map((p) => (
        <button
          key={p.id}
          type="button"
          className={chipClass(p.id)}
          data-project-id={p.id}
          title={hint}
          style={{ "--rec": shadeColor(p.color, p.shade) } as CSSProperties}
          onClick={() => onPick?.(p)}
        >
          <span className="grow-text">{p.name}</span>
        </button>
      ))}
      {projects.length > VISIBLE_PROJECTS && (
        <button
          type="button"
          className={chipClass(MORE_TARGET, true)}
          data-project-id={MORE_TARGET}
          onClick={(e) => {
            const { left, bottom } = e.currentTarget.getBoundingClientRect();
            openMenu({ x: left, y: bottom }, overflowItems(projects, onPick));
          }}
        >
          More…
        </button>
      )}
    </div>
  );
}
