import { Plus } from "lucide-react";
import type { ConditionInput, RuleField } from "../../api";
import { ConditionRow } from "./ConditionRow";
import { emptyCondition, FIELDS, fieldLabel, groupConditions } from "./conditions";

interface Props {
  conditions: ConditionInput[];
  /** Field errors by path, `conditions.<index>.value` marking a row. */
  errors: Record<string, string>;
  onChange: (conditions: ConditionInput[]) => void;
  appKeyOf?: (app: string) => string | null;
}

/** The rule's conditions as one section per field, each with a + that adds a value; buttons below add an unused field. */
export function ConditionGroups({ conditions, errors, onChange, appKeyOf }: Props) {
  const groups = groupConditions(conditions);
  const unused = FIELDS.filter((f) => !groups.some((g) => g.field === f.id));
  const add = (field: RuleField) => onChange([...conditions, emptyCondition(field)]);
  const set = (index: number, condition: ConditionInput) => onChange(conditions.map((c, i) => (i === index ? condition : c)));
  const removeAt = (index: number) => onChange(conditions.filter((_, i) => i !== index));

  return (
    <div className="conditions">
      <p className="condition-hint">Each field matches any of its values; the rule needs every field.</p>
      {groups.map(({ field, items }) => {
        const label = fieldLabel(field);
        return (
          <section key={field} className="condition-group" aria-label={label}>
            <header className="condition-group-head">
              <span className="condition-group-label">{label}</span>
              <button type="button" className="icon-btn" aria-label={`Add ${label} value`} title="Any of the values matches" onClick={() => add(field)}>
                <Plus size="0.923rem" aria-hidden="true" />
              </button>
            </header>
            {items.map(({ condition, index }, i) => (
              <div key={index} className="condition-item">
                <span className="condition-or" aria-hidden="true">
                  {i > 0 && "or"}
                </span>
                <ConditionRow
                  label={`${label} ${i + 1}`}
                  condition={condition}
                  error={errors[`conditions.${index}.value`]}
                  onChange={(c) => set(index, c)}
                  onRemove={conditions.length > 1 ? () => removeAt(index) : undefined}
                  appKeyOf={appKeyOf}
                />
              </div>
            ))}
          </section>
        );
      })}
      {unused.length > 0 && (
        <div className="condition-add" role="group" aria-label="Add a field">
          {unused.map((f) => (
            <button key={f.id} type="button" className="btn btn-quiet" onClick={() => add(f.id)}>
              <Plus size="0.923rem" aria-hidden="true" />
              {f.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
