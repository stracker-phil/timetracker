import { GripVertical } from "lucide-react";
import { useRef, type CSSProperties, type PointerEvent } from "react";
import type { Rule, RuleDay } from "../../api";
import { formatShortDuration } from "../../lib/format";
import { useStore } from "../../state/store";
import { shadeColor } from "../Accent";
import { conditionSummary } from "./conditions";

interface Props {
  rule: Rule;
  /** Its matches today; absent for a rule that does not suggest. */
  today?: RuleDay;
  /** The row follows a drag by this many px. */
  offset: number;
  /** A press on the grip, with the row's height. */
  onGrip: (e: PointerEvent<HTMLElement>, rowHeight: number) => void;
}

/** "1h 20m today · 2 to review", "Nothing today", or "Off" for a disabled rule. */
function todayText(rule: Rule, today: RuleDay | undefined): string {
  if (!rule.enabled) return "Off";
  if (!today || today.matchedMs === 0) return "Nothing today";
  const matched = `${formatShortDuration(today.matchedMs)} today`;
  return today.pending === 0 ? matched : `${matched} · ${today.pending} to review`;
}

/** A rule as a card: its switch, name, project chip, conditions and today's matches; a click opens the sheet. */
export function RuleRow({ rule, today, offset, onGrip }: Props) {
  const project = useStore((s) => s.projects.find((p) => p.project.id === rule.projectId)?.project);
  const ref = useRef<HTMLLIElement>(null);
  const classes = ["rule-card", !rule.enabled && "dimmed", offset !== 0 && "dragging"];
  const pending = rule.enabled && (today?.pending ?? 0) > 0;
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
        <span className="rule-line">
          <span className="rule-name">{rule.name}</span>
          <span className={pending ? "rule-today pending" : "rule-today"}>{todayText(rule, today)}</span>
        </span>
        <span className="rule-line">
          <span className="project-chip rule-chip" style={project ? ({ "--rec": shadeColor(project.color, project.shade) } as CSSProperties) : undefined}>
            {project?.name ?? "Unknown project"}
          </span>
          <span className="rule-summary">{conditionSummary(rule.conditions)}</span>
        </span>
      </button>
    </li>
  );
}
