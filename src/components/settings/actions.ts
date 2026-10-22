import { api } from "../../api";
import { formatDay } from "../../lib/format";
import { fail } from "../../state/requests";
import { useStore } from "../../state/store";

/** Asks where to write a backup of all data, writes it and says what it holds. */
export async function exportBackup() {
  try {
    const path = await api.chooseSavePath(`timetracker-backup-${formatDay(Date.now())}`, "json");
    if (!path) return;
    const written = await api.backupExport(path);
    const count = (n: number) => n.toLocaleString("en-US");
    useStore.getState().toast("success", `Backup written: ${count(written.recordings)} recordings, ${count(written.activities)} activities`);
  } catch (e) {
    fail(e, "Backup failed");
  }
}

/** Shows `file` selected in its folder in Finder. */
export async function revealFolder(file: string) {
  try {
    await api.revealPath(file);
  } catch (e) {
    fail(e, "Couldn't reveal the folder");
  }
}

/** Shows the database file in the data folder in Finder. */
export async function revealDataFolder() {
  await revealFolder((await api.dataInfo()).file);
}

export function showWelcomeAgain() {
  const { closeDialog, showWelcome } = useStore.getState();
  closeDialog();
  showWelcome();
}
