import { FileText } from "lucide-react";
import type { SavedReport } from "../api";
import { useBindingLabel } from "../commands";
import { useStore } from "../state/store";
import { useContextMenu, type MenuEntry } from "./ContextMenu";
import { KeyCap } from "./Field";
import { TrackerStatus } from "./TrackerStatus";
import { VIEWS } from "./views";

function NavEntry({ view }: { view: (typeof VIEWS)[number] }) {
  const current = useStore((s) => s.view === view.id);
  const setView = useStore((s) => s.setView);
  const keys = useBindingLabel(`view.${view.id}`);
  const Icon = view.icon;
  return (
    <button type="button" className={current ? "si sel" : "si"} aria-current={current ? "page" : undefined} onClick={() => setView(view.id)}>
      <Icon size="1.077rem" aria-hidden="true" />
      <span className="label">{view.label}</span>
      <KeyCap keys={keys} />
    </button>
  );
}

function renameReport(report: SavedReport) {
  const { openDialog, renameSavedReport } = useStore.getState();
  openDialog({
    type: "prompt",
    title: "Rename report",
    label: "Name",
    initial: report.name,
    confirmLabel: "Rename",
    onSubmit: (name) => void renameSavedReport(report.id, name),
  });
}

function confirmDeleteReport(report: SavedReport) {
  const { openDialog, deleteSavedReport } = useStore.getState();
  openDialog({
    type: "confirm",
    title: "Delete report",
    message: `Delete the saved report "${report.name}"? The recordings stay.`,
    confirmLabel: "Delete",
    onConfirm: () => void deleteSavedReport(report.id),
  });
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
      <div className="sidebar-body">
        <nav className="sidebar-nav" aria-label="Views">
          {VIEWS.map((view) => (
            <div key={view.id} className="sidebar-entry">
              <NavEntry view={view} />
              {view.id === "reports" && <SavedReports />}
            </div>
          ))}
        </nav>
      </div>
      <TrackerStatus />
    </aside>
  );
}
