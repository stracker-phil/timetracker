import { Minus, Plus } from "lucide-react";
import { TIMELINE_ZOOM, useStore } from "../../state/store";

/** The timeline's zoom, floating at its bottom right: −, a slider of px per minute, +. */
export function ZoomControl() {
  const zoom = useStore((s) => s.uiState.timelineZoom);
  const { stepTimelineZoom, setTimelineZoom } = useStore.getState();
  return (
    <div className="zoom-control" role="group" aria-label="Zoom">
      <button type="button" className="icon-btn" aria-label="Zoom out" disabled={zoom <= TIMELINE_ZOOM.min} onClick={() => stepTimelineZoom(-1)}>
        <Minus size="1rem" aria-hidden="true" />
      </button>
      <input
        type="range"
        className="zoom-slider"
        aria-label="Zoom level"
        min={TIMELINE_ZOOM.min}
        max={TIMELINE_ZOOM.max}
        step={0.5}
        value={zoom}
        onChange={(e) => setTimelineZoom(Number(e.target.value))}
      />
      <button type="button" className="icon-btn" aria-label="Zoom in" disabled={zoom >= TIMELINE_ZOOM.max} onClick={() => stepTimelineZoom(1)}>
        <Plus size="1rem" aria-hidden="true" />
      </button>
    </div>
  );
}
