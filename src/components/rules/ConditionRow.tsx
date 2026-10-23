import { X } from "lucide-react";
import type { ConditionInput } from "../../api";
import { AppIcon } from "../AppIcon";
import { OPS } from "./conditions";

interface Props {
  /** `App 2`: the field and the row's place in its group. */
  label: string;
  condition: ConditionInput;
  error?: string;
  onChange: (condition: ConditionInput) => void;
  /** Absent for the rule's last condition: a rule needs one. */
  onRemove?: () => void;
  /** The bundle id (or name) of a tracked app by its name, for the icon of an `app` condition. */
  appKeyOf?: (app: string) => string | null;
}

/** One value of a field's group: how it compares, and the value. */
export function ConditionRow({ label, condition, error, onChange, onRemove, appKeyOf }: Props) {
  const app = condition.field === "app" ? condition.value.trim() : "";
  return (
    <div className="condition-row" role="group" aria-label={label} aria-invalid={error ? true : undefined}>
      <select className="input condition-op" aria-label={`${label} comparison`} value={condition.op} onChange={(e) => onChange({ ...condition, op: e.target.value as ConditionInput["op"] })}>
        {OPS.map((o) => (
          <option key={o.id} value={o.id}>
            {o.label}
          </option>
        ))}
      </select>
      <div className="condition-value">
        {app && <AppIcon app={app} bundleId={appKeyOf?.(app) ?? null} />}
        <input
          className="input"
          aria-label={`${label} value`}
          placeholder={condition.op === "pattern" ? "* any text, ? one character" : "Value"}
          value={condition.value}
          onChange={(e) => onChange({ ...condition, value: e.target.value })}
        />
      </div>
      <button type="button" className="icon-btn" aria-label={`Remove ${label}`} disabled={!onRemove} onClick={onRemove}>
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
