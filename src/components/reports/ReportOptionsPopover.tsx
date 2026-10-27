import type { Rounding } from "../../api";
import { defaultFilter } from "../../state/slices/reports";
import { useStore } from "../../state/store";
import { Modal } from "../Modal";
import { Group, Row, Switch } from "../settings/controls";
import { useAnchoredStyle } from "../useAnchoredStyle";

const ROUNDING_STEPS = [5, 6, 10, 15, 30] as const;
const ROUNDINGS: { value: Rounding; label: string }[] = [
  { value: "none", label: "No rounding" },
  ...ROUNDING_STEPS.map((m) => ({ value: `nearest${m}` satisfies Rounding, label: `Nearest ${m} min` })),
  ...ROUNDING_STEPS.map((m) => ({ value: `up${m}` satisfies Rounding, label: `Up to ${m} min` })),
];

/** The Reports toolbar's options, under its button: the rounding and whether archived projects count. */
export function ReportOptionsPopover() {
  const close = useStore((s) => s.closeDialog);
  const reportFilter = useStore((s) => s.reportFilter);
  const settings = useStore((s) => s.settings);
  const setReportFilter = useStore((s) => s.setReportFilter);
  const style = useAnchoredStyle("[data-report-options]", "right");
  const filter = reportFilter ?? defaultFilter(settings);
  return (
    <Modal onClose={close} variant="popover" className="view-options" label="Report options" style={style}>
      <Group>
        <Row label="Rounding" htmlFor="report-rounding">
          <select id="report-rounding" className="input" value={filter.rounding} onChange={(e) => setReportFilter({ rounding: e.target.value as Rounding })}>
            {ROUNDINGS.map((r) => (
              <option key={r.value} value={r.value}>
                {r.label}
              </option>
            ))}
          </select>
        </Row>
        <Switch label="Include archived" description="Recordings of archived projects." checked={filter.includeArchived} onChange={(includeArchived) => setReportFilter({ includeArchived })} />
      </Group>
    </Modal>
  );
}
