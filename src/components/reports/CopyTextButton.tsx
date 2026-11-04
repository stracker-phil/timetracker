import { ChevronDown, Copy } from "lucide-react";
import { useState } from "react";
import type { Group } from "../../api";
import { useBindingLabel } from "../../commands";
import { openMenu, useMenuOpenOn } from "../menu";
import { copyReport } from "./actions";

const GROUPS: readonly Group[] = ["project", "tag", "day", "week", "month"];

interface Props {
  groupBy: Group;
  onGroupBy: (group: Group) => void;
}

/** Copy as text as a split button: the main part copies the report grouped by `groupBy`, the chevron picks the grouping. */
export function CopyTextButton({ groupBy, onGroupBy }: Props) {
  const copyKeys = useBindingLabel("reports.copyText");
  const [chevron, setChevron] = useState<HTMLButtonElement | null>(null);
  const open = useMenuOpenOn(chevron);
  const openGroups = (button: HTMLElement) =>
    openMenu(
      GROUPS.map((group) => ({ text: `By ${group}`, checked: group === groupBy, run: () => onGroupBy(group) })),
      button,
      "Group copied text by",
    );
  return (
    <div className="split-btn" role="group" aria-label="Copy as text">
      <button type="button" className="btn" title={`Copy as text, by ${groupBy}${copyKeys ? ` (${copyKeys})` : ""}`} onClick={() => void copyReport()}>
        <Copy size="1.154rem" /> Copy as text
      </button>
      <button
        ref={setChevron}
        type="button"
        className="btn split-btn-menu"
        aria-label="Group copied text by"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={(e) => openGroups(e.currentTarget)}
      >
        <ChevronDown size="1rem" />
      </button>
    </div>
  );
}
