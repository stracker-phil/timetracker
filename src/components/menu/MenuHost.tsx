import { useEffect, useRef, useState, type RefObject } from "react";
import { createPortal } from "react-dom";
import type { MenuItem, OpenMenu } from "../../state/slices/menu";
import { useStore } from "../../state/store";
import { MenuPanel } from "./MenuPanel";
import type { Placement } from "./placement";

/** Shows the menu `openMenu` asked for; mounted once per window that has menus. */
export function MenuHost() {
  const menu = useStore((s) => s.menu);
  const close = useStore((s) => s.closeMenu);
  return menu ? <OpenMenuLayer key={menu.id} menu={menu} onClose={close} /> : null;
}

function placementOf(at: OpenMenu["at"]): Placement {
  if (!(at instanceof HTMLElement)) return { point: at };
  const { left, top, right, bottom } = at.getBoundingClientRect();
  return { below: { left, top, right, bottom } };
}

/** Puts the focus back on the menu's opener, if it is still on the page. */
function returnFocus(opener: HTMLElement | null) {
  if (opener?.isConnected) opener.focus({ preventScroll: true });
}

/** Closes the menu on Escape, a press outside it, a scroll, a resize or the window losing focus; once closed, the
 *  focus goes back to the opener unless a row moved it on (a field, a sheet). */
function useDismiss(layer: RefObject<HTMLDivElement | null>, menu: OpenMenu, onClose: () => void) {
  useEffect(() => {
    const inside = (target: EventTarget | null) => target instanceof Node && !!layer.current?.contains(target);
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      onClose();
    };
    // A press on the menu's own button is left to the button, which closes it.
    const onPress = (e: MouseEvent) => {
      const onAnchor = menu.at instanceof HTMLElement && e.target instanceof Node && menu.at.contains(e.target);
      if (!inside(e.target) && !onAnchor) onClose();
    };
    const onScroll = (e: Event) => {
      if (!inside(e.target)) onClose();
    };
    window.addEventListener("keydown", onKey, true);
    document.addEventListener("mousedown", onPress, true);
    document.addEventListener("scroll", onScroll, true);
    window.addEventListener("blur", onClose);
    window.addEventListener("resize", onClose);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      document.removeEventListener("mousedown", onPress, true);
      document.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("blur", onClose);
      window.removeEventListener("resize", onClose);
      const focused = document.activeElement;
      if (!focused || focused === document.body || inside(focused)) returnFocus(menu.opener);
    };
  }, [layer, menu, onClose]);
}

/** The open menu and its submenus, portaled to `<body>` so no area restyles them. */
function OpenMenuLayer({ menu, onClose }: { menu: OpenMenu; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [placement] = useState(() => placementOf(menu.at));
  // Opened from a button or a key (lib/inputModality.ts), it starts on its first row.
  const [highlightFirst] = useState(() => menu.at instanceof HTMLElement || document.documentElement.dataset.input === "keyboard");
  useDismiss(ref, menu, onClose);

  // The focus goes back first, so a sheet the row opens returns it there too.
  const choose = (item: MenuItem) => {
    returnFocus(menu.opener);
    onClose();
    item.run?.();
  };

  return createPortal(
    <div ref={ref} className="menu-layer" onContextMenu={(e) => e.preventDefault()}>
      <MenuPanel entries={menu.entries} label={menu.label} placement={placement} highlightFirst={highlightFirst} onChoose={choose} />
    </div>,
    document.body,
  );
}
