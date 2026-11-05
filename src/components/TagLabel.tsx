import { Tag } from "lucide-react";

/** The mark for a recording without a tag: every recording should have one (docs/ux.md → Tags). */
export const NO_TAG_MARK = "⚠️";

/** The "no tag" warning sign alone, hidden from assistive tech (its owner names it). */
export function NoTagMark() {
  return (
    <span className="tag-emoji" aria-hidden="true">
      {NO_TAG_MARK}
    </span>
  );
}

/** A recording's tag: its emoji (else a tag icon) and its name; without a tag, ⚠️ and "No tag". */
export function TagLabel({ name, emoji }: { name: string | null; emoji: string | null }) {
  if (name === null) {
    return (
      <>
        <NoTagMark />
        No tag
      </>
    );
  }
  return (
    <>
      {emoji ? (
        <span className="tag-emoji" aria-hidden="true">
          {emoji}
        </span>
      ) : (
        <Tag size="0.846rem" aria-hidden="true" />
      )}
      {name}
    </>
  );
}
