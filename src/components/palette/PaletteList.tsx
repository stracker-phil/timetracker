import { Fragment, useEffect, type RefObject } from "react";
import { CornerDownLeft } from "lucide-react";
import { AccentDot } from "../Accent";
import { KeyCap } from "../Field";
import type { Item } from "./usePaletteItems";

interface Props {
  items: Item[];
  active: number;
  activeRef: RefObject<HTMLLIElement | null>;
  onHover: (i: number) => void;
  onChoose: (i: number, alt: boolean) => void;
}

/** The results under a label per section; the active row carries the accent bar and ↵. */
export function PaletteList({ items, active, activeRef, onHover, onChoose }: Props) {
  useEffect(() => {
    activeRef.current?.scrollIntoView?.({ block: "nearest" });
  }, [active, items, activeRef]);

  return (
    <ul className="palette-list" role="listbox" aria-label="Results">
      {items.map((it, i) => {
        const header = it.section !== items[i - 1]?.section ? it.section : null;
        return (
          <Fragment key={it.key}>
            {header && (
              <li role="presentation" className="palette-section">
                {header}
              </li>
            )}
            <li
              ref={i === active ? activeRef : undefined}
              role="option"
              aria-selected={i === active}
              className={i === active ? "active" : ""}
              onMouseEnter={() => onHover(i)}
              onClick={(e) => onChoose(i, e.metaKey)}
            >
              {it.accent && <AccentDot accent={it.accent} shade={it.shade} />}
              <span className="grow-text">
                {it.title}
                {it.subtitle && <span className="muted palette-subtitle">{it.subtitle}</span>}
              </span>
              <KeyCap keys={it.keys ?? ""} />
              {i === active && <CornerDownLeft size="0.923rem" className="muted" />}
            </li>
          </Fragment>
        );
      })}
      {items.length === 0 && <li className="muted">No matches.</li>}
    </ul>
  );
}

const PREFIX_HINTS = "> commands · p: projects · c: clients · @ dates";

/** The keys of the active row; the prefixes while nothing is typed. */
export function PaletteFooter({ item, empty }: { item: Item | undefined; empty: boolean }) {
  const keys = ["↵ open", item?.goAlt && `⌘↵ ${item.altLabel}`, "esc close"].filter(Boolean).join(" · ");
  return (
    <div className="palette-foot o3">
      <span>{keys}</span>
      {empty && <span>{PREFIX_HINTS}</span>}
    </div>
  );
}
