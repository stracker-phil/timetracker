import { Plus, X } from "lucide-react";
import type { Span } from "../../api";
import { formatClock } from "../../lib/format";
import { useStore } from "../../state/store";
import { TimeField } from "./TimeField";

interface Props {
  spans: Span[];
  /** ↑ and ↓ in a time field move by this (ms). */
  step: number;
  /** The start of the running timer's open span: then the spans are only shown. */
  runningSince: number | null;
  error?: string;
}

/** The recording's spans as From / To rows, each removable, and a row that adds one after the last (Rust places it). */
export function EditorSpans({ spans, step, runningSince, error }: Props) {
  const { setDraftSpan, removeDraftSpan, addDraftSpan } = useStore.getState();
  const editable = runningSince === null;
  return (
    <div className="editor-spans" role="group" aria-label="Spans">
      {spans.map((span, i) => (
        <div key={i} className="editor-span">
          <span className="editor-span-label">From</span>
          {editable ? (
            <TimeField label={`Span ${i + 1} from`} value={span.start} step={step} onChange={(start) => setDraftSpan(i, { ...span, start })} />
          ) : (
            <span className="editor-span-time">{formatClock(span.start)}</span>
          )}
          <span className="editor-span-label">To</span>
          {editable ? (
            <TimeField label={`Span ${i + 1} to`} value={span.end} step={step} onChange={(end) => setDraftSpan(i, { ...span, end })} />
          ) : (
            <span className="editor-span-time">{formatClock(span.end)}</span>
          )}
          {editable && spans.length > 1 && (
            <button type="button" className="btn btn-quiet editor-span-remove" aria-label={`Remove span ${formatClock(span.start)}–${formatClock(span.end)}`} onClick={() => removeDraftSpan(i)}>
              <X size="0.923rem" aria-hidden="true" />
            </button>
          )}
        </div>
      ))}
      {runningSince !== null && (
        <div className="editor-span">
          <span className="editor-span-label">From</span>
          <span className="editor-span-time">{formatClock(runningSince)}</span>
          <span className="editor-span-label">running</span>
        </div>
      )}
      {error && (
        <span className="form-field-error" role="alert">
          {error}
        </span>
      )}
      {editable && (
        <button type="button" className="btn btn-quiet editor-span-add" title="Or drag on the timeline" onClick={addDraftSpan}>
          <Plus size="0.923rem" aria-hidden="true" />
          Add span
        </button>
      )}
    </div>
  );
}
