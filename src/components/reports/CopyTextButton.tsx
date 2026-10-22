import { ChevronDown, Copy } from "lucide-react";
import type { Group } from "../../api";
import { useBindingLabel } from "../../commands";
import { useContextMenu } from "../ContextMenu";
import { copyReport } from "./actions";

const GROUPS: readonly Group[] = ["project", "tag", "day", "week", "month"];
const MENU_GAP = 4;

interface Props {
  groupBy: Group;
  onGroupBy: (group: Group) => void;
}

/** Copy as text as a split button: the main part copies the report grouped by `groupBy`, the chevron picks the grouping. */
export function CopyTextButton({ groupBy, onGroupBy }: Props) {
  const copyKeys = useBindingLabel("reports.copyText");
  const menu = useContextMenu();
  const openMenu = (button: HTMLElement) => {
    const rect = button.getBoundingClientRect();
    menu.open(
      { x: rect.left, y: rect.bottom + MENU_GAP },
      GROUPS.map((group) => ({ label: `By ${group}`, checked: group === groupBy, action: () => onGroupBy(group) })),
    );
  };
  return (
    <div className="split-btn" role="group" aria-label="Copy as text">
      <button type="button" className="btn" title={`Copy as text, by ${groupBy}${copyKeys ? ` (${copyKeys})` : ""}`} onClick={() => void copyReport()}>
        <Copy size="1.154rem" /> Copy as text
      </button>
      <button
        type="button"
        className="btn split-btn-menu"
        aria-label="Group copied text by"
        aria-haspopup="menu"
        aria-expanded={menu.element !== null}
        onClick={(e) => openMenu(e.currentTarget)}
      >
        <ChevronDown size="1rem" />
      </button>
      {menu.element}
    </div>
  );
}
