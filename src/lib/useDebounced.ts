import { useEffect, useState } from "react";

/** `value` once it has stayed the same for `ms`; compared by its JSON, so an equal object counts as unchanged. */
export function useDebounced<T>(value: T, ms: number): T {
  const [settled, setSettled] = useState(value);
  const key = JSON.stringify(value);
  useEffect(() => {
    const timer = setTimeout(() => setSettled(value), ms);
    return () => clearTimeout(timer);
    // `key` stands for `value`: a new object with the same content restarts nothing.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, ms]);
  return settled;
}
