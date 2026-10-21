import { ChevronLeft, ChevronRight } from "lucide-react";
import { api, type Period, type PeriodUnit, type Range } from "../api";
import { useBindingLabel } from "../commands";
import { formatDayLabel, formatMonthLabel, formatRangeLabel, today } from "../lib/day";
import { useApiResult } from "../lib/useApiResult";
import { useStore, type View } from "../state/store";

interface Shown {
  label: string;
  /** What ‹ and › step by, for their names. */
  unit: string;
  /** The Today button's label. */
  todayLabel: string;
  /** The period holds today, so the Today button has nothing to do. */
  current: boolean;
}

const CALENDAR_UNITS: Partial<Record<View, PeriodUnit>> = { day: "day", week: "week", month: "month" };
const TODAY_LABELS: Record<PeriodUnit, string> = { day: "Today", week: "This week", month: "This month" };
const REPORT_UNIT = "period";

type Preset = Exclude<Range["kind"], "custom">;

const PRESETS: Record<Preset, Omit<Shown, "unit">> = {
  today: { label: "Today", todayLabel: "Today", current: true },
  yesterday: { label: "Yesterday", todayLabel: "Today", current: false },
  thisWeek: { label: "This week", todayLabel: "This week", current: true },
  lastWeek: { label: "Last week", todayLabel: "This week", current: false },
  thisMonth: { label: "This month", todayLabel: "This month", current: true },
  lastMonth: { label: "Last month", todayLabel: "This month", current: false },
};

const TODAY_RANGE: Range = { kind: "today" };

const LABELS: Record<PeriodUnit, (period: Period) => string> = {
  day: ({ from }) => formatDayLabel(from),
  week: ({ from, to }) => formatRangeLabel(from, to),
  month: ({ from }) => formatMonthLabel(from),
};

function reportPeriod(range: Range): Shown {
  if (range.kind === "custom") return { label: formatRangeLabel(range.from, range.to), unit: REPORT_UNIT, todayLabel: "Today", current: false };
  return { ...PRESETS[range.kind], unit: REPORT_UNIT };
}

/** The report's range, or the default one until the filter changes. */
export const useReportRange = () => useStore((s) => s.reportFilter?.range) ?? TODAY_RANGE;

/** The day, week or month the Day, Week or Month view shows (from Rust), with its unit; null elsewhere. */
export function useCalendarPeriod(): { unit: PeriodUnit; period: Period } | null {
  const view = useStore((s) => s.view);
  const day = useStore((s) => s.day);
  const unit = CALENDAR_UNITS[view];
  const { data } = useApiResult(unit ? `${unit}:${day}:${today()}` : null, () => api.calendarPeriod(unit as PeriodUnit, day), { keepData: true });
  return unit && data ? { unit, period: data } : null;
}

function useShown(): Shown | null {
  const view = useStore((s) => s.view);
  const range = useReportRange();
  const calendar = useCalendarPeriod();
  if (view === "reports") return reportPeriod(range);
  if (!calendar) return null;
  const { unit, period } = calendar;
  return { label: LABELS[unit](period), unit, todayLabel: TODAY_LABELS[unit], current: period.holdsToday };
}

const withKeys = (label: string, keys: string) => (keys ? `${label} (${keys})` : label);

/**
 * The top of the sidebar: the shown day, week, month or report period, ‹ › and Today. The heading opens the
 * date picker. Views without a period keep the space empty, so the view list never moves.
 */
export function DateNavigator() {
  const shown = useShown();
  const step = useStore((s) => s.step);
  const goToday = useStore((s) => s.goToday);
  const openDialog = useStore((s) => s.openDialog);
  const prevKeys = useBindingLabel("nav.prev");
  const nextKeys = useBindingLabel("nav.next");
  const todayKeys = useBindingLabel("nav.today");
  if (!shown) {
    return (
      <div className="date-nav date-nav--idle" aria-hidden="true">
        <span className="date-nav-title">&nbsp;</span>
        <div className="date-nav-buttons" />
      </div>
    );
  }
  const prevName = `Previous ${shown.unit}`;
  const nextName = `Next ${shown.unit}`;
  return (
    <div className="date-nav" role="group" aria-label="Date">
      <button type="button" className="date-nav-title" aria-haspopup="dialog" title="Pick a date" data-date-heading onClick={() => openDialog({ type: "datePicker" })}>
        {shown.label}
      </button>
      <div className="date-nav-buttons">
        <button type="button" className="icon-btn" aria-label={prevName} title={withKeys(prevName, prevKeys)} onClick={() => step(-1)}>
          <ChevronLeft size="1.077rem" />
        </button>
        <button type="button" className="icon-btn" aria-label={nextName} title={withKeys(nextName, nextKeys)} onClick={() => step(1)}>
          <ChevronRight size="1.077rem" />
        </button>
        <button type="button" className="btn" disabled={shown.current} title={withKeys(shown.todayLabel, todayKeys)} onClick={goToday}>
          {shown.todayLabel}
        </button>
      </div>
    </div>
  );
}
