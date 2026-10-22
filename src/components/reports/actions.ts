import { api, type CopiedNotes, type SavedReport } from "../../api";
import { copyText } from "../../lib/clipboard";
import { plural } from "../../lib/format";
import { selectReportFilter } from "../../state/slices/reports";
import { fail, useStore } from "../../state/store";

export async function copyReport() {
  try {
    await copyText(await api.reportText(selectReportFilter(useStore.getState())), "the report");
  } catch (e) {
    fail(e, "Could not build the report");
  }
}

/** Copies the notes and toasts how many, for `whose` when given. */
export async function copyNotes(notes: CopiedNotes, whose: string | null) {
  if (notes.count === 0) {
    useStore.getState().toast("info", whose ? `No notes for ${whose}` : "No notes in the report");
    return;
  }
  const what = plural(notes.count, "note", "notes");
  await copyText(notes.text, whose ? `${what} for ${whose}` : what);
}

/** Every note of the shown report, in time order. */
export async function copyReportNotes() {
  try {
    await copyNotes((await api.reportRun(selectReportFilter(useStore.getState()))).notes, null);
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
