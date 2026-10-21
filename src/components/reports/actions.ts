import { api } from "../../api";
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
