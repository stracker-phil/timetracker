import { GripVertical } from "lucide-react";
import { useRef, type PointerEvent } from "react";
import type { Rule } from "../../api";
import { useStore } from "../../state/store";
import { AccentDot } from "../Accent";
import { conditionSummary } from "./conditions";

interface Props {
  rule: Rule;
  /** The row follows a drag by this many px. */
  offset: number;
  /** A press on the grip, with the row's height. */
  onGrip: (e: PointerEvent<HTMLElement>, rowHeight: number) => void;
}

export function RuleRow({ rule, offset, onGrip }: Props) {
  const project = useStore((s) => s.projects.find((p) => p.project.id === rule.projectId)?.project);
  const ref = useRef<HTMLLIElement>(null);
  const classes = ["row", "rule-row", !rule.enabled && "dimmed", offset !== 0 && "dragging"];
  return (
    <li ref={ref} className={classes.filter(Boolean).join(" ")} data-rule-id={rule.id} style={offset ? { transform: `translateY(${offset}px)` } : undefined}>
      <span className="rule-grip" aria-label={`Drag to reorder ${rule.name}`} onPointerDown={(e) => onGrip(e, ref.current?.getBoundingClientRect().height ?? 0)}>
        <GripVertical size="1rem" aria-hidden="true" />
      </span>
      <input
        className="switch"
        type="checkbox"
        role="switch"
        aria-label={`Suggest from ${rule.name}`}
        checked={rule.enabled}
        onChange={(e) => void useStore.getState().setRuleEnabled(rule.id, e.target.checked)}
      />
      <button type="button" className="rule-open" onClick={() => useStore.getState().openDialog({ type: "rule", id: rule.id })}>
        <span className="rule-name">{rule.name}</span>
        <span className="rule-project">
          {project && <AccentDot accent={project.color} shade={project.shade} />}
          {project?.name ?? "Unknown project"}
        </span>
        <span className="rule-summary o2 grow-text">{conditionSummary(rule.conditions)}</span>
      </button>
    </li>
  );
}
