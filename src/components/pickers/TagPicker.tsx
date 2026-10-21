import { Plus, Tag as TagIcon } from "lucide-react";
import { useRef, useState } from "react";
import type { TagListItem } from "../../api";
import { fuzzyFilter } from "../../lib/fuzzy";
import { useStore } from "../../state/store";
import { InsetField } from "../Field";
import { listKeyDown, optionsOf, PickerList, usePickerIndex, type PickerRow } from "./PickerList";

type Choice = { kind: "none" } | { kind: "tag"; id: string } | { kind: "new"; name: string };
/** The "No tag" row shows only while the query is empty. */
/** "No tag" and every tag while the query is empty; else the matches, and "New tag" for a new name. */
function rowsFor(query: string, tags: TagListItem[]): PickerRow<Choice>[] {
  const name = query.trim();
  const rows: PickerRow<Choice>[] = [];
  if (!name) rows.push({ kind: "option", key: "none", value: { kind: "none" }, content: <span className="grow-text o3">No tag</span> });
  rows.push(
    ...fuzzyFilter(tags, name, (t) => [t.name]).map((t): PickerRow<Choice> => ({
      kind: "option",
      key: t.id,
      value: { kind: "tag", id: t.id },
      content: <span className="mono grow-text">{t.name}</span>,
    })),
  );
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
  value: string | null;
  onChange: (tagId: string | null) => void;
}

/** One tag or none: typing filters, Enter picks or creates, ⌫ on an empty query removes the tag. */
export function TagPicker({ value, onChange }: Props) {
  const tags = useStore((s) => s.tags);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const fieldRef = useRef<HTMLDivElement>(null);
  const rows = rowsFor(query, tags);
  const options = optionsOf(rows);
  const [current, setIndex] = usePickerIndex(options.length);
  const chosen = value === null ? null : (tags.find((t) => t.id === value)?.name ?? value);

  const close = () => {
    setOpen(false);
    setQuery("");
    setIndex(0);
  };
  const pick = (choice: Choice) => {
    close();
    if (choice.kind === "none") return onChange(null);
    if (choice.kind === "tag") return onChange(choice.id);
    void useStore
      .getState()
      .saveTag(null, choice.name)
      .then((tag) => onChange(tag.id), () => {});
  };

  return (
    <div className="picker" ref={fieldRef}>
      <InsetField icon={<TagIcon size="1.077rem" />}>
        <input
          aria-label="Tag"
          role="combobox"
          aria-expanded={open}
          className={chosen !== null && !open ? "mono" : undefined}
          placeholder={open && chosen !== null ? chosen : "No tag"}
          value={open ? query : (chosen ?? "")}
          onFocus={() => setOpen(true)}
          onBlur={close}
          onChange={(e) => {
            setOpen(true);
            setQuery(e.target.value);
            setIndex(0);
          }}
          onKeyDown={(e) => {
            if (listKeyDown(e, { current, count: options.length, open, setIndex, setOpen: (o) => (o ? setOpen(true) : close()) })) return;
            if (e.key === "Enter" && open && options.length > 0) {
              e.preventDefault();
              pick(options[current]);
            } else if (e.key === "Backspace" && query === "" && value !== null) {
              e.preventDefault();
              onChange(null);
            }
          }}
        />
      </InsetField>
      {open && options.length > 0 && <PickerList anchor={fieldRef} label="Tags" rows={rows} active={current} onHover={setIndex} onPick={pick} />}
    </div>
  );
}
