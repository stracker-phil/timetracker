import { useRef, useState, type ReactNode } from "react";
import { fuzzyFilter } from "../../lib/fuzzy";
import { AccentDot } from "../Accent";
import { InsetField } from "../Field";
import { listKeyDown, optionsOf, PickerList, usePickerIndex, type PickerRow } from "../pickers/PickerList";

export interface SelectItem {
  id: string;
  label: string;
  accent?: string;
  shade?: number;
}

interface Props {
  /** The field's name, e.g. "Projects"; also its placeholder. */
  label: string;
  icon: ReactNode;
  items: SelectItem[];
  value: string[];
  onChange: (ids: string[]) => void;
}

/** Chosen items as `a · b`; typing filters, Enter adds, ⌫ on an empty query removes the last. None chosen means all. */
export function MultiSelect({ label, icon, items, value, onChange }: Props) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const fieldRef = useRef<HTMLDivElement>(null);
  const free = items.filter((item) => !value.includes(item.id));
  const rows: PickerRow<string>[] = fuzzyFilter(free, query.trim(), (item) => [item.label]).map((item) => ({
    kind: "option",
    key: item.id,
    value: item.id,
    content: (
      <>
        {item.accent && <AccentDot accent={item.accent} shade={item.shade} />}
        <span className="grow-text">{item.label}</span>
      </>
    ),
  }));
  const options = optionsOf(rows);
  const [current, setIndex] = usePickerIndex(options.length);
  const names = value.map((id) => items.find((item) => item.id === id)?.label ?? id);

  const add = (id: string) => {
    onChange([...value, id]);
    setQuery("");
    setIndex(0);
  };

  return (
    <div className="picker multi-select" ref={fieldRef}>
      <InsetField icon={icon}>
        {names.length > 0 && <span className="multi-select-chosen">{names.join(" · ")}</span>}
        <input
          aria-label={label}
          role="combobox"
          aria-expanded={open}
          placeholder={names.length ? "" : `All ${label.toLowerCase()}`}
          value={query}
          onFocus={() => setOpen(true)}
          onBlur={() => setOpen(false)}
          onChange={(e) => {
            setOpen(true);
            setQuery(e.target.value);
            setIndex(0);
          }}
          onKeyDown={(e) => {
            if (listKeyDown(e, { current, count: options.length, open, setIndex, setOpen })) return;
            if (e.key === "Enter" && options.length > 0) {
              e.preventDefault();
              add(options[current]);
            } else if (e.key === "Backspace" && query === "" && value.length > 0) {
              e.preventDefault();
              onChange(value.slice(0, -1));
            }
          }}
        />
      </InsetField>
      {open && options.length > 0 && <PickerList anchor={fieldRef} label={label} rows={rows} active={current} onHover={setIndex} onPick={add} />}
    </div>
  );
}
