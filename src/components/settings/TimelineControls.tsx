import { useId } from "react";
import type { Continuity, Noise, TimelineSettings } from "../../api";
import { Row, Segmented, Switch } from "./controls";
import { TITLES, saveTimeline } from "./timelineActions";

/** Rust's `SUMMARY_MINUTES` and `SNAP_MINUTES`; `Settings::sanitized` snaps other values to these. */
const SUMMARY_MINUTES = [5, 10, 15, 20, 30, 45, 60] as const;
const SNAP_MINUTES: readonly string[] = ["1", "5", "10", "15"];

/** The gaps and lengths of Rust's `continuity_ms` and `noise_ms`. */
const CONTINUITY: readonly Continuity[] = ["off", "short", "regular", "extended"];
const CONTINUITY_LABELS: Record<Continuity, string> = { off: "Off", short: "2 min", regular: "8 min", extended: "16 min" };
const NOISE: readonly Noise[] = ["off", "low", "medium", "high"];
const NOISE_LABELS: Record<Noise, string> = { off: "Off", low: "2 min", medium: "4 min", high: "6 min" };

function SummaryMinutes({ value }: { value: number }) {
  const id = useId();
  const index = Math.max(0, SUMMARY_MINUTES.findIndex((m) => m === value));
  return (
    <Row label="Summary interval" description="Each interval shows the app with the most time in it." htmlFor={id}>
      <div className="settings-input">
        <input
          id={id}
          type="range"
          min={0}
          max={SUMMARY_MINUTES.length - 1}
          step={1}
          value={index}
          list={`${id}-marks`}
          onChange={(e) => saveTimeline({ summaryMinutes: SUMMARY_MINUTES[Number(e.target.value)] })}
        />
        <datalist id={`${id}-marks`}>
          {SUMMARY_MINUTES.map((m, i) => (
            <option key={m} value={i} />
          ))}
        </datalist>
        <span className="tabular range-value">{value} min</span>
      </div>
    </Row>
  );
}

/** Every timeline setting; each change is saved right away and the Day view reloads. */
export function TimelineControls({ timeline }: { timeline: TimelineSettings }) {
  return (
    <>
      <Switch label="Summary lane" description="One lane with the main app of each interval." checked={timeline.summaryLane} onChange={(summaryLane) => saveTimeline({ summaryLane })} />
      {timeline.summaryLane && <SummaryMinutes value={timeline.summaryMinutes} />}
      <Switch label="Show all activity" description="The activity lane next to the summary." checked={timeline.showAllActivity} onChange={(showAllActivity) => saveTimeline({ showAllActivity })} />
      <Switch label="Compact activity" description="Shorter rows, one line of text per activity." checked={timeline.compact} onChange={(compact) => saveTimeline({ compact })} />
      <Switch label="Group by app and website" description="One activity per app, whatever its windows are called." checked={timeline.groupByApp} onChange={(groupByApp) => saveTimeline({ groupByApp })} />
      <Segmented
        label="Join short gaps"
        description="Activity of the same app with a shorter gap between shows as one."
        options={CONTINUITY}
        labels={CONTINUITY_LABELS}
        value={timeline.continuity}
        onChange={(continuity) => saveTimeline({ continuity })}
      />
      <Segmented
        label="Hide activity shorter than"
        description="Short activity leaves the timeline but still counts toward the day's time."
        options={NOISE}
        labels={NOISE_LABELS}
        value={timeline.noise}
        onChange={(noise) => saveTimeline({ noise })}
      />
      <Segmented label="Titles" description="Label activity with the document or page name, or with the app." options={TITLES} value={timeline.titles} onChange={(titles) => saveTimeline({ titles })} />
      <Segmented
        label="Time snapping (minutes)"
        description="The grid recordings snap to."
        options={SNAP_MINUTES}
        value={String(timeline.snapMinutes)}
        onChange={(m) => saveTimeline({ snapMinutes: Number(m) })}
      />
    </>
  );
}
