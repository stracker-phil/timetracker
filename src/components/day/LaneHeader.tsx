import type { Lane } from "../../api";

/** The sticky labels over the recordings column, the time axis and each lane. */
export function LaneHeader({ lanes }: { lanes: Lane[] }) {
  return (
    <div className="timeline-head" role="row">
      <span className="recordings-col-head o3">Recordings</span>
      <span aria-hidden="true" />
      <div className="lane-heads">
        {lanes.map((lane) => (
          <span key={lane.id} className="lane-head" role="columnheader" title={lane.label}>
            {lane.label}
          </span>
        ))}
      </div>
    </div>
  );
}
