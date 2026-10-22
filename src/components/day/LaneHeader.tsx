import { History, Layers, Plug } from "lucide-react";
import type { Lane } from "../../api";
import { formatDuration } from "../../lib/format";
import { AppIcon } from "../AppIcon";

function LaneIcon({ lane }: { lane: Lane }) {
  switch (lane.kind) {
    case "summary":
      return <Layers size={16} aria-hidden="true" />;
    case "activity":
      return <History size={16} aria-hidden="true" />;
    case "app":
      return <AppIcon app={lane.label} bundleId={lane.bundleId} size={16} />;
    case "source":
      return <Plug size={16} aria-hidden="true" />;
  }
}

/** The sticky heads over the lanes; the activity lane shows the day's tracked time. */
export function LaneHeader({ lanes, trackedMs }: { lanes: Lane[]; trackedMs: number }) {
  return (
    <div className="timeline-head" role="row">
      <span aria-hidden="true" />
      <span aria-hidden="true" />
      <div className="lane-heads">
        {lanes.map((lane) => (
          <span key={lane.id} className={`lane-head lane-head--${lane.kind}`} role="columnheader" title={lane.label}>
            <LaneIcon lane={lane} />
            <span className="lane-head-label">{lane.label}</span>
            {lane.kind === "activity" && <span className="lane-head-total">{formatDuration(trackedMs)}</span>}
          </span>
        ))}
      </div>
    </div>
  );
}
