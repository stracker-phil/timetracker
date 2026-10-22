import { api, type RuleInput } from "../../api";
import { formatClock, formatDuration } from "../../lib/format";
import { formatDayLabel } from "../../lib/day";
import { useApiResult } from "../../lib/useApiResult";

export const PREVIEW_DAYS = 7;

/** A rule can match only once each condition has a value (`is` may compare with nothing). */
const complete = (input: RuleInput) => input.conditions.length > 0 && input.conditions.every((c) => c.op === "is" || c.value.trim() !== "");

/** What the conditions match over the last week, refreshed while typing. */
export function RulePreviewPanel({ input }: { input: RuleInput }) {
  const key = complete(input) ? JSON.stringify(input.conditions) : null;
  const preview = useApiResult(key, () => api.rulePreview(input, PREVIEW_DAYS), { delayMs: 300, keepData: true });
  const data = key === null ? null : preview.data;

  return (
    <section className="rule-preview" aria-label="Matching activity">
      <h3>Last {PREVIEW_DAYS} days</h3>
      {key === null && <p className="o2">Give each condition a value to see what it matches.</p>}
      {preview.state === "failed" && key !== null && <p className="settings-alert">{preview.error.message}</p>}
      {data && (
        <>
          <p className="o2 mono">{data.hits.length === 0 ? "Nothing matches" : `${formatDuration(data.totalMs)} on ${data.days === 1 ? "1 day" : `${data.days} days`}`}</p>
          <ul className="rule-hits">
            {data.hits.map(({ block, day }) => (
              <li key={`${day}:${block.id}`}>
                <span className="mono o2">
                  {formatDayLabel(day)} {formatClock(block.start)}
                </span>
                <span className="rule-hit-app">{block.app}</span>
                <span className="grow-text">{block.title}</span>
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}
