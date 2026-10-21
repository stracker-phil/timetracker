import type { ReactNode } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { useBindingLabel } from "../../commands";
import { useStore } from "../../state/store";

const withKeys = (label: string, keys: string) => (keys ? `${label} (${keys})` : label);

/** ‹ Today › and the title of the Week and Month views; `unit` names what the arrows step by. */
export function CalendarHeader({ title, unit, current, children }: { title: string; unit: "week" | "month"; current: boolean; children?: ReactNode }) {
  const { step, goToday } = useStore.getState();
  const prevKeys = useBindingLabel("nav.prev");
  const nextKeys = useBindingLabel("nav.next");
  const todayKeys = useBindingLabel("nav.today");
  return (
    <header className="day-head">
      <button type="button" className="icon-btn" aria-label={`Previous ${unit}`} title={withKeys(`Previous ${unit}`, prevKeys)} onClick={() => step(-1)}>
        <ChevronLeft size="1.077rem" />
      </button>
      <button type="button" className="btn" disabled={current} title={withKeys("Today", todayKeys)} onClick={goToday}>
        Today
      </button>
      <button type="button" className="icon-btn" aria-label={`Next ${unit}`} title={withKeys(`Next ${unit}`, nextKeys)} onClick={() => step(1)}>
        <ChevronRight size="1.077rem" />
      </button>
      <h1 className="day-title">{title}</h1>
      {children}
    </header>
  );
}
