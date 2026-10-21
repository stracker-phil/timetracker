import { Plus, Tag as TagIcon } from "lucide-react";
import { useRef, useState } from "react";
import type { TagListItem } from "../../api";
import { fuzzyFilter } from "../../lib/fuzzy";
import { useStore } from "../../state/store";
import { InsetField } from "../Field";
import { listKeyDown, optionsOf, PickerList, usePickerIndex, type PickerRow } from "./PickerList";

type Choice = { kind: "tag"; id: string } | { kind: "new"; name: string };

function rowsFor(query: string, tags: TagListItem[], chosen: string[]): PickerRow<Choice>[] {
  const name = query.trim();
  const free = tags.filter((t) => !chosen.includes(t.id));
  const rows: PickerRow<Choice>[] = fuzzyFilter(free, name, (t) => [t.name]).map((t) => ({
    kind: "option",
    key: t.id,
    value: { kind: "tag", id: t.id },
    content: <span className="mono grow-text">{t.name}</span>,
  }));
  const exists = tags.some((t) => t.name.toLowerCase() === name.toLowerCase());
  if (name && !exists) {
    rows.push({
      kind: "option",
      key: "new",
      value: { kind: "new", name },
      content: (
        <>
          <Plus size="1rem" aria-hidden="true" />
          <span className="grow-text">New tag "{name}"</span>
        </>
      ),
    });
  }
  return rows;
}

interface Props {
  value: string[];
  onChange: (tagIds: string[]) => void;
}

/** Chosen tags as `a · b` in the field; typing filters, Enter adds or creates, ⌫ on an empty query removes the last. */
export function TagPicker({ value, onChange }: Props) {
  const tags = useStore((s) => s.tags);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const fieldRef = useRef<HTMLDivElement>(null);
  const rows = rowsFor(query, tags, value);
  const options = optionsOf(rows);
  const [current, setIndex] = usePickerIndex(options.length);
  const names = value.map((id) => tags.find((t) => t.id === id)?.name ?? id);

  const add = (id: string) => {
    if (!value.includes(id)) onChange([...value, id]);
    setQuery("");
    setIndex(0);
  };
  const pick = (choice: Choice) => {
    if (choice.kind === "tag") return add(choice.id);
    void useStore
      .getState()
      .saveTag(null, choice.name)
      .then((tag) => add(tag.id), () => {});
  };

  return (
    <div className="picker" ref={fieldRef}>
      <InsetField icon={<TagIcon size="1.077rem" />} className="tag-picker">
        {names.length > 0 && <span className="mono tag-picker-chosen">{names.join(" · ")}</span>}
        <input
          aria-label="Tags"
          role="combobox"
          aria-expanded={open}
          placeholder={names.length ? "" : "Tags"}
          value={query}
          onFocus={() => setOpen(true)}
          onBlur={() => setOpen(false)}
          onChange={(e) => {
            setOpen(true);
            setQuery(e.target.value);
            setIndex(0);
          }}
          onKeyDown={(e) => {
            if (listKeyDown(e, { current, count: options.length, open, setIndex, setOpen })) return;
            if (e.key === "Enter" && options.length > 0) {
              e.preventDefault();
              pick(options[current]);
            } else if (e.key === "Backspace" && query === "" && value.length > 0) {
              e.preventDefault();
              onChange(value.slice(0, -1));
            }
          }}
        />
      </InsetField>
      {open && options.length > 0 && <PickerList anchor={fieldRef} label="Tags" rows={rows} active={current} onHover={setIndex} onPick={pick} />}
    </div>
  );
}
