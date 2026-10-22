import { Clock, FileText, type LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import type { SavedReport } from "../api";
import { useBindingLabel } from "../commands";
import { today } from "../lib/day";
import { useStore } from "../state/store";
import { useContextMenu, type MenuEntry } from "./ContextMenu";
import { KeyCap } from "./Field";
import { MiniCalendar } from "./MiniCalendar";
import { confirmDeleteReport, renameReport } from "./reports/actions";
import { SidebarStatus } from "./SidebarStatus";
import { VIEWS } from "./views";

/** The pages below the calendar, which shows the Day, Week and Month itself; a separator between the groups. */
const PAGES = [VIEWS.filter((v) => v.id === "reports"), VIEWS.filter((v) => v.id === "projects" || v.id === "tags" || v.id === "rules")];

interface ItemProps {
  current: boolean;
  icon: LucideIcon;
  label: string;
  onClick: () => void;
  /** Trailing content, such as a key cap. */
  children?: ReactNode;
}

function SidebarItem({ current, icon: Icon, label, onClick, children }: ItemProps) {
  return (
    <button type="button" className={current ? "si sel" : "si"} aria-current={current ? "page" : undefined} onClick={onClick}>
      <Icon size="1.077rem" aria-hidden="true" />
      <span className="label">{label}</span>
      {children}
    </button>
  );
}

function NavEntry({ view }: { view: (typeof VIEWS)[number] }) {
  const current = useStore((s) => s.view === view.id);
  const keys = useBindingLabel(`view.${view.id}`);
  return (
    <SidebarItem current={current} icon={view.icon} label={view.label} onClick={() => useStore.getState().setView(view.id)}>
      <KeyCap keys={keys} />
    </SidebarItem>
  );
}

/** Today's Day view, ready to record today. */
function HoursEntry() {
  const current = useStore((s) => s.view === "day" && s.day === today());
  return <SidebarItem current={current} icon={Clock} label="Hours" onClick={() => useStore.getState().goToDay(today())} />;
}

const savedReportMenu = (report: SavedReport): MenuEntry[] => [
  { label: "Rename…", action: () => renameReport(report) },
  { label: "Delete", action: () => confirmDeleteReport(report) },
];

/** The saved reports under Reports; a click shows one, right-click renames or deletes it. */
function SavedReports() {
  const saved = useStore((s) => s.savedReports);
  const menu = useContextMenu();
  if (saved.length === 0) return null;
  return (
    <div className="sidebar-saved" role="group" aria-label="Saved reports">
      {saved.map((report) => (
        <button
          key={report.id}
          type="button"
          className="si si-sub"
          onClick={() => useStore.getState().openSavedReport(report)}
          onContextMenu={(e) => menu.open(e, savedReportMenu(report))}
        >
          <FileText size="0.923rem" aria-hidden="true" />
          <span className="label">{report.name}</span>
        </button>
      ))}
      {menu.element}
    </div>
  );
}

export function Sidebar() {
  return (
    <aside className="sidebar" aria-label="Sidebar">
      <div className="sidebar-titlebar" data-tauri-drag-region />
      <div className="sidebar-body">
        <MiniCalendar />
        <nav className="sidebar-nav" aria-label="Views">
          <HoursEntry />
          {PAGES.map((group, i) => (
            <div key={group[0].id} className="sidebar-group">
              {i > 0 && <hr className="sidebar-sep" />}
              {group.map((view) => (
                <div key={view.id} className="sidebar-entry">
                  <NavEntry view={view} />
                  {view.id === "reports" && <SavedReports />}
                </div>
              ))}
            </div>
          ))}
        </nav>
      </div>
      <SidebarStatus />
    </aside>
  );
}
