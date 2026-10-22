import { Pencil, X } from "lucide-react";
import type { TagListItem } from "../../api";
import { plural } from "../../lib/format";
import { TagEmojiField } from "./TagEmojiField";
import { useTagRename } from "./useTagRename";

interface Props {
  tag: TagListItem;
  onDelete: (tag: TagListItem) => void;
}

/** One tag row: its emoji, inline rename, recording count and actions. */
export function TagRow({ tag, onDelete }: Props) {
  const rename = useTagRename(tag);

  return (
    <li ref={rename.rowRef} className="tag-row" tabIndex={0} onKeyDown={rename.rowKeyDown}>
      <TagEmojiField tag={tag} />
      {rename.draft === null ? (
        <span className="grow-text" onDoubleClick={rename.start}>
          {tag.name}
        </span>
      ) : (
        <span className="tag-rename">
          <input
            ref={rename.fieldRef}
            className="input"
            aria-label="Tag name"
            aria-invalid={rename.error !== null}
            autoFocus
            value={rename.draft}
            onChange={(e) => rename.change(e.target.value)}
            onKeyDown={rename.fieldKeyDown}
            onBlur={rename.blur}
          />
          {rename.error && (
            <span className="form-field-error" role="alert">
              {rename.error}
            </span>
          )}
        </span>
      )}
      <span className="o2 tabular">{plural(tag.recordings, "recording", "recordings")}</span>
      <button type="button" className="icon-btn" aria-label={`Rename tag ${tag.name}`} title="Rename" onClick={rename.start}>
        <Pencil size="1.077rem" />
      </button>
      <button type="button" className="icon-btn" aria-label={`Delete tag ${tag.name}`} title="Delete" onClick={() => onDelete(tag)}>
        <X size="1.077rem" />
      </button>
    </li>
  );
}
