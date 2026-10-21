import { useStore } from "../state/store";
import { Modal } from "./Modal";
import { ClientSheet } from "./projects/ClientSheet";
import { ProjectSheet } from "./projects/ProjectSheet";
import { SettingsDialog } from "./settings/SettingsDialog";
import { ShortcutsDialog } from "./ShortcutsDialog";

function ConfirmDialog({ title, message, confirmLabel, onConfirm }: { title: string; message: string; confirmLabel: string; onConfirm: () => void }) {
  const close = useStore((s) => s.closeDialog);
  return (
    <Modal onClose={close} labelledBy="confirm-title">
      <h2 id="confirm-title">{title}</h2>
      <p className="muted" style={{ margin: 0 }}>{message}</p>
      <div className="actions">
        <button className="btn" onClick={close}>Cancel</button>
        <button
          className="btn btn-danger"
          autoFocus
          onClick={() => {
            close();
            onConfirm();
          }}
        >
          {confirmLabel}
        </button>
      </div>
    </Modal>
  );
}

export function Dialogs() {
  const dialog = useStore((s) => s.dialog);
  switch (dialog?.type) {
    case "settings":
      return <SettingsDialog />;
    case "shortcuts":
      return <ShortcutsDialog scopes={dialog.scopes} />;
    case "confirm":
      return <ConfirmDialog {...dialog} />;
    case "project":
      return <ProjectSheet key={dialog.id ?? ""} id={dialog.id} initialClientId={dialog.clientId} onClose={useStore.getState().closeDialog} />;
    case "client":
      return <ClientSheet key={dialog.id ?? ""} id={dialog.id} onClose={useStore.getState().closeDialog} />;
    default:
      return null;
  }
}
