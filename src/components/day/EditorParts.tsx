import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { CoveredActivity as Activity } from "../../api";
import { formatClock, formatShortDuration } from "../../lib/format";
import { spansText } from "../../lib/recordings";
import type { EditorOverlap } from "../../state/slices/editor";
import { useStore } from "../../state/store";

const MINUTE_MS = 60_000;

/** The recording's length as `0 h 20 m`, both numbers editable; a change moves the last span's end. */
export function DurationField({ total, readOnly, onChange }: { total: number; readOnly: boolean; onChange: (ms: number) => void }) {
  const minutes = Math.round(total / MINUTE_MS);
  const [hours, rest] = [Math.floor(minutes / 60), minutes % 60];
  const set = (h: number, m: number) => {
    if (Number.isInteger(h) && Number.isInteger(m) && h >= 0 && m >= 0 && h * 60 + m > 0) onChange((h * 60 + m) * MINUTE_MS);
  };
  return (
    <div className="editor-duration" role="group" aria-label="Duration">
      <input className="duration-input" inputMode="numeric" aria-label="Hours" value={hours} readOnly={readOnly} onChange={(e) => set(Number(e.target.value), rest)} />
      <span className="duration-unit">h</span>
      <input className="duration-input" inputMode="numeric" aria-label="Minutes" value={rest} readOnly={readOnly} onChange={(e) => set(hours, Number(e.target.value))} />
      <span className="duration-unit">m</span>
    </div>
  );
}

/** The activity the spans cover: a count that opens the list. */
export function CoveredActivity({ activity }: { activity: Activity[] }) {
  const [open, setOpen] = useState(false);
  const Chevron = open ? ChevronDown : ChevronRight;
  return (
    <div className="editor-activity">
      <button type="button" className="editor-activity-head" aria-expanded={open} disabled={activity.length === 0} onClick={() => setOpen(!open)}>
        <Chevron size="0.923rem" aria-hidden="true" />
        <span>Activity</span>
        <span className="editor-activity-count">{activity.length}</span>
      </button>
      {open && (
        <ul className="editor-activity-list">
          {activity.map((a) => (
            <li key={a.id}>
              <span className="editor-activity-label">{a.private ? "Private" : a.label}</span>
              <span className="o3">
                {formatClock(a.start)} · {formatShortDuration(a.insideMs)}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** The recordings a save would overlap, asked in the editor: replace their overlapped part, or go back. */
export function OverlapNotice({ overlap }: { overlap: EditorOverlap }) {
  const projects = useStore((s) => s.projects);
  const nameOf = (id: string) => projects.find((p) => p.project.id === id)?.project.name ?? "Unknown project";
  const { answerOverlap } = useStore.getState();
  const count = overlap.recordings.length;
  return (
    <div className="editor-overlap" role="alert">
      <p className="editor-overlap-title">{count === 1 ? "This overlaps a recording" : `This overlaps ${count} recordings`}</p>
      <ul className="overlap-list">
        {overlap.recordings.map((r) => (
          <li key={r.id}>
            {spansText(r.spans)} {nameOf(r.projectId)}
            {r.note && <span className="o2"> · {r.note}</span>}
          </li>
        ))}
      </ul>
      <div className="actions">
        <button type="button" className="btn" onClick={() => answerOverlap(false)}>
          Back
        </button>
        <button type="button" className="btn btn-primary" autoFocus onClick={() => answerOverlap(true)}>
          Replace overlapped part
        </button>
      </div>
    </div>
  );
}
