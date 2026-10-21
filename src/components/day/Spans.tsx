import type { AwayBlock, Gap } from "../../api";
import { formatDuration } from "../../lib/format";

interface Props {
  away: AwayBlock[];
  gaps: Gap[];
  y: (ms: number) => number;
}

/** An away span shorter than this (px) has no room for its label. */
const LABEL_MIN_PX = 16;

/** Away spans (labeled) and gaps (bare), outlined across the whole lanes area behind the blocks. */
export function Spans({ away, gaps, y }: Props) {
  return (
    <>
      {away.map((a) => {
        const height = y(a.end) - y(a.start);
        return (
          <div key={`away-${a.id}-${a.start}`} className="span away" style={{ top: y(a.start), height }}>
            {height >= LABEL_MIN_PX && (
              <span className="span-label">
                Away · {a.reason} {formatDuration(a.end - a.start)}
              </span>
            )}
          </div>
        );
      })}
      {gaps.map((g) => (
        <div key={`gap-${g.start}`} className="span gap" data-gap-start={g.start} style={{ top: y(g.start), height: y(g.end) - y(g.start) }} />
      ))}
    </>
  );
}
