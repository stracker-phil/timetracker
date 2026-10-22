import { useId, useState } from "react";
import { toAppError, type Project, type ProjectInput } from "../../api";
import { useStore } from "../../state/store";
import { AccentSwatches, ShadeSwatches } from "../Accent";
import { FormField } from "../Field";
import { Modal } from "../Modal";

const NEW_CLIENT = "\u0000new";

interface Props {
  /** The project to edit; absent creates one. */
  id?: string;
  /** A new project's name, e.g. what was typed into a picker. */
  initialName?: string;
  /** The client a new project starts with. */
  initialClientId?: string;
  onClose: () => void;
  /** After a save, with the saved project. */
  onSaved?: (project: Project) => void;
}

/** Create or edit a project; archive or restore an existing one. */
export function ProjectSheet({ id, initialName = "", initialClientId, onClose, onSaved }: Props) {
  const existing = useStore((s) => s.projects.find((p) => p.project.id === id)?.project);
  const clients = useStore((s) => s.clients);
  const { saveProject, archiveProject, saveClient } = useStore.getState();
  const [input, setInput] = useState<ProjectInput>(() => {
    const { clientId, name, color, shade, billable } = existing ?? { clientId: initialClientId ?? null, name: initialName, color: "orange", shade: null, billable: false };
    return { clientId, name, color, shade, billable };
  });
  const [newClient, setNewClient] = useState<string | null>(null);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const ids = { name: useId(), client: useId(), newClient: useId() };
  // Archived clients only show when the project already belongs to one.
  const choices = clients.filter((c) => c.archivedAt === null || c.id === input.clientId);
  const patch = (p: Partial<ProjectInput>) => setInput((current) => ({ ...current, ...p }));
  // A client's color wins; a project without a client, or with a new one, picks its own.
  const client = newClient === null ? clients.find((c) => c.id === input.clientId) : undefined;
  const accent = client?.color ?? input.color;

  /** The field errors of a failed save; other errors were toasted by the store. */
  const fieldError = (e: unknown, rename: (path: string) => string = (path) => path) => {
    const field = toAppError(e).field;
    if (field) setErrors({ [rename(field.path)]: field.message });
  };

  async function save() {
    setErrors({});
    setSaving(true);
    try {
      let clientId = input.clientId;
      if (newClient !== null) {
        try {
          clientId = (await saveClient(null, { name: newClient, color: input.color })).id;
        } catch (e) {
          return fieldError(e, () => "clientName");
        }
        // Created once: a retry after a project error keeps this client.
        setNewClient(null);
        patch({ clientId });
      }
      try {
        const project = await saveProject(id ?? null, { ...input, clientId });
        onClose();
        onSaved?.(project);
      } catch (e) {
        fieldError(e);
      }
    } finally {
      setSaving(false);
    }
  }

  return (
    <Modal onClose={onClose} labelledBy="project-sheet-title" className="project-sheet">
      <form
        className="form-contents"
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <h2 id="project-sheet-title">{existing ? "Edit project" : "New project"}</h2>
        <FormField label="Name" htmlFor={ids.name} error={errors.name}>
          <input id={ids.name} className="input" autoFocus value={input.name} onChange={(e) => patch({ name: e.target.value })} />
        </FormField>
        <FormField label="Client" htmlFor={ids.client} error={errors.clientId}>
          <select
            id={ids.client}
            className="input"
            value={newClient !== null ? NEW_CLIENT : (input.clientId ?? "")}
            onChange={(e) => {
              if (e.target.value === NEW_CLIENT) return setNewClient("");
              setNewClient(null);
              patch({ clientId: e.target.value || null });
            }}
          >
            <option value="">No client</option>
            {choices.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
                {c.archivedAt !== null ? " (archived)" : ""}
              </option>
            ))}
            <option value={NEW_CLIENT}>New client…</option>
          </select>
        </FormField>
        {newClient !== null && (
          <FormField label="New client" htmlFor={ids.newClient} error={errors.clientName}>
            <input id={ids.newClient} className="input" autoFocus value={newClient} onChange={(e) => setNewClient(e.target.value)} />
          </FormField>
        )}
        {!client && (
          <FormField label="Color" error={errors.color}>
            <AccentSwatches value={input.color} onChange={(color) => patch({ color })} />
          </FormField>
        )}
        <FormField label={client ? `Shade of ${client.name}'s color` : "Shade"} error={errors.shade}>
          <ShadeSwatches accent={accent} value={input.shade} onChange={(shade) => patch({ shade })} />
        </FormField>
        <label className="form-switch">
          <input type="checkbox" className="switch" role="switch" checked={input.billable} onChange={(e) => patch({ billable: e.target.checked })} />
          <span>
            Billable
            <span className="form-switch-desc">Its recordings count as billable time.</span>
          </span>
        </label>
        <div className="actions">
          {existing && (
            <button
              type="button"
              className="btn actions-start"
              onClick={() => {
                onClose();
                void archiveProject(existing.id, existing.archivedAt === null).catch(() => {});
              }}
            >
              {existing.archivedAt === null ? "Archive" : "Restore"}
            </button>
          )}
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="submit" className="btn btn-primary" disabled={saving}>
            Save
          </button>
        </div>
      </form>
    </Modal>
  );
}
