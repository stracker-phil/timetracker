import { useEffect, useRef, useState } from "react";
import { api, type ActivityRow, type AwayRow } from "../api";
import { formatClock, formatDay, formatDuration } from "../lib/format";
import { useApiResult } from "../lib/useApiResult";
import { selectDayVersion, useStore } from "../state/store";

const RELOAD_MS = 10_000;

/** The calendar day before or after `day` (`YYYY-MM-DD`). */
function shiftDay(day: string, by: number): string {
  const [y, m, d] = day.split("-").map(Number);
  return formatDay(new Date(y, m - 1, d + by).getTime());
}

/** A counter that follows tracker statuses, at most once per `RELOAD_MS`; a status inside that window counts when it ends. */
function useTrackerPulse(): number {
  const tracker = useStore((s) => s.tracker);
  const [pulse, setPulse] = useState(0);
  const last = useRef(0);
  useEffect(() => {
    const wait = last.current + RELOAD_MS - Date.now();
    const bump = () => {
      last.current = Date.now();
      setPulse((p) => p + 1);
    };
    if (wait <= 0) return bump();
    const timer = setTimeout(bump, wait);
    return () => clearTimeout(timer);
  }, [tracker]);
  return pulse;
}

interface Column<T> {
  title: string;
  value: (row: T) => string;
}

function Rows<T extends ActivityRow | AwayRow>({ label, rows, columns }: { label: string; rows: T[]; columns: Column<T>[] }) {
  return (
    <table className="debug-table" aria-label={label}>
      <thead>
        <tr>
          <th>Start</th>
          <th>End</th>
          <th>Duration</th>
          {columns.map((c) => (
            <th key={c.title}>{c.title}</th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => (
          <tr key={row.id} className={"private" in row && row.private ? "o3" : undefined}>
            <td className="mono">{formatClock(row.start)}</td>
            <td className="mono">{row.end === null ? "" : formatClock(row.end)}</td>
            <td className="mono">{formatDuration((row.end ?? row.start) - row.start)}</td>
            {columns.map((c) => (
              <td key={c.title}>{c.value(row)}</td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** A plain list of a day's activity and away spans. */
export function ActivityDebug() {
  const day = useStore((s) => s.day);
  const goToDay = useStore((s) => s.goToDay);
  const pulse = useTrackerPulse();
  const dayVersion = useStore(selectDayVersion(day));
  const key = `${day}\u0000${pulse}\u0000${dayVersion}`;
  const activity = useApiResult(key, () => api.activityList(day), { keepData: true });
  const away = useApiResult(key, () => api.awayList(day), { keepData: true });

  return (
    <section className="activity-debug" aria-label="Activity">
      <div className="activity-debug-head">
        <button type="button" className="icon-btn" aria-label="Previous day" onClick={() => goToDay(shiftDay(day, -1))}>
          ←
        </button>
        <input className="input mono" type="date" aria-label="Day" value={day} onChange={(e) => {
            // A cleared field keeps the day shown.
            if (e.target.value) goToDay(e.target.value);
          }} />
        <button type="button" className="icon-btn" aria-label="Next day" onClick={() => goToDay(shiftDay(day, 1))}>
          →
        </button>
      </div>
      {activity.error && <p className="settings-alert">{activity.error.message}</p>}
      <h2>Activity</h2>
      <Rows
        label="Activity"
        rows={activity.data ?? []}
        columns={[
          { title: "App", value: (r) => r.app },
          { title: "Title", value: (r) => r.title },
          { title: "URL or path", value: (r) => r.url ?? r.path ?? "" },
        ]}
      />
      <h2>Away</h2>
      <Rows label="Away" rows={away.data ?? []} columns={[{ title: "Reason", value: (r) => r.reason }]} />
    </section>
  );
}
