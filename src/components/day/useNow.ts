import { useEffect, useState } from "react";

/** The current time, renewed every `tickMs` while `live`. */
export function useNow(live: boolean, tickMs: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!live) return;
    const timer = setInterval(() => setNow(Date.now()), tickMs);
    return () => clearInterval(timer);
  }, [live, tickMs]);
  return now;
}
