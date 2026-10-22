import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactNode, type RefObject } from "react";
import { createPortal } from "react-dom";
import { Check } from "lucide-react";

export type PickerRow<T> = { kind: "section"; label: string } | { kind: "option"; key: string; value: T; content: ReactNode };

interface Props<T> {
  anchor: RefObject<HTMLElement | null>;
  label: string;
  rows: PickerRow<T>[];
  /** Index into the options only (sections are skipped). */
  active: number;
  /** Marks the picker's value with a check. */
  isCurrent?: (value: T) => boolean;
  onHover: (index: number) => void;
  onPick: (value: T) => void;
}

/** The options of a picker, below its field in a portal, so a sheet's overflow never clips them. */
export function PickerList<T>({ anchor, label, rows, active, isCurrent, onHover, onPick }: Props<T>) {
  const [box, setBox] = useState<{ left: number; top: number; width: number } | null>(null);
  const activeRef = useRef<HTMLLIElement>(null);

  useLayoutEffect(() => {
    const rect = anchor.current?.getBoundingClientRect();
    if (rect) setBox({ left: rect.left, top: rect.bottom + 4, width: rect.width });
  }, [anchor]);

  useEffect(() => {
    activeRef.current?.scrollIntoView?.({ block: "nearest" });
  }, [active]);

  let option = -1;
  return createPortal(
    <ul
      className="palette-list picker-list"
      role="listbox"
      aria-label={label}
      style={box ?? undefined}
      // Keeps the focus in the field, so a click picks before the field's blur closes the list.
      onMouseDown={(e) => e.preventDefault()}
    >
      {rows.map((row) => {
        if (row.kind === "section") {
          return (
            <li key={`section:${row.label}`} className="palette-section" role="presentation">
              {row.label}
            </li>
          );
        }
        const index = ++option;
        const current = isCurrent?.(row.value) ?? false;
        return (
          <li
            key={row.key}
            ref={index === active ? activeRef : undefined}
            role="option"
            aria-selected={index === active}
            aria-checked={isCurrent ? current : undefined}
            className={index === active ? "active" : ""}
            onMouseEnter={() => onHover(index)}
            onClick={() => onPick(row.value)}
          >
            {row.content}
            {current && <Check className="picker-check" size="0.923rem" aria-hidden="true" />}
          </li>
        );
      })}
    </ul>,
    document.body,
  );
}

/** The options of `rows`, in order, for keyboard moves. */
export function optionsOf<T>(rows: PickerRow<T>[]): T[] {
  return rows.flatMap((row) => (row.kind === "option" ? [row.value] : []));
}

/** The highlighted option, kept within the `count` options as the list changes. */
export function usePickerIndex(count: number): [number, (index: number) => void] {
  const [index, setIndex] = useState(0);
  return [Math.max(0, Math.min(index, count - 1)), setIndex];
}

interface ListKeys {
  current: number;
  count: number;
  open: boolean;
  setIndex: (index: number) => void;
  setOpen: (open: boolean) => void;
}

/** ↑ ↓ move (opening the list) and Esc closes the list only, not the sheet around the picker; returns whether it took the key. */
export function listKeyDown(e: KeyboardEvent, { current, count, open, setIndex, setOpen }: ListKeys): boolean {
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    setOpen(true);
    setIndex(e.key === "ArrowDown" ? Math.min(count - 1, current + 1) : Math.max(0, current - 1));
    return true;
  }
  if (e.key === "Escape" && open) {
    e.stopPropagation();
    setOpen(false);
    return true;
  }
  return false;
}
