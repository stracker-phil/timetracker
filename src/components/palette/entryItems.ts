import type { FoundEntry } from "../../api";
import { formatClock } from "../../lib/format";
import { NO_PROJECT } from "../../state/slices/suggestions";
import { useStore } from "../../state/store";
import { score } from "./parse";
import type { Item } from "./usePaletteItems";

const fields = (e: FoundEntry) => [e.project ?? NO_PROJECT, e.note, e.client ?? "", e.tag ?? "", e.by ?? ""];

/** `Wed, Nov 11 · 09:00–10:30 · weekly call`; a running one ends `now`. */
function subtitle(e: FoundEntry): string {
  const times = `${formatClock(e.start)}–${e.end === null ? "now" : formatClock(e.end)}`;
  return [e.dayLabel, times, e.note].filter(Boolean).join(" · ");
}

function entryItem(e: FoundEntry, section: string, rank: number): Item {
  return {
    key: `${e.entry.kind}:${e.entry.id}`,
    section,
    title: e.project ?? NO_PROJECT,
    subtitle: subtitle(e),
    accent: e.color,
    shade: e.shade,
    rank,
    go: () => {
      const s = useStore.getState();
      s.setPaletteOpen(false);
      s.revealEntry(e.entry);
    },
  };
}

/** The `r:` rows, each opening its day's Day view with it selected: the last days' suggestions, then the matching recordings. */
export function entryItems(suggestions: FoundEntry[], recordings: FoundEntry[], needle: string): Item[][] {
  return [
    suggestions.map((e) => entryItem(e, "Suggestions", score(fields(e), needle))),
    recordings.map((e) => entryItem(e, "Recordings", score(fields(e), needle))),
  ];
}
