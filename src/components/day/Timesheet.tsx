import type { ReactNode } from "react";
import { Plus } from "lucide-react";
import type { DayView } from "../../api";
import type { Draft, Editor } from "../../state/slices/editor";
import { runCommand, useBindingLabel } from "../../commands";
import { formatDuration } from "../../lib/format";
import { sheetEntries, type SheetEntry } from "../../lib/sheet";
import { useStore } from "../../state/store";
import { RecordingEditor } from "./RecordingEditor";
import { SuggestionCard } from "./SuggestionCard";
import { TimesheetCard } from "./TimesheetCard";

/** The entry the open editor takes the place of (`replaces`) or goes before; `entryId` null puts it last. */
interface Slot {
  entryId: string | null;
  replaces: boolean;
}

/**
 * Where the open editor goes: in place of the recording or suggestion it edits, else before the first entry starting
 * after its first span.
 */
function editorSlot(entries: SheetEntry[], editor: Editor | null, draft: Draft | null): Slot | null {
  if (!editor || !draft) return null;
  const edited = editor.mode === "edit" ? editor.id : editor.suggestion?.id;
  if (edited && entries.some((e) => e.id === edited)) return { entryId: edited, replaces: true };
  if (editor.mode === "edit") return { entryId: entries[0]?.id ?? null, replaces: false };
  const start = draft.spans[0]?.start ?? editor.start;
  return { entryId: entries.find((e) => e.start > start)?.id ?? null, replaces: false };
}

/**
 * The left pane of the Day view: the day's recorded total, + for a new recording, and one
 * card per recording and suggestion in time order, with the editor open in the place of what it edits or creates.
 */
export function Timesheet({ view }: { view: DayView }) {
  const focused = useStore((s) => s.focusedRecording);
  const focusedSuggestion = useStore((s) => s.focusedSuggestion);
  const hovered = useStore((s) => s.hoveredRecording);
  const editor = useStore((s) => s.editor);
  const draft = useStore((s) => s.draft);
  const newKeys = useBindingLabel("recording.new");
  const entries = sheetEntries(view);
  const slot = editorSlot(entries, editor, draft);
  // One keyed list: the editor moves among the cards without being mounted again.
  const editorEl = slot && <RecordingEditor key={editor?.mode === "edit" ? `edit-${editor.id}` : "new"} />;
  const items: ReactNode[] = entries.flatMap((entry) => {
    const el =
      entry.kind === "card" ? (
        <TimesheetCard key={entry.id} card={entry.card} focused={entry.id === focused} hovered={entry.id === hovered} />
      ) : (
        <SuggestionCard key={entry.id} suggestion={entry.suggestion} focused={entry.id === focusedSuggestion} />
      );
    if (slot?.entryId !== entry.id) return [el];
    return slot.replaces ? [editorEl] : [editorEl, el];
  });
  if (slot && slot.entryId === null) items.push(editorEl);
  return (
    <aside className="timesheet" aria-label="Timesheet">
      <header className="timesheet-head">
        <div className="timesheet-heading">
          <span className="timesheet-label">Timesheet</span>
          <span className="timesheet-total" aria-label="Recorded">
            {formatDuration(view.totals.recordedMs)}
          </span>
        </div>
        {!editor && (
          <button type="button" className="btn timesheet-add" aria-label="New recording" title={newKeys ? `New recording (${newKeys})` : "New recording"} onClick={() => runCommand("recording.new")}>
            <Plus size="1.077rem" aria-hidden="true" />
          </button>
        )}
      </header>
      <div className="timesheet-cards">
        {items.length === 0 ? <p className="timesheet-empty o3">Nothing recorded yet. Drag over the timeline or click an activity.</p> : items}
      </div>
    </aside>
  );
}
