import { ChevronDown } from "lucide-react";
import { useRef, useState, type KeyboardEvent, type MouseEvent, type ReactNode } from "react";
import { optionsOf, PickerList, type PickerRow } from "./PickerList";

interface Props<T> {
  /** The field's accessible name, e.g. `Project`. */
  label: string;
  /** The list's accessible name, e.g. `Projects`. */
  listLabel: string;
  icon: ReactNode;
  /** The value as text; `null` shows the placeholder. */
  text: string | null;
  placeholder: string;
  rowsFor: (query: string) => PickerRow<T>[];
  isCurrent: (value: T) => boolean;
  /** `query` is what was typed when the option was picked. */
  onPick: (value: T, query: string) => void;
  /** ⌫ on the field without a query: removes the value (pickers that allow none). */
  onClear?: () => void;
  /** Focus opens the list (an empty field waiting for a pick). */
  openOnFocus?: boolean;
  autoFocus?: boolean;
}

/**
 * A combobox that shows its value and only focuses on Tab or a first click (with `openOnFocus`, both open the list). Typing, ↓ ↑, Space, a second
 * click or the chevron open the list on the current option; ↵ picks, Esc closes the list and keeps the
 * value. With the list closed, ↵ and Esc belong to the form around it.
 */
export function PickerField<T>({ label, listLabel, icon, text, placeholder, rowsFor, isCurrent, onPick, onClear, openOnFocus, autoFocus }: Props<T>) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const fieldRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const rows = rowsFor(query);
  const options = optionsOf(rows);
  const active = Math.max(0, Math.min(index, options.length - 1));

  const show = (typed: string) => {
    setOpen(true);
    setQuery(typed);
    setIndex(typed ? 0 : Math.max(0, optionsOf(rowsFor(typed)).findIndex(isCurrent)));
  };
  const close = () => {
    setOpen(false);
    setQuery("");
  };
  const pick = (value: T) => {
    close();
    onPick(value, query);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.metaKey || e.ctrlKey || e.altKey || e.nativeEvent.isComposing) return;
    const clear = e.key === "Backspace" && (!open || query === "") && onClear && text !== null;
    if (clear) {
      e.preventDefault();
      onClear();
    } else if (!open) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === " ") {
        e.preventDefault();
        show("");
      } else if (e.key.length === 1) {
        // The field shows the value while closed: the first key starts a new query.
        e.preventDefault();
        show(e.key);
      } else if (e.key === "Backspace" || e.key === "Delete") {
        e.preventDefault();
      }
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      setIndex(e.key === "ArrowDown" ? Math.min(options.length - 1, active + 1) : Math.max(0, active - 1));
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      close();
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (options.length > 0) pick(options[active]);
    }
  };

  // A first click only focuses; a click on the focused field opens the list.
  const onFieldMouseDown = (e: MouseEvent) => {
    const input = inputRef.current;
    if (!input) return;
    const focused = document.activeElement === input;
    if (e.target !== input) {
      e.preventDefault();
      input.focus();
    }
    if (focused && !open) show("");
  };

  return (
    <div className="picker">
      <div ref={fieldRef} className="picker-field" onMouseDown={onFieldMouseDown}>
        <span className="picker-icon" aria-hidden="true">
          {icon}
        </span>
        <input
          ref={inputRef}
          aria-label={label}
          role="combobox"
          aria-expanded={open}
          autoFocus={autoFocus}
          autoComplete="off"
          spellCheck={false}
          placeholder={open ? (text ?? placeholder) : placeholder}
          value={open ? query : (text ?? "")}
          onFocus={() => openOnFocus && !open && show("")}
          onBlur={close}
          onChange={(e) => show(e.target.value)}
          onKeyDown={onKeyDown}
        />
        <button
          type="button"
          tabIndex={-1}
          className="picker-chevron"
          aria-label={`Show ${listLabel.toLowerCase()}`}
          onMouseDown={(e) => {
            e.preventDefault();
            e.stopPropagation();
            inputRef.current?.focus();
          }}
          onClick={() => (open ? close() : show(""))}
        >
          <ChevronDown size="0.923rem" aria-hidden="true" />
        </button>
      </div>
      {open && options.length > 0 && <PickerList anchor={fieldRef} label={listLabel} rows={rows} active={active} isCurrent={isCurrent} onHover={setIndex} onPick={pick} />}
    </div>
  );
}
