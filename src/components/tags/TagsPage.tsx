import { Plus, X } from "lucide-react";
import { useState } from "react";
import type { TagListItem } from "../../api";
import { plural } from "../../lib/format";
import { useStore } from "../../state/store";
import { InsetField } from "../Field";
import { Toolbar } from "../Toolbar";

/** The tags, one per recording, for kinds of work across projects; added and deleted here. */
export function TagsPage() {
  const tags = useStore((s) => s.tags);
  const [draft, setDraft] = useState("");

  function remove(tag: TagListItem) {
    const { deleteTag, openDialog } = useStore.getState();
    if (tag.recordings === 0) return void deleteTag(tag.id).catch(() => {});
    openDialog({
      type: "confirm",
      title: `Delete the tag ${tag.name}?`,
      message: `${plural(tag.recordings, "recording carries", "recordings carry")} it; they keep their time and lose the tag.`,
      confirmLabel: "Delete",
      onConfirm: () => void deleteTag(tag.id).catch(() => {}),
    });
  }

  function add() {
    const name = draft.trim();
    if (!name) return;
    void useStore
      .getState()
      .saveTag(null, name)
      .then(() => setDraft(""), () => {});
  }

  return (
    <section className="view-page" aria-label="Tags">
      <Toolbar title="Tags" detail={<span className="toolbar-detail">One per recording, for kinds of work across projects.</span>} />
      <div className="tags-page">
        <ul className="tag-rows">
          {tags.map((tag) => (
            <li key={tag.id} className="tag-row">
              <span className="grow-text">{tag.name}</span>
              <span className="o2 tabular">{plural(tag.recordings, "recording", "recordings")}</span>
              <button type="button" className="icon-btn" aria-label={`Delete tag ${tag.name}`} title="Delete" onClick={() => remove(tag)}>
                <X size="1.077rem" />
              </button>
            </li>
          ))}
        </ul>
        <InsetField icon={<Plus size="1.077rem" />} className="tag-add">
          <input
            aria-label="New tag"
            placeholder="New tag"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") add();
            }}
          />
        </InsetField>
      </div>
    </section>
  );
}
