import { useState } from "react";
import { ChevronDown, ChevronRight, ChevronsDownUp, ChevronsUpDown, Copy } from "lucide-react";
import type { ReportClient, ReportProject, ReportRow } from "../../api";
import { formatDuration, plural } from "../../lib/format";
import { spansText } from "../../lib/recordings";
import { AccentDot } from "../Accent";
import { copyNotes } from "./actions";

function RecordingRow({ row }: { row: ReportRow }) {
  return (
    <tr className="report-recording">
      <td>
        <span className="report-recording-when">
          {row.dayLabel} · {spansText(row.spans)}
        </span>
        {row.note && <span className="report-recording-note">{row.note}</span>}
        {row.tag && <span className="report-recording-tag o2">{row.tag}</span>}
      </td>
      <td />
      <td className="report-hours">{formatDuration(row.roundedMs)}</td>
    </tr>
  );
}

function ProjectRows({ project, open, onToggle }: { project: ReportProject; open: boolean; onToggle: () => void }) {
  const Chevron = open ? ChevronDown : ChevronRight;
  return (
    <>
      <tr className="report-project">
        <td>
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
      {open && project.rows.map((row) => <RecordingRow key={`${row.id}:${row.day}`} row={row} />)}
    </>
  );
}

/** Clients, their projects and the projects' recordings, each with its hours; a project unfolds its recordings. */
export function ReportTable({ clients }: { clients: ReportClient[] }) {
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
      <header className="report-table-head">
        <h2>Projects</h2>
        <button type="button" className="btn" onClick={() => setOpen(new Set(allOpen ? [] : ids))}>
          {allOpen ? <ChevronsDownUp size="1rem" aria-hidden="true" /> : <ChevronsUpDown size="1rem" aria-hidden="true" />}
          {allOpen ? "Collapse all" : "Expand all"}
        </button>
      </header>
      <table className="report-rows">
        <thead>
          <tr>
            <th scope="col">Project</th>
            <th scope="col" aria-label="Notes" />
            <th scope="col" className="report-hours">
              Hours
            </th>
          </tr>
        </thead>
        {clients.map((client) => (
          <tbody key={client.key}>
            <tr className="report-client">
              <th scope="rowgroup" colSpan={2}>
                {client.color && <AccentDot accent={client.color} />}
                {client.label}
              </th>
              <td className="report-hours">{formatDuration(client.roundedMs)}</td>
            </tr>
            {client.projects.map((project) => (
              <ProjectRows key={project.id} project={project} open={open.has(project.id)} onToggle={() => toggle(project.id)} />
            ))}
          </tbody>
        ))}
      </table>
    </section>
  );
}
