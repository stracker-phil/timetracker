import { Tag } from "lucide-react";

/** A tag on a recording: its emoji (else a tag icon) and its name. */
export function TagLabel({ name, emoji }: { name: string; emoji: string | null }) {
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
