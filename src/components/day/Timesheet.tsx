import type { ReactNode } from "react";
import { Plus } from "lucide-react";
import type { DayView, TimesheetCard as CardData } from "../../api";
import type { Draft, Editor } from "../../state/slices/editor";
import { runCommand, useBindingLabel } from "../../commands";
import { formatShortDuration } from "../../lib/format";
import { useStore } from "../../state/store";
import { RecordingEditor } from "./RecordingEditor";
import { TimesheetCard } from "./TimesheetCard";

/** The card the open editor takes the place of (`replaces`) or goes before; `cardId` null puts it last. */
interface Slot {
  cardId: string | null;
  replaces: boolean;
}

/** Where the open editor goes: in place of the card it edits, else before the first card starting after its first span. */
function editorSlot(cards: CardData[], editor: Editor | null, draft: Draft | null): Slot | null {
  if (!editor || !draft) return null;
  if (editor.mode === "edit") {
    const edited = cards.some((c) => c.id === editor.id);
    return edited ? { cardId: editor.id, replaces: true } : { cardId: cards[0]?.id ?? null, replaces: false };
  }
  const start = draft.spans[0]?.start ?? editor.start;
  return { cardId: cards.find((c) => c.start > start)?.id ?? null, replaces: false };
}

/**
 * The left pane of the Day view: the day's recorded total, + for a new recording, and one card per recording in
 * order, with the editor open in the place of the recording it edits or creates.
 */
export function Timesheet({ view }: { view: DayView }) {
  const focused = useStore((s) => s.focusedRecording);
  const hovered = useStore((s) => s.hoveredRecording);
  const editor = useStore((s) => s.editor);
  const draft = useStore((s) => s.draft);
  const newKeys = useBindingLabel("recording.new");
  const slot = editorSlot(view.timesheet, editor, draft);
  // One keyed list: the editor moves among the cards without being mounted again.
  const editorEl = slot && <RecordingEditor key={editor?.mode === "edit" ? `edit-${editor.id}` : "new"} />;
  const items: ReactNode[] = view.timesheet.flatMap((card) => {
    const cardEl = <TimesheetCard key={card.id} card={card} focused={card.id === focused} hovered={card.id === hovered} />;
    if (slot?.cardId !== card.id) return [cardEl];
    return slot.replaces ? [editorEl] : [editorEl, cardEl];
  });
  if (slot && slot.cardId === null) items.push(editorEl);
  return (
    <aside className="timesheet" aria-label="Timesheet">
      <header className="timesheet-head">
        <div className="timesheet-heading">
          <span className="timesheet-label">Timesheet</span>
          <span className="timesheet-total" aria-label="Recorded">
            {formatShortDuration(view.totals.recordedMs)}
          </span>
        </div>
        <button type="button" className="btn timesheet-add" aria-label="New recording" title={newKeys ? `New recording (${newKeys})` : "New recording"} onClick={() => runCommand("recording.new")}>
          <Plus size="1.077rem" aria-hidden="true" />
        </button>
      </header>
      <div className="timesheet-cards">
        {items.length === 0 ? <p className="timesheet-empty o3">Nothing recorded yet. Drag over the timeline or click an activity.</p> : items}
      </div>
    </aside>
  );
}
