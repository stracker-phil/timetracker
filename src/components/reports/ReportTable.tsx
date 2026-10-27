import { useState } from "react";
import { ChevronDown, ChevronRight, ChevronsDownUp, ChevronsUpDown, Copy } from "lucide-react";
import type { ReportClient, ReportProject, ReportRow } from "../../api";
import { formatDuration, plural } from "../../lib/format";
import { spansText } from "../../lib/recordings";
import { AccentDot } from "../Accent";
import { copyNotes } from "./actions";

function RecordingRow({ row, showDay }: { row: ReportRow; showDay: boolean }) {
  return (
    <tr className="report-recording">
      <td className="report-when">
        {showDay && <span className="report-day">{row.dayLabel}</span>}
        <span className="report-times">{spansText(row.spans)}</span>
      </td>
      <td className="report-note">{row.note}</td>
      <td className="report-tag" colSpan={2}>
        {row.tag}
      </td>
      <td className="report-hours">{formatDuration(row.roundedMs)}</td>
    </tr>
  );
}

function ProjectRows({ project, open, onToggle, showDay }: { project: ReportProject; open: boolean; onToggle: () => void; showDay: boolean }) {
  const Chevron = open ? ChevronDown : ChevronRight;
  return (
    <>
      <tr className="report-project">
        <td colSpan={3}>
          <button type="button" className="report-project-toggle" aria-expanded={open} onClick={onToggle}>
            <Chevron size="0.923rem" aria-hidden="true" />
            <AccentDot accent={project.color} shade={project.shade} />
            <span className="report-project-name">{project.name}</span>{" "}
            {project.billable && <span className="secondary-label">billable</span>}{" "}
            <span className="o3">{plural(project.rows.length, "recording", "recordings")}</span>
          </button>
        </td>
        <td className="report-project-actions">
          <button
            type="button"
            className="btn btn-quiet"
            aria-label={`Copy notes of ${project.label}`}
            title={plural(project.notes.count, "note", "notes")}
            disabled={project.notes.count === 0}
            onClick={() => void copyNotes(project.notes, project.label)}
          >
            <Copy size="1rem" aria-hidden="true" /> Copy notes
          </button>
        </td>
        <td className="report-hours">{formatDuration(project.roundedMs)}</td>
      </tr>
      {open && project.rows.map((row) => <RecordingRow key={`${row.id}:${row.day}`} row={row} showDay={showDay} />)}
    </>
  );
}

/** Clients, projects and their unfolding recordings with hours; recordings show their day only when `multiDay`. */
export function ReportTable({ clients, multiDay = false }: { clients: ReportClient[]; multiDay?: boolean }) {
  const [open, setOpen] = useState<ReadonlySet<string>>(new Set());
  const ids = clients.flatMap((c) => c.projects.map((p) => p.id));
  const allOpen = ids.every((id) => open.has(id));
  const toggle = (id: string) => {
    const next = new Set(open);
    if (!next.delete(id)) next.add(id);
    setOpen(next);
  };
  return (
    <section className="report-table" aria-label="Recordings by project">
      <header className="report-section-head">
        <h2>Projects</h2>
        <button type="button" className="btn btn-quiet" onClick={() => setOpen(new Set(allOpen ? [] : ids))}>
          {allOpen ? <ChevronsDownUp size="1rem" aria-hidden="true" /> : <ChevronsUpDown size="1rem" aria-hidden="true" />}
          {allOpen ? "Collapse all" : "Expand all"}
        </button>
      </header>
      <table className="report-rows">
        {clients.map((client) => (
          <tbody key={client.key}>
            <tr className="report-client">
              <th scope="rowgroup" colSpan={4}>
                {client.color && <AccentDot accent={client.color} />}
                {client.label}
              </th>
              <td className="report-hours">{formatDuration(client.roundedMs)}</td>
            </tr>
            {client.projects.map((project) => (
              <ProjectRows key={project.id} project={project} open={open.has(project.id)} onToggle={() => toggle(project.id)} showDay={multiDay} />
            ))}
          </tbody>
        ))}
      </table>
    </section>
  );
}
