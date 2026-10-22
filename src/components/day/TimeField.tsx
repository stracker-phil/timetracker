import { useRef, useState, type KeyboardEvent } from "react";
import { api } from "../../api";
import { formatClock } from "../../lib/format";

interface Props {
  id?: string;
  /** The accessible name, without a visible label. */
  label?: string;
  value: number;
  /** ↑ and ↓ move by this (ms). */
  step: number;
  /** May return the promise of what it starts, which `typedTimesSettled` waits for too. */
  onChange: (ms: number) => void | Promise<unknown>;
}

const pending = new Set<Promise<unknown>>();

function track(work: void | Promise<unknown>) {
  if (!work) return;
  const done = () => void pending.delete(work);
  pending.add(work);
  work.then(done, done);
}

/** Resolves once every time field's typing has been read by Rust and passed on; a save awaits it first. */
export async function typedTimesSettled(): Promise<void> {
  while (pending.size > 0) await Promise.allSettled([...pending]);
}

/**
 * A clock time typed as `HH:MM`, read by Rust on the value's day; each valid time counts at once (so ↵ saves
 * it), a typo reverts on blur. The typed text shows while it belongs to the value; a value changed from
 * outside (a new selection) shows instead.
 */
export function TimeField({ id, label, value, step, onChange }: Props) {
  const [typed, setTyped] = useState<{ text: string; value: number } | null>(null);
  const latest = useRef(0);
  const text = typed && typed.value === value ? typed.text : formatClock(value);
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
    e.preventDefault();
    latest.current++;
    setTyped(null);
    track(onChange(value + (e.key === "ArrowUp" ? step : -step)));
  };
  const type = (next: string) => {
    const asked = ++latest.current;
    setTyped({ text: next, value });
    track(
      api.calendarParseTime(next, value).then(
        (ms) => {
          if (asked !== latest.current || ms === null) return;
          setTyped((t) => t && { text: next, value: ms });
          return onChange(ms);
        },
        () => {},
      ),
    );
  };
  return <input id={id} aria-label={label} className="input time-input" value={text} onChange={(e) => type(e.target.value)} onBlur={() => setTyped(null)} onKeyDown={onKeyDown} />;
}
