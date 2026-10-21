import { formatClock, HOUR_MS } from "../../lib/format";

/** From this zoom on (px per minute), half hours get a mark and a hairline too. */
export const HALF_HOURS_FROM_PX = 3;

interface Props {
  bounds: [number, number];
  pxPerMinute: number;
}

/** Hour labels down the axis; the lanes draw the matching hairlines. */
export function TimeAxis({ bounds, pxPerMinute }: Props) {
  const y = (ms: number) => ((ms - bounds[0]) / 60_000) * pxPerMinute;
  const step = pxPerMinute >= HALF_HOURS_FROM_PX ? HOUR_MS / 2 : HOUR_MS;
  const marks: number[] = [];
  for (let t = bounds[0]; t < bounds[1]; t += step) marks.push(t);
  return (
    <div className="time-axis" aria-hidden="true">
      {marks.map((t) => {
        const hour = (t - bounds[0]) % HOUR_MS === 0;
        return (
          <div key={t} className={hour ? "axis-mark hour" : "axis-mark"} style={{ top: y(t) }}>
            {hour && <span className="axis-label mono">{formatClock(t)}</span>}
          </div>
        );
      })}
    </div>
  );
}
