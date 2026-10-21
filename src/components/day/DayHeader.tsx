import { ChevronLeft, ChevronRight, Ellipsis, SlidersHorizontal } from "lucide-react";
import type { DayTotals } from "../../api";
import { runCommand, useBindingLabel } from "../../commands";
import { formatDayLabel, isToday } from "../../lib/day";
import { formatHm } from "../../lib/format";
import { useStore } from "../../state/store";
import { useContextMenu } from "../ContextMenu";
import { SegmentedButtons } from "../settings/controls";

const DAY_MODES = ["timeline", "list"] as const;

const withKeys = (label: string, keys: string) => (keys ? `${label} (${keys})` : label);

/** Day navigation, the date, the day's totals, the Timeline or List switch and the ⋯ menu (delete activity). */
export function DayHeader({ day, totals }: { day: string; totals: DayTotals | null }) {
  const { step, goToday } = useStore.getState();
  const prevKeys = useBindingLabel("nav.prev");
  const nextKeys = useBindingLabel("nav.next");
  const todayKeys = useBindingLabel("nav.today");
  const settingsKeys = useBindingLabel("day.settings");
  const mode = useStore((s) => s.uiState.dayMode);
  const setDayMode = useStore((s) => s.setDayMode);
  const menu = useContextMenu();
  const openMenu = (button: HTMLElement) => {
    const { left, bottom } = button.getBoundingClientRect();
    menu.open({ x: left, y: bottom + 4 }, [
      { label: "Delete activity for this day…", action: () => runCommand("activity.deleteDay") },
      { label: "Delete activity in a range…", action: () => runCommand("activity.deleteRange") },
    ]);
  };
  return (
    <header className="day-head">
      <button type="button" className="icon-btn" aria-label="Previous day" title={withKeys("Previous day", prevKeys)} onClick={() => step(-1)}>
        <ChevronLeft size="1.077rem" />
      </button>
      <button type="button" className="btn" disabled={isToday(day)} title={withKeys("Today", todayKeys)} onClick={goToday}>
        Today
      </button>
      <button type="button" className="icon-btn" aria-label="Next day" title={withKeys("Next day", nextKeys)} onClick={() => step(1)}>
        <ChevronRight size="1.077rem" />
      </button>
      <h1 className="day-title">{formatDayLabel(day)}</h1>
      {totals && (
        <span className="day-totals mono o2" aria-label="Totals">
          recorded {formatHm(totals.recordedMs)} · tracked {formatHm(totals.trackedMs)} · away {formatHm(totals.awayMs)}
        </span>
      )}
      <SegmentedButtons label="Day mode" options={DAY_MODES} value={mode} onChange={setDayMode} />
      <button
        type="button"
        className="icon-btn"
        aria-label="Timeline settings"
        title={withKeys("Timeline settings", settingsKeys)}
        data-timeline-settings
        onClick={() => useStore.getState().openDialog({ type: "timelineSettings" })}
      >
        <SlidersHorizontal size="1.077rem" />
      </button>
      <button type="button" className="icon-btn" aria-label="More day actions" aria-haspopup="menu" title="More" onClick={(e) => openMenu(e.currentTarget)}>
        <Ellipsis size="1.077rem" />
      </button>
      {menu.element}
    </header>
  );
}
