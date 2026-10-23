import type { CSSProperties } from "react";
import { History, Layers, Plug } from "lucide-react";
import type { Lane } from "../../api";
import { formatDuration } from "../../lib/format";
import { AppIcon } from "../AppIcon";
import { laneCols } from "./Lane";

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

/** The sticky heads over the lanes; the activity lane shows the day's tracked time, a pinned app lane only its icon. */
export function LaneHeader({ lanes, trackedMs }: { lanes: Lane[]; trackedMs: number }) {
  return (
    <div className="timeline-head" role="row">
      <span className="head-axis" aria-hidden="true" />
      <span className="head-col" aria-hidden="true" />
      <div className="lane-heads">
        {lanes.map((lane) => (
          <span
            key={lane.id}
            className={`lane-head lane-head--${lane.kind}`}
            role="columnheader"
            aria-label={lane.label}
            title={lane.label}
            style={{ "--lane-cols": laneCols(lane) } as CSSProperties}
          >
            <LaneIcon lane={lane} />
            {lane.kind !== "app" && <span className="lane-head-label">{lane.label}</span>}
            {lane.kind === "activity" && <span className="lane-head-total">{formatDuration(trackedMs)}</span>}
          </span>
        ))}
      </div>
    </div>
  );
}
