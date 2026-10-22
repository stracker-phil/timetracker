import type { ReactNode } from "react";
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
      <input className="input duration-input" inputMode="numeric" aria-label="Hours" value={hours} readOnly={readOnly} onChange={(e) => set(Number(e.target.value), rest)} />
      <span className="duration-unit">h</span>
      <input className="input duration-input" inputMode="numeric" aria-label="Minutes" value={rest} readOnly={readOnly} onChange={(e) => set(hours, Number(e.target.value))} />
      <span className="duration-unit">m</span>
    </div>
  );
}

/** A full-width control of the editor with its error below. */
export function EditorField({ error, className = "", children }: { error?: string; className?: string; children: ReactNode }) {
  return (
    <div className={`editor-field ${className}`.trim()}>
      {children}
      {error && (
        <span className="form-field-error" role="alert">
          {error}
        </span>
      )}
    </div>
  );
}

/** An `EditorField` behind an icon in place of a label (none: just the indent), lined up with a picker's icon. */
export function EditorLine({ icon, error, children }: { icon?: ReactNode; error?: string; children: ReactNode }) {
  return (
    <div className="editor-line">
      <span className="editor-line-icon" aria-hidden="true">
        {icon}
      </span>
      <EditorField error={error}>{children}</EditorField>
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
