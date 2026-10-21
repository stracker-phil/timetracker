import { X } from "lucide-react";
import type { ConditionInput } from "../../api";
import { FIELDS, OPS } from "./conditions";

interface Props {
  index: number;
  condition: ConditionInput;
  error?: string;
  onChange: (condition: ConditionInput) => void;
  /** Absent for the last condition: a rule needs one. */
  onRemove?: () => void;
}

/** One condition: a field, how it compares, and the value. */
export function ConditionRow({ index, condition, error, onChange, onRemove }: Props) {
  const label = `Condition ${index + 1}`;
  return (
    <div className="condition-row" role="group" aria-label={label} aria-invalid={error ? true : undefined}>
      <select className="input" aria-label={`${label} field`} value={condition.field} onChange={(e) => onChange({ ...condition, field: e.target.value as ConditionInput["field"] })}>
        {FIELDS.map((f) => (
          <option key={f.id} value={f.id}>
            {f.label}
          </option>
        ))}
      </select>
      <select className="input" aria-label={`${label} comparison`} value={condition.op} onChange={(e) => onChange({ ...condition, op: e.target.value as ConditionInput["op"] })}>
        {OPS.map((o) => (
          <option key={o.id} value={o.id}>
            {o.label}
          </option>
        ))}
      </select>
      <input
        className="input mono"
        aria-label={`${label} value`}
        placeholder={condition.op === "pattern" ? "* any text, ? one character" : "Value"}
        value={condition.value}
        onChange={(e) => onChange({ ...condition, value: e.target.value })}
      />
      <button type="button" className="icon-btn" aria-label={`Remove condition ${index + 1}`} disabled={!onRemove} onClick={onRemove}>
        <X size="0.923rem" aria-hidden="true" />
      </button>
      {error && (
        <span className="form-field-error condition-error" role="alert">
          {error}
        </span>
      )}
    </div>
  );
}
