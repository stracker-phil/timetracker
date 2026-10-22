import type { Lane } from "../../api";
import { AppIcon } from "../AppIcon";

/** The sticky labels over the recordings column, the time axis and each lane. */
export function LaneHeader({ lanes }: { lanes: Lane[] }) {
  return (
    <div className="timeline-head" role="row">
      <span className="recordings-col-head o3">Recordings</span>
      <span aria-hidden="true" />
      <div className="lane-heads">
        {lanes.map((lane) => (
          <span key={lane.id} className="lane-head" role="columnheader" title={lane.label}>
            {lane.kind === "app" && <AppIcon app={lane.label} bundleId={lane.appKey} size={16} />}
            <span className="lane-head-label">{lane.label}</span>
          </span>
        ))}
      </div>
    </div>
  );
}
