import { Check, ChevronRight } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";
import type { MenuEntry, MenuItem } from "../../state/slices/menu";
import { isEnabled, isItem, nextEnabled, rowStartingWith } from "./keys";
import { place, type Placement } from "./placement";

interface Props {
  entries: MenuEntry[];
  label: string;
  placement: Placement;
  /** Opened from a button or a key: the first row starts highlighted. */
  highlightFirst: boolean;
  /** Closes every panel, then runs the row. */
  onChoose: (item: MenuItem) => void;
  /** A submenu's ←: back to its parent row. */
  onBack?: () => void;
}

interface Section {
  caption?: string;
  /** Each entry with its index in `entries`. */
  entries: [MenuEntry, number][];
}

interface Submenu {
  index: number;
  items: MenuEntry[];
  label: string;
  placement: Placement;
  focused: boolean;
}

/** A caption starts a captioned section, which runs to the next caption or separator. */
function sectionsOf(entries: MenuEntry[]): Section[] {
  const sections: Section[] = [{ entries: [] }];
  entries.forEach((entry, index) => {
    const last = sections[sections.length - 1];
    if (entry !== "separator" && "caption" in entry) sections.push({ caption: entry.caption, entries: [] });
    else if (entry === "separator" && last.caption !== undefined) sections.push({ entries: [[entry, index]] });
    else last.entries.push([entry, index]);
  });
  return sections.filter((section) => section.caption !== undefined || section.entries.length > 0);
}

/** One menu panel: its rows, the keys that move through them, and the submenu of the highlighted row. */
export function MenuPanel({ entries, label, placement, highlightFirst, onChoose, onBack }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const enabled = entries.flatMap((entry, index) => (isEnabled(entry) ? [index] : []));
  const [active, setActive] = useState(highlightFirst ? (enabled[0] ?? -1) : -1);
  const [submenu, setSubmenu] = useState<Submenu | null>(null);
  const [position, setPosition] = useState<CSSProperties>({ left: 0, top: 0, visibility: "hidden" });
  // Leaves a check column only when a row has a check, so the texts of a menu line up.
  const checks = entries.some((entry) => isItem(entry) && entry.checked !== undefined);

  useLayoutEffect(() => {
    const panel = ref.current;
    if (!panel) return;
    const size = { width: panel.offsetWidth, height: panel.offsetHeight };
    setPosition(place(placement, size, { width: window.innerWidth, height: window.innerHeight }));
  }, [placement]);

  const rowOf = (index: number) => (index >= 0 ? ref.current?.querySelector<HTMLElement>(`[data-index="${index}"]`) : null);

  // The focus follows the highlight, unless a submenu opened from the keyboard holds it. The panel is hidden until
  // it is placed, and a hidden element takes no focus, so it focuses again once placed.
  useEffect(() => {
    if (submenu?.focused) return;
    (rowOf(active) ?? ref.current)?.focus({ preventScroll: true });
  }, [active, submenu, position]);

  const openSubmenu = (index: number, focused: boolean) => {
    const entry = entries[index];
    const row = rowOf(index);
    if (!row || !isItem(entry) || !entry.items) return;
    const { left, top, right, bottom } = row.getBoundingClientRect();
    setSubmenu({ index, items: entry.items, label: entry.text, placement: { beside: { left, top, right, bottom } }, focused });
  };

  const choose = (index: number) => {
    const entry = entries[index];
    if (!isEnabled(entry)) return;
    if (entry.items) openSubmenu(index, true);
    else onChoose(entry);
  };

  const hover = (index: number) => {
    const entry = entries[index];
    if (!isEnabled(entry)) return;
    setActive(index);
    if (entry.items) openSubmenu(index, false);
    else setSubmenu(null);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    switch (e.key) {
      case "ArrowDown":
        setActive(nextEnabled(enabled, active, 1));
        break;
      case "ArrowUp":
        setActive(nextEnabled(enabled, active, -1));
        break;
      case "Home":
        if (enabled.length) setActive(enabled[0]);
        break;
      case "End":
        if (enabled.length) setActive(enabled[enabled.length - 1]);
        break;
      case "Enter":
      case " ":
        if (active >= 0) choose(active);
        break;
      case "ArrowRight": {
        const entry = entries[active];
        if (entry && isItem(entry) && entry.items) openSubmenu(active, true);
        break;
      }
      case "ArrowLeft":
        onBack?.();
        break;
      case "Tab":
        break;
      default: {
        if (e.key.length !== 1 || e.metaKey || e.ctrlKey || e.altKey) return;
        const match = rowStartingWith(entries, enabled, active, e.key);
        if (match !== undefined) setActive(match);
      }
    }
    e.preventDefault();
    e.stopPropagation();
  };

  return (
    <>
      <div
        ref={ref}
        className={checks ? "menu menu--checks" : "menu"}
        role="menu"
        aria-label={label}
        tabIndex={-1}
        style={position}
        onKeyDown={onKeyDown}
        onMouseLeave={() => {
          if (!submenu) setActive(-1);
        }}
      >
        {sectionsOf(entries).map((section, at) => (
          <MenuSection key={at} section={section} active={active} openIndex={submenu?.index ?? null} onHover={hover} onChoose={choose} />
        ))}
      </div>
      {submenu && (
        <MenuPanel
          key={submenu.index}
          entries={submenu.items}
          label={submenu.label}
          placement={submenu.placement}
          highlightFirst={submenu.focused}
          onChoose={onChoose}
          onBack={() => setSubmenu(null)}
        />
      )}
    </>
  );
}

