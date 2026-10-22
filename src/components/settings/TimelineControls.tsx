import { useId } from "react";
import type { Continuity, Noise, TimelineSettings, Titles } from "../../api";
import { useStore } from "../../state/store";
import { Row, Segmented, Switch } from "./controls";

/** Rust's `SUMMARY_MINUTES` and `SNAP_MINUTES`; `Settings::sanitized` snaps other values to these. */
const SUMMARY_MINUTES = [5, 10, 15, 20, 30, 45, 60] as const;
const SNAP_MINUTES: readonly string[] = ["1", "5", "10", "15"];

const CONTINUITY: readonly Continuity[] = ["off", "short", "regular", "extended"];
const NOISE: readonly Noise[] = ["off", "low", "medium", "high"];
const TITLES: readonly Titles[] = ["document", "app"];

const saveTimeline = (patch: Partial<TimelineSettings>) => void useStore.getState().saveTimeline(patch);

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
        <span className="mono range-value">{value} min</span>
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
      <Switch label="Compact activity" description="Shorter rows, one line of text per block." checked={timeline.compact} onChange={(compact) => saveTimeline({ compact })} />
      <Switch label="Group by app and website" description="One block per app, whatever its windows are called." checked={timeline.groupByApp} onChange={(groupByApp) => saveTimeline({ groupByApp })} />
      <Segmented
        label="Continuity"
        description="Joins blocks of the same app across gaps shorter than 2, 8 or 16 minutes."
        options={CONTINUITY}
        value={timeline.continuity}
        onChange={(continuity) => saveTimeline({ continuity })}
      />
      <Segmented label="Noise cancellation" description="Hides blocks shorter than 2, 4 or 6 minutes." options={NOISE} value={timeline.noise} onChange={(noise) => saveTimeline({ noise })} />
      <Segmented label="Titles" description="Label blocks with the document or page name, or with the app." options={TITLES} value={timeline.titles} onChange={(titles) => saveTimeline({ titles })} />
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
