/** Shared controls for settings pages: grouped cards and rows that apply their change right away. */
import { createContext, useContext, useEffect, useId, useRef, type ReactNode } from "react";
import { SegmentedControl, type SegmentedControlProps } from "../SegmentedControl";

/** The row Settings was opened at: a new object per request, so opening the same row again reveals it again. */
export interface RevealTarget {
  id: string;
  /** Revealed instead while the row isn't shown. */
  under?: string;
}

export const ScrollTarget = createContext<RevealTarget | undefined>(undefined);

/** Targets whose own row was revealed; a fallback (`under`) revealed first gives way to the row when it mounts. */
const revealed = new WeakSet<RevealTarget>();

function scrollAndWash(element: HTMLElement) {
  element.scrollIntoView?.({ block: "center" });
  element.classList.remove("found");
  void element.offsetWidth; // restarts the wash
  element.classList.add("found");
}

/** Scrolls to and washes the element when the request names its anchor. */
function useScrollTarget<E extends HTMLElement>(anchor: string | undefined) {
  const target = useContext(ScrollTarget);
  const ref = useRef<E>(null);
  useEffect(() => {
    const element = ref.current;
    if (!anchor || !target || !element || revealed.has(target)) return;
    if (anchor !== target.id && anchor !== target.under) return;
    if (anchor === target.id) revealed.add(target);
    else if (element.ownerDocument.querySelector(`[data-anchor="${CSS.escape(target.id)}"]`)) return;
    scrollAndWash(element);
  }, [anchor, target]);
  return ref;
}

/** A row group with its heading. `anchor`: the id the page registers for the group (a list of rows built from
 *  data), so the search can open it. */
export function Group({ title, anchor, children }: { title?: string; anchor?: string; children: ReactNode }) {
  const ref = useScrollTarget<HTMLElement>(anchor);
  return (
    <section ref={ref} className="settings-group" aria-label={title} data-anchor={anchor}>
      {title && <h3>{title}</h3>}
      <div className="settings-card">{children}</div>
    </section>
  );
}

interface RowProps {
  label: string;
  icon?: ReactNode;
  description?: ReactNode;
  /** The row id the page registers (`SettingsRow.id`). */
  anchor?: string;
  htmlFor?: string;
  /** The control goes below the label instead of beside it. */
  stacked?: boolean;
  /** Indented under the row above it, the switch it depends on. */
  nested?: boolean;
  children?: ReactNode;
}

/** One card row: label and description on the left, the control on the right. */
export function Row({ label, icon, description, anchor, htmlFor, stacked, nested, children }: RowProps) {
  const ref = useScrollTarget<HTMLDivElement>(anchor);
  const className = ["settings-row", stacked && "stacked", nested && "nested"].filter(Boolean).join(" ");
  return (
    <div ref={ref} className={className} data-anchor={anchor}>
      <div className="settings-row-text">
        {htmlFor ? (
          <label htmlFor={htmlFor}>{label}</label>
        ) : (
          <div className={icon ? "settings-row-label with-icon" : "settings-row-label"}>
            {icon ? (
              <>
                {icon}
                <span>{label}</span>
              </>
            ) : (
              label
            )}
          </div>
        )}
        {description && <div className="settings-row-desc">{description}</div>}
      </div>
      <div className="settings-row-control">{children}</div>
    </div>
  );
}

export function Switch({ label, description, anchor, checked, onChange }: { label: string; description?: ReactNode; anchor?: string; checked: boolean; onChange: (v: boolean) => void }) {
  const id = useId();
  return (
    <Row label={label} description={description} anchor={anchor} htmlFor={id}>
      <input id={id} className="switch" type="checkbox" role="switch" checked={checked} onChange={(e) => onChange(e.target.checked)} />
    </Row>
  );
}

export function Segmented<T extends string>({ label, description, anchor, ...buttons }: SegmentedControlProps<T> & { description?: ReactNode; anchor?: string }) {
  return (
    <Row label={label} description={description} anchor={anchor}>
      <SegmentedControl label={label} {...buttons} />
    </Row>
  );
}
