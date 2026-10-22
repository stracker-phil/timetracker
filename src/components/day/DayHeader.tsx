import { Columns3, Ellipsis, SlidersHorizontal } from "lucide-react";
import type { DayTotals } from "../../api";
import { runCommand, useBindingLabel } from "../../commands";
import { formatDayLabel, isToday } from "../../lib/day";
import { formatHm } from "../../lib/format";
import { useStore } from "../../state/store";
import { useContextMenu } from "../ContextMenu";
import { SegmentedControl } from "../SegmentedControl";
import { Toolbar } from "../Toolbar";

const DAY_MODES = ["timeline", "list"] as const;

const withKeys = (label: string, keys: string) => (keys ? `${label} (${keys})` : label);

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
    <Toolbar
      title={formatDayLabel(day)}
      detail={
        totals &&
        !isToday(day) && (
          <span className="toolbar-detail" aria-label="Totals">
            recorded {formatHm(totals.recordedMs)} · tracked {formatHm(totals.trackedMs)} · away {formatHm(totals.awayMs)}
          </span>
        )
      }
    >
      <SegmentedControl label="Day mode" options={DAY_MODES} value={mode} onChange={setDayMode} />
      {mode === "timeline" && (
        <button type="button" className="btn btn-quiet" data-lanes-button aria-haspopup="dialog" onClick={() => useStore.getState().openDialog({ type: "lanes" })}>
          <Columns3 size="1.154rem" aria-hidden="true" />
          Lanes
        </button>
      )}
      <button
        type="button"
        className="icon-btn"
        aria-label="Timeline settings"
        title={withKeys("Timeline settings", settingsKeys)}
        data-timeline-settings
        onClick={() => useStore.getState().openDialog({ type: "timelineSettings" })}
      >
        <SlidersHorizontal size="1.154rem" />
      </button>
      <button type="button" className="icon-btn" aria-label="More day actions" aria-haspopup="menu" title="More" onClick={(e) => openMenu(e.currentTarget)}>
        <Ellipsis size="1.154rem" />
      </button>
      {menu.element}
    </Toolbar>
  );
}