interface SectionProps {
  section: Section;
  active: number;
  openIndex: number | null;
  onHover: (index: number) => void;
  onChoose: (index: number) => void;
}

function MenuSection({ section, active, openIndex, onHover, onChoose }: SectionProps) {
  const rows = section.entries.map(([entry, index]) =>
    isItem(entry) ? (
      <MenuRow key={index} item={entry} index={index} active={index === active} open={index === openIndex} onHover={onHover} onChoose={onChoose} />
    ) : (
      <div key={index} className="menu__separator" role="separator" />
    ),
  );
  if (section.caption === undefined) return <>{rows}</>;
  return (
    <div className="menu__group" role="group" aria-label={section.caption}>
      <span className="menu__caption" aria-hidden="true">
        {section.caption}
      </span>
      {rows}
    </div>
  );
}

interface RowProps {
  item: MenuItem;
  index: number;
  active: boolean;
  open: boolean;
  onHover: (index: number) => void;
  onChoose: (index: number) => void;
}

function MenuRow({ item, index, active, open, onHover, onChoose }: RowProps) {
  const choice = item.checked !== undefined;
  const role = !choice ? "menuitem" : item.toggle ? "menuitemcheckbox" : "menuitemradio";
  const disabled = item.enabled === false;
  const classes = ["menu__item", item.danger && "menu__item--danger", item.detail && "menu__item--detail"].filter(Boolean).join(" ");
  return (
    <button
      type="button"
      role={role}
      tabIndex={-1}
      data-index={index}
      className={classes}
      data-active={active || undefined}
      title={item.title}
      aria-checked={choice ? item.checked : undefined}
      aria-disabled={disabled || undefined}
      aria-haspopup={item.items ? "menu" : undefined}
      aria-expanded={item.items ? open : undefined}
      onMouseEnter={() => onHover(index)}
      onClick={() => onChoose(index)}
    >
      <Check className="menu__check" size="0.923rem" strokeWidth={2.2} aria-hidden="true" visibility={item.checked ? "visible" : "hidden"} />
      {item.icon && <span className="menu__icon">{item.icon}</span>}
      <span className="menu__text">
        <span className="menu__label">{item.text}</span>
        {item.detail && <span className="menu__detail">{item.detail}</span>}
      </span>
      {item.hint && <span className="menu__hint">{item.hint}</span>}
      {item.items && <ChevronRight className="menu__submenu" size="0.923rem" strokeWidth={2} aria-hidden="true" />}
    </button>
  );
}
