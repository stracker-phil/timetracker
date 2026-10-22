import { Plus } from "lucide-react";
import type { TagListItem } from "../../api";
import { runCommand, useBindingLabel } from "../../commands";
import { plural } from "../../lib/format";
import { useStore } from "../../state/store";
import { Toolbar } from "../Toolbar";
import { TagRow } from "./TagRow";

/** The tags, one per recording, for kinds of work across projects; added, renamed and deleted here. */
export function TagsPage() {
  const tags = useStore((s) => s.tags);
  const newKeys = useBindingLabel("tags.new");

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

  return (
    <section className="view-page" aria-label="Tags">
      <Toolbar title="Tags" detail={<span className="toolbar-detail">One per recording, for kinds of work across projects.</span>}>
        <button type="button" className="btn" title={newKeys ? `New tag (${newKeys})` : undefined} onClick={() => runCommand("tags.new")}>
          <Plus size="1.154rem" /> New tag
        </button>
      </Toolbar>
      <div className="tags-page">
        {tags.length === 0 ? (
          <p className="o3">No tags yet.</p>
        ) : (
          <ul className="tag-rows">
            {tags.map((tag) => (
              <TagRow key={tag.id} tag={tag} onDelete={remove} />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
