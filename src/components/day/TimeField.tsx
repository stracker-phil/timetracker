import { useState, type KeyboardEvent } from "react";
import { formatClock } from "../../lib/format";

/** `HH:MM` on the local day of `day`; null when it is not a time. Rust snaps what the editor sends. */
export function atClock(day: number, text: string): number | null {
  const match = /^(\d{1,2}):?(\d{2})$/.exec(text.trim());
  if (!match) return null;
  const [hours, minutes] = [Number(match[1]), Number(match[2])];
  if (hours > 23 || minutes > 59) return null;
  const date = new Date(day);
  date.setHours(hours, minutes, 0, 0);
  return date.getTime();
}

interface Props {
  id?: string;
  /** The accessible name, without a visible label. */
  label?: string;
  value: number;
  /** ↑ and ↓ move by this (ms). */
  step: number;
  onChange: (ms: number) => void;
}

/**
 * A clock time typed as `HH:MM`; each valid time counts at once (so ↵ saves it), a typo reverts on blur. The
 * typed text shows while it belongs to the value; a value changed from outside (a new selection) shows instead.
 */
export function TimeField({ id, label, value, step, onChange }: Props) {
  const [typed, setTyped] = useState<{ text: string; value: number } | null>(null);
  const text = typed && typed.value === value ? typed.text : formatClock(value);
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
    e.preventDefault();
    setTyped(null);
    onChange(value + (e.key === "ArrowUp" ? step : -step));
  };
  const type = (next: string) => {
    const ms = atClock(value, next);
    setTyped({ text: next, value: ms ?? value });
    if (ms !== null) onChange(ms);
  };
  return <input id={id} aria-label={label} className="input time-input" value={text} onChange={(e) => type(e.target.value)} onBlur={() => setTyped(null)} onKeyDown={onKeyDown} />;
}
