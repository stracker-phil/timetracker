import type { RuleInput } from "../../api";
import { formatClock, formatDuration } from "../../lib/format";
import { useDebounced } from "../../lib/useDebounced";
import { useRulePreview } from "../../queries/rules";

export const PREVIEW_DAYS = 7;
/** Typing waits this long before asking. */
const DEBOUNCE_MS = 300;

/** A rule can match only once each condition has a value (`is` may compare with nothing). */
const complete = (input: RuleInput) => input.conditions.length > 0 && input.conditions.every((c) => c.op === "is" || c.value.trim() !== "");

/** What the conditions match over the last week, refreshed while typing. */
export function RulePreviewPanel({ input }: { input: RuleInput }) {
  const asked = useDebounced(complete(input) ? input : null, DEBOUNCE_MS);
  const preview = useRulePreview(asked, PREVIEW_DAYS);
  const shown = complete(input);
  const data = shown ? preview.data : null;

  return (
    <section className="rule-preview scroller" aria-label="Matching activity">
      <h3>Last {PREVIEW_DAYS} days</h3>
      {!shown && <p className="o2">Give each condition a value to see what it matches.</p>}
      {preview.error && shown && <p className="settings-alert">{preview.error.message}</p>}
      {data && (
        <>
          <p className="o2">{data.hits.length === 0 ? "Nothing matches" : `${formatDuration(data.totalMs)} on ${data.days === 1 ? "1 day" : `${data.days} days`}`}</p>
          <ul className="rule-hits">
            {data.hits.map(({ block, day, dayLabel }) => (
              <li key={`${day}:${block.id}`}>
                <span className="o2">
                  {dayLabel} {formatClock(block.start)}
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
