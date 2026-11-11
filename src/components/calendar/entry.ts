import type { CalendarEntry } from "../../api";
import { useStore } from "../../state/store";

/** Opens the editor on the entry's recording. */
export function editEntry(entry: CalendarEntry) {
  useStore.getState().switchEditor({ mode: "edit", id: entry.id, at: { start: entry.start, end: entry.end }, spanId: entry.spanIds[0] });
}

/** `Website: Landing page`, or the project alone without a note. */
export const entryName = (entry: CalendarEntry) => (entry.note ? `${entry.projectName}: ${entry.note}` : entry.projectName);
