import { Plus } from "lucide-react";
import { useEffect, useState, type PointerEvent } from "react";
import { useStore } from "../../state/store";
import { pointerDrag } from "../day/pointerDrag";
import { Toolbar } from "../Toolbar";
import { RuleRow } from "./RuleRow";

/** The row a drag of `dy` px from `index` lands on, within the list. */
const targetIndex = (index: number, dy: number, rowHeight: number, count: number) => Math.max(0, Math.min(count - 1, index + Math.round(dy / rowHeight)));

function moved(ids: string[], from: number, to: number): string[] {
  const next = [...ids];
  const [id] = next.splice(from, 1);
  next.splice(to, 0, id);
  return next;
}

/** The rules in the order they are tried: the first that matches an activity suggests its project. */
export function RulesView() {
  const rules = useStore((s) => s.rules);
  const today = useStore((s) => s.rulesToday);
  const [drag, setDrag] = useState<{ id: string; dy: number } | null>(null);

  useEffect(() => {
    void useStore.getState().loadRules();
  }, []);

  const grip = (index: number) => (e: PointerEvent<HTMLElement>, rowHeight: number) => {
    if (e.button !== 0 || rowHeight <= 0) return;
    const id = rules[index].id;
    pointerDrag(e, {
      move: (dy) => setDrag({ id, dy }),
      end: (dy, dragged) => {
        setDrag(null);
        const to = targetIndex(index, dy, rowHeight, rules.length);
        if (dragged && to !== index) {
          void useStore.getState().reorderRules(
            moved(
              rules.map((r) => r.id),
              index,
              to,
            ),
          );
        }
      },
    });
  };

  return (
    <section className="view-page" aria-label="Rules">
      <Toolbar title="Rules">
        <button type="button" className="btn" onClick={() => useStore.getState().openDialog({ type: "rule" })}>
          <Plus size="1.154rem" /> New rule
        </button>
      </Toolbar>
      <div className="rules-page">
        <p className="rules-intro">
          Rules suggest recordings for activity they match; the first matching rule wins. Suggestions wait in the Day view until you approve or dismiss them. ⌥-drag an activity block onto a project to make a rule.
        </p>
        {rules.length === 0 ? (
          <p className="rules-empty">No rules yet.</p>
        ) : (
          <ul className="rule-rows" aria-label="Rules in order">
            {rules.map((rule, index) => (
              <RuleRow key={rule.id} rule={rule} today={today.find((t) => t.ruleId === rule.id)} offset={drag?.id === rule.id ? drag.dy : 0} onGrip={grip(index)} />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
