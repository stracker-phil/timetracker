import { Plus, X } from "lucide-react";
import type { Span } from "../../api";
import { formatClock } from "../../lib/format";
import { useStore } from "../../state/store";
import { EditorRow } from "./EditorParts";
import { TimeField } from "./TimeField";

interface Props {
  spans: Span[];
  /** ↑ and ↓ in a time field move by this (ms). */
  step: number;
  /** The start of the running timer's open span: then the spans are only shown. */
  runningSince: number | null;
  error?: string;
}

/**
 * The recording's spans (its times) as `From 10:00 to 10:20` rows, each removable while there are several, and a row that
 * adds one after the last (Rust places it).
 */
export function EditorSpans({ spans, step, runningSince, error }: Props) {
  const { setDraftSpan, removeDraftSpan, addDraftSpan } = useStore.getState();
  const editable = runningSince === null;
  return (
    <div className="editor-spans" role="group" aria-label="Times">
      {spans.map((span, i) => (
        <EditorRow key={i} label="From">
          <div className="editor-span">
            {editable ? (
              <TimeField label={`Time ${i + 1} from`} value={span.start} step={step} onChange={(start) => setDraftSpan(i, { ...span, start })} />
            ) : (
              <span className="editor-span-time">{formatClock(span.start)}</span>
            )}
            <span className="editor-span-to">to</span>
            {editable ? (
              <TimeField label={`Time ${i + 1} to`} value={span.end} step={step} onChange={(end) => setDraftSpan(i, { ...span, end })} />
            ) : (
              <span className="editor-span-time">{formatClock(span.end)}</span>
            )}
            {editable && spans.length > 1 && (
              <button type="button" className="icon-btn editor-span-remove" aria-label={`Remove time ${formatClock(span.start)}–${formatClock(span.end)}`} onClick={() => removeDraftSpan(i)}>
                <X size="0.923rem" aria-hidden="true" />
              </button>
            )}
          </div>
        </EditorRow>
      ))}
      {runningSince !== null && (
        <EditorRow label="From">
          <div className="editor-span">
            <span className="editor-span-time">{formatClock(runningSince)}</span>
            <span className="editor-span-to">running</span>
          </div>
        </EditorRow>
      )}
      {(editable || error) && (
        <EditorRow label="" error={error}>
          {editable && (
            <button type="button" className="btn btn-quiet editor-span-add" title="Or drag on the timeline" onClick={addDraftSpan}>
              <Plus size="0.923rem" aria-hidden="true" />
              Add time
            </button>
          )}
        </EditorRow>
      )}
    </div>
  );
}
