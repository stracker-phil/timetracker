import { useId, useState } from "react";
import { api, toAppError, type Rule, type RuleInput } from "../../api";
import { useApiResult } from "../../lib/useApiResult";
import { useStore } from "../../state/store";
import { FormField } from "../Field";
import { Modal } from "../Modal";
import { NoteInput } from "../NoteInput";
import { ProjectPicker } from "../pickers/ProjectPicker";
import { TagPicker } from "../pickers/TagPicker";
import { ConditionGroups } from "./ConditionGroups";
import { emptyRule } from "./conditions";
import { RulePreviewPanel } from "./RulePreviewPanel";

interface Props {
  /** The rule to edit; absent creates one. */
  id?: string;
  draft?: RuleInput;
  onClose: () => void;
}

const inputOf = ({ name, projectId, enabled, note, tagId, conditions }: Rule): RuleInput => ({
  name,
  projectId,
  enabled,
  note,
  tagId,
  conditions: conditions.map(({ field, op, value }) => ({ field, op, value })),
});

function confirmDelete(id: string, name: string, onDeleted: () => void) {
  const { openDialog, deleteRule } = useStore.getState();
  openDialog({
    type: "confirm",
    title: "Delete rule",
    message: `Delete the rule "${name}"? Recordings stay.`,
    confirmLabel: "Delete",
    onConfirm: () => void deleteRule(id).then(onDeleted),
  });
}

/** Create or edit a rule, with the activity its conditions match beside it. Nothing is stored before Save. */
export function RuleSheet({ id, draft, onClose }: Props) {
  const existing = useStore((s) => s.rules.find((r) => r.id === id));
  const [input, setInput] = useState<RuleInput>(() => (existing ? inputOf(existing) : (draft ?? emptyRule())));
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const ids = { name: useId(), note: useId(), enabled: useId() };
  const patch = (p: Partial<RuleInput>) => setInput((current) => ({ ...current, ...p }));
  const { data: apps } = useApiResult("apps", api.appsList);
  const appKeyOf = (app: string) => apps?.find((a) => a.app.toLowerCase() === app.trim().toLowerCase())?.appKey ?? null;

  async function save() {
    setErrors({});
    setSaving(true);
    try {
      await useStore.getState().saveRule(id ?? null, input);
      onClose();
    } catch (e) {
      const field = toAppError(e).field;
      if (field) setErrors({ [field.path]: field.message });
    } finally {
      setSaving(false);
    }
  }

  return (
    <Modal onClose={onClose} labelledBy="rule-sheet-title" className="rule-sheet">
      <form
        className="rule-sheet-form form-contents"
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <h2 id="rule-sheet-title">{existing ? "Edit rule" : "New rule"}</h2>
        <FormField label="Name" htmlFor={ids.name} error={errors.name}>
          <input id={ids.name} className="input" autoFocus value={input.name} onChange={(e) => patch({ name: e.target.value })} />
        </FormField>
        <FormField label="Project" error={errors.projectId}>
          <ProjectPicker value={input.projectId || null} onChange={(projectId) => patch({ projectId })} />
        </FormField>
        <FormField label="Note" htmlFor={ids.note} error={errors.note}>
          <NoteInput id={ids.note} maxLength={500} value={input.note} onChange={(note) => patch({ note })} />
        </FormField>
        <FormField label="Tag" error={errors.tagId}>
          <TagPicker value={input.tagId} onChange={(tagId) => patch({ tagId })} />
        </FormField>
        <FormField label="Conditions" error={errors.conditions}>
          <ConditionGroups conditions={input.conditions} errors={errors} onChange={(conditions) => patch({ conditions })} appKeyOf={appKeyOf} />
        </FormField>
        <label className="rule-enabled" htmlFor={ids.enabled}>
          <input id={ids.enabled} className="switch" type="checkbox" role="switch" checked={input.enabled} onChange={(e) => patch({ enabled: e.target.checked })} />
          Suggest recordings
        </label>
        <div className="actions">
          {existing && (
            <button type="button" className="btn btn-danger actions-start" onClick={() => confirmDelete(existing.id, existing.name, onClose)}>
              Delete
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
      <RulePreviewPanel input={input} />
    </Modal>
  );
}
