import { useEffect, useState } from "react";
import { api, type RecordingProposal, type Span } from "../../api";

/** Spans settle this long (ms) before Rust is asked again, so typing a time or dragging asks once. */
const SETTLE_MS = 150;

/** Rust's proposal for the draft's spans: the activity they cover and a project. */
export function useProposal(spans: Span[] | null): RecordingProposal | null {
  const [proposal, setProposal] = useState<RecordingProposal | null>(null);

  useEffect(() => {
    if (!spans) return;
    let alive = true;
    const timer = setTimeout(() => {
      api.recordingProposal(spans).then(
        (found) => alive && setProposal(found),
        () => {},
      );
    }, SETTLE_MS);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [spans]);

  return proposal;
}
