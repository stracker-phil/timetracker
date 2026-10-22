import { useEffect, useState } from "react";
import { api, type Span } from "../../api";

/** Spans settle this long (ms) before Rust is asked again, so typing a time or dragging asks once. */
const SETTLE_MS = 150;

/** The project Rust proposes for the draft's spans, if any. */
export function useProposal(spans: Span[] | null): string | null {
  const [projectId, setProjectId] = useState<string | null>(null);

  useEffect(() => {
    if (!spans) return;
    let alive = true;
    const timer = setTimeout(() => {
      api.recordingProposal(spans).then(
        (found) => alive && setProjectId(found.projectId),
        () => {},
      );
    }, SETTLE_MS);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [spans]);

  return projectId;
}
