import type { KeyboardEvent, PointerEvent as ReactPointerEvent } from "react";
import type { ListRow as Row } from "../../../api";
import { formatClock, formatHm } from "../../../lib/format";

const spansText = (spans: Row["spans"]) => spans.map(([start, end]) => `${formatClock(start)}–${formatClock(end)}`).join(", ");

function ListRowDetails({ row }: { row: Row }) {
  return (
    <dl className="list-details">
      <dt>Title</dt>
      <dd className="mono">{row.title}</dd>
      {row.url && (
        <>
          <dt>URL</dt>
          <dd className="mono">{row.url}</dd>
        </>
      )}
      {row.path && (
        <>
          <dt>Path</dt>
          <dd className="mono">{row.path}</dd>
        </>
      )}
      <dt>Spans</dt>
      <dd className="mono">{spansText(row.spans)}</dd>
    </dl>
  );
}

interface Props {
  row: Row;
  focused: boolean;
  open: boolean;
  dragging: boolean;
  onPointerDown: (e: ReactPointerEvent<HTMLLIElement>) => void;
  onKeyDown: (e: KeyboardEvent<HTMLLIElement>) => void;
  onFocus: () => void;
  onToggleDetails: () => void;
}

export function ListRow({ row, focused, open, dragging, onPointerDown, onKeyDown, onFocus, onToggleDetails }: Props) {
  const where = row.url ?? row.path;
  const share = row.totalMs > 0 ? Math.min(1, row.coveredMs / row.totalMs) : 0;
  return (
    <li
      className={["list-row", dragging && "dragging", row.private && "private"].filter(Boolean).join(" ")}
      tabIndex={focused ? 0 : -1}
      data-row-key={row.key}
      onPointerDown={onPointerDown}
      onKeyDown={onKeyDown}
      onFocus={onFocus}
    >
      <div className="list-row-main">
        <span className="list-app o2">{row.app}</span>
        <button
          type="button"
          className="list-title mono"
          tabIndex={-1}
          aria-expanded={open}
          data-list-title
          // A mouse click is handled at the pointer's release (the row captures the pointer for dragging);
          // this is for clicks without a pointer, such as VoiceOver's.
          onClick={(e) => e.detail === 0 && onToggleDetails()}
        >
          {row.label}
        </button>
        {where && <span className="list-where mono o3">{where}</span>}
        <span className="list-total mono">
          <span>
            {formatHm(row.totalMs)}
            {row.spans.length > 1 && <span className="o3"> · {row.spans.length} spans</span>}
          </span>
          <span className="covered-bar" title={`${formatHm(row.coveredMs)} recorded`}>
            <span style={{ width: `${share * 100}%` }} />
          </span>
        </span>
      </div>
      {open && <ListRowDetails row={row} />}
    </li>
  );
}
