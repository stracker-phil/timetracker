import { useEffect, useState } from "react";
import { api, type RecordingAutofill } from "../../api";

/** Typing settles this long (ms) before Rust is asked, so a note asks once. */
const SETTLE_MS = 250;

/** The project and tag of an earlier recording with this note, if any. */
export function useAutofill(note: string): RecordingAutofill | null {
  const [found, setFound] = useState<RecordingAutofill | null>(null);

  useEffect(() => {
    let alive = true;
    const timer = setTimeout(() => {
      api.recordingAutofill(note).then(
        (result) => alive && setFound(result),
        () => {},
      );
    }, SETTLE_MS);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [note]);

  return found;
}
