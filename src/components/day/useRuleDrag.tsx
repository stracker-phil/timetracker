import { useState, type PointerEvent as ReactPointerEvent, type RefObject } from "react";
import { createPortal } from "react-dom";
import { useShallow } from "zustand/react/shallow";
import type { ActivityBlock, Project } from "../../api";
import { useStore, type Store } from "../../state/store";
import { AccentDot } from "../Accent";
import { draftFromBlock } from "../rules/conditions";
import { pointerDrag } from "./pointerDrag";

const TARGETS = 5;

/** Recently used projects, else the first active ones. */
const selectTargets = (s: Store): Project[] =>
  s.recent.length > 0
    ? s.recent
    : s.projects
        .filter((p) => p.project.archivedAt === null)
        .slice(0, TARGETS)
        .map((p) => p.project);

const projectAt = (e: PointerEvent): string | null => {
  const target = document.elementFromPoint(e.clientX, e.clientY);
  return target?.closest<HTMLElement>("[data-rule-project]")?.dataset.ruleProject ?? null;
};

interface Drag {
  left: number;
  top: number;
  width: number;
  over: string | null;
}

/**
 * ⌥-dragging an activity block shows projects over the top of the recordings column; dropping on one opens a
 * new rule for the block's activity there. Nothing is saved before the sheet's Save.
 */
export function useRuleDrag(columnRef: RefObject<HTMLElement | null>, scrollRef: RefObject<HTMLElement | null>) {
  const targets = useStore(useShallow(selectTargets));
  const [drag, setDrag] = useState<Drag | null>(null);

  const start = (e: ReactPointerEvent<HTMLElement>, block: ActivityBlock) => {
    const column = columnRef.current?.getBoundingClientRect();
    const box = scrollRef.current?.getBoundingClientRect();
    if (!column || !box) return;
    e.preventDefault();
    setDrag({ left: column.left, top: Math.max(column.top, box.top), width: column.width, over: null });
    pointerDrag(e, {
      move: (_, ev) => setDrag((d) => d && { ...d, over: projectAt(ev) }),
      end: (_, dragged, ev) => {
        setDrag(null);
        const project = selectTargets(useStore.getState()).find((p) => p.id === projectAt(ev));
        if (dragged && project) useStore.getState().openDialog({ type: "rule", draft: draftFromBlock(block, project) });
      },
    });
  };

  const strip =
    drag &&
    createPortal(
      <div className="rule-drop-strip" role="list" aria-label="Drop on a project to make a rule" style={{ left: drag.left, top: drag.top, width: drag.width }}>
        {targets.length === 0 && <span className="o3">No projects yet</span>}
        {targets.map((p) => (
          <div key={p.id} role="listitem" className={drag.over === p.id ? "rule-drop over" : "rule-drop"} data-rule-project={p.id}>
            <AccentDot accent={p.color} shade={p.shade} />
            <span className="grow-text">{p.name}</span>
          </div>
        ))}
      </div>,
      document.body,
    );

  return { start, strip };
}
