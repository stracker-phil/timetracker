import { useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { api, type ActivityBlock } from "../../api";
import { formatClock, formatDuration } from "../../lib/format";
import { fail } from "../../state/store";

const GAP = 8;
const EDGE = 4;

interface Props {
  block: ActivityBlock;
  /** The block's element. */
  anchor: Element | null;
  onPointerEnter?: () => void;
  onPointerLeave?: () => void;
}

const isWebUrl = (url: string) => /^https?:\/\//i.test(url);

/** Beside the block when there is room, else below it, or above it near the window's bottom. */
function placeCard(target: DOMRect, width: number, height: number): { left: number; top: number } {
  const maxLeft = window.innerWidth - width - EDGE;
  const clampTop = (top: number) => Math.max(EDGE, Math.min(top, window.innerHeight - height - EDGE));
  if (target.right + GAP <= maxLeft) return { left: target.right + GAP, top: clampTop(target.top) };
  if (target.left - GAP - width >= EDGE) return { left: target.left - GAP - width, top: clampTop(target.top) };
  const below = target.bottom + GAP;
  const top = below + height <= window.innerHeight - EDGE ? below : target.top - GAP - height;
  return { left: Math.max(EDGE, Math.min(target.left + GAP, maxLeft)), top: clampTop(top) };
}

/** A timeline block's details. */
export function HoverCard({ block, anchor, onPointerEnter, onPointerLeave }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);

  useLayoutEffect(() => {
    const el = ref.current;
    const target = anchor?.getBoundingClientRect();
    if (!el || !target) return;
    const { width, height } = el.getBoundingClientRect();
    setPos(placeCard(target, width, height));
  }, [anchor, block]);

  const { url } = block;
  const source = block.source.replace(/^external:/, "");
  return createPortal(
    <div
      ref={ref}
      className="hover-card"
      role="dialog"
      aria-label={`${block.app} details`}
      style={pos ?? { visibility: "hidden" }}
      onPointerEnter={onPointerEnter}
      onPointerLeave={onPointerLeave}
    >
      <strong>{block.app}</strong>
      <p className={block.private ? "mono o3" : "mono"}>{block.title}</p>
      {url &&
        (isWebUrl(url) ? (
          <button type="button" className="link-btn mono hover-card-url" onClick={() => void api.openUrl(url).catch((e: unknown) => fail(e, "Could not open the URL"))}>
            {url}
          </button>
        ) : (
          <p className="mono hover-card-url">{url}</p>
        ))}
      {block.path && <p className="mono hover-card-url">{block.path}</p>}
      <p className="mono o2">
        {formatClock(block.start)}–{formatClock(block.end)} · {formatDuration(block.end - block.start)}
        {source !== "tracker" && ` · ${source}`}
      </p>
    </div>,
    document.body,
  );
}
