import { useId, useState } from "react";
import { toAppError } from "../../api";
import { useStore } from "../../state/store";
import { Modal } from "../Modal";
import { FormField } from "../Field";

/** Create (no `id`) or rename a client; Archive also archives its projects. */
export function ClientSheet({ id, onClose }: { id?: string; onClose: () => void }) {
  const existing = useStore((s) => s.clients.find((c) => c.id === id));
  const { saveClient, archiveClient } = useStore.getState();
  const [name, setName] = useState(existing?.name ?? "");
  const [error, setError] = useState<string>();
  const nameId = useId();

  async function save() {
    try {
      await saveClient(id ?? null, name);
      onClose();
    } catch (e) {
      setError(toAppError(e).field?.message);
    }
  }

  return (
    <Modal onClose={onClose} labelledBy="client-sheet-title">
      <form
        className="form-contents"
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <h2 id="client-sheet-title">{existing ? "Edit client" : "New client"}</h2>
        <FormField label="Name" htmlFor={nameId} error={error}>
          <input id={nameId} className="input" autoFocus value={name} onChange={(e) => setName(e.target.value)} />
        </FormField>
        <div className="actions">
          {existing && (
            <button
              type="button"
              className="btn actions-start"
              title={existing.archivedAt === null ? "Also archives the client's projects" : undefined}
              onClick={() => {
                onClose();
                void archiveClient(existing.id, existing.archivedAt === null).catch(() => {});
              }}
            >
              {existing.archivedAt === null ? "Archive" : "Restore"}
            </button>
          )}
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="submit" className="btn btn-primary">
            Save
          </button>
        </div>
      </form>
    </Modal>
  );
}
