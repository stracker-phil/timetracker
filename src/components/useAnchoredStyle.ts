import { useLayoutEffect, useState, type CSSProperties } from "react";

const GAP = 6;
const EDGE = 8;

/** A popover's place under the element matching `selector`, aligned to its left or right edge; centered when it isn't there. */
export function useAnchoredStyle(selector: string, align: "left" | "right"): CSSProperties | undefined {
  const [style, setStyle] = useState<CSSProperties>();
  useLayoutEffect(() => {
    const rect = document.querySelector(selector)?.getBoundingClientRect();
    if (!rect) return;
    const top = rect.bottom + GAP;
    setStyle(align === "left" ? { position: "fixed", top, left: rect.left } : { position: "fixed", top, right: Math.max(EDGE, window.innerWidth - rect.right) });
  }, [selector, align]);
  return style;
}
