/** The window's one tooltip (ADR 0052): it takes over every `title`, so macOS's small, slow tooltip never shows. */
import { useEffect, useLayoutEffect, useRef, useState } from "react";

/** How long the pointer rests before a tooltip shows. */
const SHOW_DELAY_MS = 450;
/** After one tooltip, the next shows at once for this long (moving along a toolbar). */
const WARM_MS = 600;
const GAP = 6;
/** The whole tooltip stays this far inside the window. */
const EDGE = 8;
/** Room a one-line tooltip needs below its control before it goes above instead. */
const ONE_LINE = 28;
/** How far up from the pointer cut-off text is looked for (the text sits in a few wrappers at most). */
const CUT_OFF_DEPTH = 4;

interface Tip {
  text: string;
  rect: DOMRect;
}

/** Moves an element's `title` to `data-tooltip`; an element named only by its title keeps it as `aria-label`. */
function takeTitle(element: Element) {
  const title = element.getAttribute("title");
  if (!title) return;
  element.removeAttribute("title");
  element.setAttribute("data-tooltip", title);
  if (!element.hasAttribute("aria-label") && !element.textContent?.trim()) element.setAttribute("aria-label", title);
}

/** Text that doesn't fit: cut with an ellipsis, or clamped to fewer lines. */
function cutOff(element: Element): boolean {
  if (!(element instanceof HTMLElement)) return false;
  const style = getComputedStyle(element);
  if (style.textOverflow === "ellipsis" && element.scrollWidth > element.clientWidth) return true;
  const clamp = style.getPropertyValue("-webkit-line-clamp");
  return clamp !== "" && clamp !== "none" && element.scrollHeight > element.clientHeight;
}

function cutOffAround(target: Element): Element | null {
  let element: Element | null = target;
  for (let depth = 0; element && depth < CUT_OFF_DEPTH; depth += 1, element = element.parentElement) {
    if (cutOff(element)) return element;
  }
  return null;
}

const titled = (element: Element) => element.closest("[title], [data-tooltip]");

/** The element whose tooltip shows: cut-off text shows itself whole, else the nearest title. */
const owner = (target: EventTarget | null) => (target instanceof Element ? (cutOffAround(target) ?? titled(target)) : null);

function tipText(element: Element): string | null {
  if (cutOff(element)) return element.textContent?.trim() || null;
  takeTitle(element);
  return element.getAttribute("data-tooltip");
}

const HIDE_EVENTS = ["pointerdown", "keydown", "focusout", "wheel"] as const;

/** One tooltip for the whole window: after a short rest, at once while moving between controls, and on keyboard
 *  focus. Any element with a `title` gets it, and any text cut off with an ellipsis shows in full. */
export function TooltipHost() {
  const [tip, setTip] = useState<Tip | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  // Measured before it paints, so it can keep clear of the window's edges.
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    if (ref.current) setWidth(ref.current.offsetWidth);
  }, [tip]);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    let current: Element | null = null;
    let hiddenAt = 0;
    const hide = () => {
      clearTimeout(timer);
      if (current) hiddenAt = Date.now();
      current = null;
      setTip(null);
    };
    const show = (element: Element, delay: number) => {
      const text = tipText(element);
      if (!text) return;
      clearTimeout(timer);
      current = element;
      const reveal = () => {
        if (current === element && element.isConnected) setTip({ text, rect: element.getBoundingClientRect() });
      };
      if (delay === 0) reveal();
      else timer = setTimeout(reveal, delay);
    };
    const warm = () => Date.now() - hiddenAt < WARM_MS;
    // Finding cut-off text reads styles and layout, so it waits until the pointer has rested.
    const onOver = (e: PointerEvent) => {
      const target = e.target;
      if (!(target instanceof Element)) return hide();
      const withTitle = target.closest("[title]");
      if (withTitle) takeTitle(withTitle);
      if (current?.contains(target) && titled(target) === titled(current)) return;
      const wasShowing = current !== null;
      hide();
      const reveal = () => {
        const element = owner(target);
        if (element && target.isConnected) show(element, 0);
      };
      if (wasShowing || warm()) reveal();
      else timer = setTimeout(reveal, SHOW_DELAY_MS);
    };
    const onFocus = (e: FocusEvent) => {
      const element = owner(e.target);
      if (element && e.target instanceof Element && e.target.matches(":focus-visible")) show(element, SHOW_DELAY_MS);
    };
    document.addEventListener("pointerover", onOver);
    document.addEventListener("focusin", onFocus);
    for (const event of HIDE_EVENTS) document.addEventListener(event, hide, true);
    window.addEventListener("blur", hide);
    return () => {
      clearTimeout(timer);
      document.removeEventListener("pointerover", onOver);
      document.removeEventListener("focusin", onFocus);
      for (const event of HIDE_EVENTS) document.removeEventListener(event, hide, true);
      window.removeEventListener("blur", hide);
    };
  }, []);

  if (!tip) return null;
  const below = tip.rect.bottom + GAP + ONE_LINE < window.innerHeight;
  const half = width / 2;
  const left = Math.min(Math.max(tip.rect.left + tip.rect.width / 2, EDGE + half), window.innerWidth - EDGE - half);
  const style = below ? { left, top: tip.rect.bottom + GAP } : { left, bottom: window.innerHeight - tip.rect.top + GAP };
  return (
    <div ref={ref} className={below ? "tooltip" : "tooltip tooltip--above"} role="tooltip" style={style}>
      {tip.text}
    </div>
  );
}
