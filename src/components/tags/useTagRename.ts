import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { toAppError, type TagListItem } from "../../api";
import { useStore } from "../../state/store";

/** In-place rename of `tag`: ↵ or blur saves through `saveTag`, Escape and an empty or unchanged name cancel. */
export function useTagRename(tag: TagListItem) {
  const rowRef = useRef<HTMLLIElement>(null);
  const fieldRef = useRef<HTMLInputElement>(null);
  const [draft, setDraft] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // `draft` cannot guard this: the field stays open while a save runs, and refocusing the row blurs it before
  // the render that closes it, so a blur after ↵ or Escape would save again.
  const settled = useRef(false);
  const editing = draft !== null;

  useEffect(() => {
    if (editing) fieldRef.current?.select();
  }, [editing]);

  function start() {
    if (editing) return;
    settled.current = false;
    setDraft(tag.name);
    setError(null);
  }

  function finish(refocus: boolean) {
    settled.current = true;
    setDraft(null);
    setError(null);
    if (refocus) rowRef.current?.focus();
  }

  function commit(refocus: boolean) {
    if (draft === null || settled.current) return;
    const name = draft.trim();
    if (!name || name === tag.name) {
      finish(refocus);
      return;
    }
    settled.current = true;
    useStore
      .getState()
      .saveTag(tag.id, name)
      .then(
        () => finish(refocus),
        (e: unknown) => {
          // The store toasts other failures; a field error, such as a taken name, shows here.
          settled.current = false;
          const { field, message } = toAppError(e);
          setError(field ? message : null);
        },
      );
  }

  return {
    rowRef,
    fieldRef,
    draft,
    error,
    start,
    change: (value: string) => {
      setDraft(value);
      setError(null);
    },
    blur: () => commit(false),
    fieldKeyDown: (e: KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter") {
        e.preventDefault();
        commit(true);
      } else if (e.key === "Escape") {
        e.preventDefault();
        finish(true);
      }
    },
    rowKeyDown: (e: KeyboardEvent<HTMLLIElement>) => {
      if (e.key !== "Enter" || e.target !== e.currentTarget) return;
      e.preventDefault();
      start();
    },
  };
}
