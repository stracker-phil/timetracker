import { useState, type KeyboardEvent } from "react";
import { toAppError, type TagListItem } from "../../api";
import { useStore } from "../../state/store";

/** The tag's emoji, edited in place; empty removes it. Failures other than a field error are toasted by the store. */
export function TagEmojiField({ tag }: { tag: TagListItem }) {
  const saved = tag.emoji ?? "";
  const [draft, setDraft] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const value = draft ?? saved;

  function commit() {
    if (draft === null) return;
    const emoji = draft.trim();
    if (emoji === saved) {
      setDraft(null);
      return;
    }
    useStore
      .getState()
      .setTagEmoji(tag.id, emoji || null)
      .then(
        () => {
          setDraft(null);
          setError(null);
        },
        (e: unknown) => {
          const { field, message } = toAppError(e);
          setError(field ? message : null);
        },
      );
  }

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    e.stopPropagation();
    if (e.key === "Enter") {
      e.preventDefault();
      commit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      setDraft(null);
      setError(null);
    }
  };

  return (
    <span className="tag-emoji-field">
      <input
        className="input"
        aria-label={`Emoji for ${tag.name}`}
        aria-invalid={error !== null}
        title={error ?? "Emoji (⌃⌘Space)"}
        placeholder="🏷"
        value={value}
        onChange={(e) => {
          setDraft(e.target.value);
          setError(null);
        }}
        onKeyDown={onKeyDown}
        onBlur={commit}
      />
      {error && (
        <span className="form-field-error" role="alert">
          {error}
        </span>
      )}
    </span>
  );
}
