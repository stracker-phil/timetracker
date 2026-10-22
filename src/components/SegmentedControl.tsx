import { capitalize } from "../lib/format";

export interface SegmentedControlProps<T extends string> {
  label: string;
  options: readonly T[];
  /** `null` presses none. */
  value: T | null;
  onChange: (v: T) => void;
}

/** A row of mutually exclusive buttons; the pressed one is raised (docs/ux.md, Components). */
export function SegmentedControl<T extends string>({ label, options, value, onChange }: SegmentedControlProps<T>) {
  return (
    <div className="segmented" role="group" aria-label={label}>
      {options.map((o) => (
        <button key={o} type="button" aria-pressed={value === o} onClick={() => onChange(o)}>
          {capitalize(o)}
        </button>
      ))}
    </div>
  );
}
