import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { ReportGroup } from "../../api";
import { formatHm } from "../../lib/format";
import { spansText } from "../../lib/recordings";
import { AccentDot } from "../Accent";

/** One group: its label and hours, the notes below, and its recordings on request. */
export function ReportGroupTable({ group, rounded }: { group: ReportGroup; rounded: boolean }) {
  const [open, setOpen] = useState(false);
  const count = group.rows.length;
  return (
    <section className="report-group" aria-label={group.label}>
      <header className="report-group-head">
        {group.color && <AccentDot accent={group.color} shade={group.shade ?? undefined} />}
        <h3 className="grow-text">{group.label}</h3>
        {rounded && group.roundedMs !== group.durationMs && <span className="mono o3" title="Before rounding">{formatHm(group.durationMs)}</span>}
        <span className="mono report-hours">{formatHm(group.roundedMs)} h</span>
      </header>
      {group.notes.length > 0 && (
        <ul className="report-notes">
          {group.notes.map((note) => (
            <li key={note.text}>{note.text}</li>
          ))}
        </ul>
      )}
      <button type="button" className="report-rows-toggle o3" aria-expanded={open} onClick={() => setOpen(!open)}>
        {open ? <ChevronDown size="0.923rem" aria-hidden="true" /> : <ChevronRight size="0.923rem" aria-hidden="true" />}
        {count === 1 ? "1 recording" : `${count} recordings`}
      </button>
      {open && (
        <table className="report-rows">
          <thead>
            <tr>
              <th>Day</th>
              <th>Time</th>
              <th>Duration</th>
              <th>Project</th>
              <th>Tag</th>
              <th>Note</th>
            </tr>
          </thead>
          <tbody>
            {group.rows.map((row) => (
              <tr key={`${row.id}:${row.day}`}>
                <td className="mono">{row.day}</td>
                <td className="mono">{spansText(row.spans)}</td>
                <td className="mono">{formatHm(rounded ? row.roundedMs : row.durationMs)}</td>
                <td>{row.client ? `${row.client} / ${row.project}` : row.project}</td>
                <td className="mono o2">{row.tag}</td>
                <td>{row.note}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
