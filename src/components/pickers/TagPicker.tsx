import { Plus, Tag as TagIcon } from "lucide-react";
import type { TagListItem } from "../../api";
import { fuzzyFilter } from "../../lib/fuzzy";
import { useStore } from "../../state/store";
import { PickerField } from "./PickerField";
import type { PickerRow } from "./PickerList";

type Choice = { kind: "none" } | { kind: "tag"; id: string } | { kind: "new"; name: string };
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
      content: <span className="grow-text">{t.name}</span>,
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

/** One tag or none: picks or creates a tag; ⌫ removes it (see `PickerField` for the keys). */
export function TagPicker({ value, onChange }: Props) {
  const tags = useStore((s) => s.tags);
  const chosen = value === null ? null : (tags.find((t) => t.id === value)?.name ?? value);

  const pick = (choice: Choice) => {
    if (choice.kind === "none") return onChange(null);
    if (choice.kind === "tag") return onChange(choice.id);
    void useStore
      .getState()
      .saveTag(null, choice.name)
      .then((tag) => onChange(tag.id), () => {});
  };

  return (
    <PickerField
      label="Tag"
      listLabel="Tags"
      icon={<TagIcon size="1rem" />}
      text={chosen}
      placeholder="No tag"
      rowsFor={(query) => rowsFor(query, tags)}
      isCurrent={(choice) => (choice.kind === "none" ? value === null : choice.kind === "tag" && choice.id === value)}
      onPick={pick}
      onClear={() => onChange(null)}
    />
  );
}
