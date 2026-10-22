import type { Recording } from "../../api";
import { spansText } from "../../lib/recordings";
import { useStore } from "../../state/store";
import { Modal } from "../Modal";

interface Props {
  recordings: Recording[];
  onReplace: () => void;
  onCancel: () => void;
}

/** Asks whether a new or changed recording may replace the overlapped part of the ones listed. */
export function OverlapDialog({ recordings, onReplace, onCancel }: Props) {
  const projects = useStore((s) => s.projects);
  const close = (replace: boolean) => {
    useStore.getState().closeDialog();
    if (replace) onReplace();
    else onCancel();
  };
  const nameOf = (id: string) => projects.find((p) => p.project.id === id)?.project.name ?? "Unknown project";
  return (
    <Modal onClose={() => close(false)} labelledBy="overlap-title">
      <h2 id="overlap-title">{recordings.length === 1 ? "This overlaps a recording" : `This overlaps ${recordings.length} recordings`}</h2>
      <ul className="overlap-list">
        {recordings.map((r) => (
          <li key={r.id}>
            <span className="tabular">{spansText(r.spans)}</span> {nameOf(r.projectId)}
            {r.note && <span className="o2"> · {r.note}</span>}
          </li>
        ))}
      </ul>
      <div className="actions">
        <button type="button" className="btn" onClick={() => close(false)}>
          Cancel
        </button>
        <button type="button" className="btn btn-primary" autoFocus onClick={() => close(true)}>
          Replace overlapped part
        </button>
      </div>
    </Modal>
  );
}
