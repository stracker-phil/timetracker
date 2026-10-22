import { CircleCheck, Ellipsis, SlidersHorizontal } from "lucide-react";
import type { DayTotals } from "../../api";
import { runCommand, useBindingLabel } from "../../commands";
import { formatDuration } from "../../lib/format";
import { useStore } from "../../state/store";
import { useContextMenu } from "../ContextMenu";
import { Toolbar } from "../Toolbar";

const withKeys = (label: string, keys: string) => (keys ? `${label} (${keys})` : label);

/** `recorded 1h 00m · tracked 6h 12m`, without the parts that are zero; empty when both are. */
function totalsText(totals: DayTotals): string {
  const parts: [string, number][] = [
    ["recorded", totals.recordedMs],
    ["tracked", totals.trackedMs],
  ];
  return parts
    .filter(([, ms]) => ms > 0)
    .map(([name, ms]) => `${name} ${formatDuration(ms)}`)
    .join(" · ");
}

/** A green check once the day has recorded time, else a grey dot. */
function RecordedMark({ recorded }: { recorded: boolean }) {
  return recorded ? (
    <CircleCheck className="day-mark day-mark--recorded" size="1.077rem" role="img" aria-label="Recorded" />
  ) : (
    <span className="day-mark day-mark--empty" role="img" aria-label="Nothing recorded" />
  );
}

/** `label` is the day's title from Rust, in the format of the day title setting. */
export function DayHeader({ label = "", totals }: { label?: string; totals: DayTotals | null }) {
  const optionsKeys = useBindingLabel("day.settings");
  const text = totals ? totalsText(totals) : "";
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
      title={label}
      detail={
        totals && (
          <>
            <RecordedMark recorded={totals.recordedMs > 0} />
            {text && (
              <span className="toolbar-detail" aria-label="Totals">
                {text}
              </span>
            )}
          </>
        )
      }
    >
      <button
        type="button"
        className="icon-btn"
        aria-label="View options"
        aria-haspopup="dialog"
        title={withKeys("View options", optionsKeys)}
        data-view-options
        onClick={() => useStore.getState().openDialog({ type: "viewOptions" })}
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
