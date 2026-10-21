import { ChevronLeft, ChevronRight } from "lucide-react";
import type { DayTotals } from "../../api";
import { useBindingLabel } from "../../commands";
import { formatDayLabel, isToday } from "../../lib/day";
import { formatHm } from "../../lib/format";
import { useStore } from "../../state/store";

const withKeys = (label: string, keys: string) => (keys ? `${label} (${keys})` : label);

/** Day navigation, the date and the day's totals. */
export function DayHeader({ day, totals }: { day: string; totals: DayTotals | null }) {
  const { prevDay, nextDay, goToday } = useStore.getState();
  const prevKeys = useBindingLabel("nav.prevDay");
  const nextKeys = useBindingLabel("nav.nextDay");
  const todayKeys = useBindingLabel("nav.today");
  return (
    <header className="day-head">
      <button type="button" className="icon-btn" aria-label="Previous day" title={withKeys("Previous day", prevKeys)} onClick={prevDay}>
        <ChevronLeft size="1.077rem" />
      </button>
      <button type="button" className="btn" disabled={isToday(day)} title={withKeys("Today", todayKeys)} onClick={goToday}>
        Today
      </button>
      <button type="button" className="icon-btn" aria-label="Next day" title={withKeys("Next day", nextKeys)} onClick={nextDay}>
        <ChevronRight size="1.077rem" />
      </button>
      <h1 className="day-title">{formatDayLabel(day)}</h1>
      {totals && (
        <span className="day-totals mono o2" aria-label="Totals">
          recorded {formatHm(totals.recordedMs)} · tracked {formatHm(totals.trackedMs)} · away {formatHm(totals.awayMs)}
        </span>
      )}
    </header>
  );
}
