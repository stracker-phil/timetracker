import { api, type SavedReport } from "../../api";
import { copyText } from "../../lib/clipboard";
import { defaultFilter } from "../../state/slices/reports";
import { fail, useStore } from "../../state/store";

export async function copyReport() {
  const s = useStore.getState();
  try {
    await copyText(await api.reportText(s.reportFilter ?? defaultFilter(s.settings)), "the report");
  } catch (e) {
    fail(e, "Could not build the report");
  }
}

export function promptSaveReport() {
  const { openDialog, saveReport } = useStore.getState();
  openDialog({ type: "prompt", title: "Save report", label: "Name", initial: "", confirmLabel: "Save", onSubmit: (name) => void saveReport(name) });
}

export function renameReport(report: SavedReport) {
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

export function confirmDeleteReport(report: SavedReport) {
  const { openDialog, deleteSavedReport } = useStore.getState();
  openDialog({
    type: "confirm",
    title: "Delete report",
    message: `Delete the saved report "${report.name}"? The recordings stay.`,
    confirmLabel: "Delete",
    onConfirm: () => void deleteSavedReport(report.id),
  });
}

const openedReport = () => {
  const { savedReports, openedReport: id } = useStore.getState();
  return savedReports.find((r) => r.id === id);
};

export function promptRenameSavedReport() {
  const report = openedReport();
  if (report) renameReport(report);
}

export function promptDeleteSavedReport() {
  const report = openedReport();
  if (report) confirmDeleteReport(report);
}
