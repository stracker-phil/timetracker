import { Ellipsis, SlidersHorizontal } from "lucide-react";
import type { DayTotals } from "../../api";
import { runCommand, useBindingLabel } from "../../commands";
import { formatDayLabel, isToday } from "../../lib/day";
import { formatHm } from "../../lib/format";
import { useStore } from "../../state/store";
import { useContextMenu } from "../ContextMenu";
import { SegmentedControl } from "../SegmentedControl";

const DAY_MODES = ["timeline", "list"] as const;

const withKeys = (label: string, keys: string) => (keys ? `${label} (${keys})` : label);

/**
 * The date, the day's totals (only for a day other than today: the status bar shows today's), the Timeline or
 * List switch and the ⋯ menu (delete activity). The sidebar changes the day.
 */
export function DayHeader({ day, totals }: { day: string; totals: DayTotals | null }) {
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
      <h1 className="day-title">{formatDayLabel(day)}</h1>
      {totals && !isToday(day) && (
        <span className="day-totals mono o2" aria-label="Totals">
          recorded {formatHm(totals.recordedMs)} · tracked {formatHm(totals.trackedMs)} · away {formatHm(totals.awayMs)}
        </span>
      )}
      <SegmentedControl label="Day mode" options={DAY_MODES} value={mode} onChange={setDayMode} />
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
