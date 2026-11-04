import { ChevronsUpDown } from "lucide-react";
import { useState } from "react";
import { useStore } from "../../state/store";

export interface PopUpOption<T extends string | number> {
  value: T;
  label: string;
}

interface Props<T extends string | number> {
  value: T;
  options: readonly PopUpOption<T>[];
  onChange: (value: T) => void;
  "aria-label"?: string;
  /** For a `<label htmlFor>` naming it. */
  id?: string;
  disabled?: boolean;
  className?: string;
}

/** A macOS pop-up button: the current choice and a chevron, as wide as its widest choice, opening the choices. */
export function PopUpButton<T extends string | number>({ value, options, onChange, "aria-label": label, id, disabled, className }: Props<T>) {
  const [button, setButton] = useState<HTMLButtonElement | null>(null);
  const open = useStore((s) => button !== null && s.menu?.at === button);
  const current = options.find((o) => o.value === value);

  const show = () => {
    if (!button) return;
    const entries = options.map((o) => ({
      text: o.label,
      checked: o.value === value,
      run: () => {
        if (o.value !== value) onChange(o.value);
      },
    }));
    useStore.getState().openMenu(entries, button, label ?? labelOf(button));
  };

  return (
    <button
      ref={setButton}
      type="button"
      id={id}
      className={className ? `popup-button ${className}` : "popup-button"}
      aria-label={label}
      aria-haspopup="menu"
      aria-expanded={open}
      disabled={disabled}
      onClick={show}
      onKeyDown={(e) => {
        if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
        e.preventDefault();
        if (!open) show();
      }}
    >
      <span className="popup-button__value">
        <span>{current?.label ?? ""}</span>
        {options.map((o) => (
          <span key={String(o.value)} className="popup-button__sizer" aria-hidden="true">
            {o.label}
          </span>
        ))}
      </span>
      <ChevronsUpDown className="popup-button__chevron" size="0.846rem" strokeWidth={2.2} aria-hidden="true" />
    </button>
  );
}

/** The text of the `<label>` naming the button, for its menu's name. */
function labelOf(button: HTMLButtonElement): string | undefined {
  return button.labels?.[0]?.textContent ?? undefined;
}
