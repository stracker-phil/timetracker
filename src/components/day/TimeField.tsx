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
  id: string;
  value: number;
  /** ↑ and ↓ move by this (ms). */
  step: number;
  onChange: (ms: number) => void;
}

/** A clock time typed as `HH:MM`; a typo reverts on blur. */
export function TimeField({ id, value, step, onChange }: Props) {
  const [text, setText] = useState(formatClock(value));
  const change = (ms: number) => {
    setText(formatClock(ms));
    onChange(ms);
  };
  const commit = () => {
    const ms = atClock(value, text);
    if (ms === null) setText(formatClock(value));
    else change(ms);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
    e.preventDefault();
    change(value + (e.key === "ArrowUp" ? step : -step));
  };
  return <input id={id} className="input mono time-input" value={text} onChange={(e) => setText(e.target.value)} onBlur={commit} onKeyDown={onKeyDown} />;
}
