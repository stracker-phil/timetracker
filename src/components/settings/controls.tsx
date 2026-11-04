/** Shared controls for settings pages: grouped cards and rows that apply their change right away. */
import { createContext, useContext, useEffect, useId, useRef, type ReactNode } from "react";
import { SegmentedControl, type SegmentedControlProps } from "../SegmentedControl";

/** The section the window was opened at (`open_settings`); its group scrolls to the top. */
export const SectionTarget = createContext<string | undefined>(undefined);

/** A row group with its heading. `section`: the id this group is registered under in the page's `sections`; the
 *  window scrolls to it. */
export function Group({ title, section, children }: { title?: string; section?: string; children: ReactNode }) {
  const target = useContext(SectionTarget);
  const ref = useRef<HTMLElement>(null);
  useEffect(() => {
    if (section && section === target) ref.current?.scrollIntoView?.({ block: "start" });
  }, [section, target]);
  return (
    <section ref={ref} className="settings-group" aria-label={title}>
      {title && <h3>{title}</h3>}
      <div className="settings-card">{children}</div>
    </section>
  );
}

interface RowProps {
  label: string;
  icon?: ReactNode;
  description?: ReactNode;
  htmlFor?: string;
  /** The control goes below the label instead of beside it. */
  stacked?: boolean;
  /** Indented under the row above it, the switch it depends on. */
  nested?: boolean;
  children?: ReactNode;
}

/** One card row: label and description on the left, the control on the right. */
export function Row({ label, icon, description, htmlFor, stacked, nested, children }: RowProps) {
  const className = ["settings-row", stacked && "stacked", nested && "nested"].filter(Boolean).join(" ");
  return (
    <div className={className}>
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

export function Switch({ label, description, checked, onChange }: { label: string; description?: ReactNode; checked: boolean; onChange: (v: boolean) => void }) {
  const id = useId();
  return (
    <Row label={label} description={description} htmlFor={id}>
      <input id={id} className="switch" type="checkbox" role="switch" checked={checked} onChange={(e) => onChange(e.target.checked)} />
    </Row>
  );
}

export function Segmented<T extends string>({ label, description, ...buttons }: SegmentedControlProps<T> & { description?: ReactNode }) {
  return (
    <Row label={label} description={description}>
      <SegmentedControl label={label} {...buttons} />
    </Row>
  );
}
