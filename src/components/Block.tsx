import type { HTMLAttributes, ReactNode } from "react";

interface Props extends Omit<HTMLAttributes<HTMLDivElement>, "title"> {
  top: number;
  height: number;
  /** Horizontal place within the lane, e.g. `"50%"`; the full width when left out. */
  left?: string;
  width?: string;
  /** Leave out for a block too short for text. */
  title?: ReactNode;
  subtitle?: ReactNode;
  /** Bottom right, as on a `Card`, e.g. the duration. */
  trailing?: ReactNode;
  /** Extra content after the text, e.g. an action button. */
  children?: ReactNode;
}

/**
 * A block placed on the timeline by time: a soft filled surface with a strong title, a normal subtitle and a
 * trailing value (docs/ux.md, Components). Short blocks put the three on one line (`one-line`).
 */
export function Block({ top, height, left, width, title, subtitle, trailing, className, style, children, ...rest }: Props) {
  return (
    <div className={className ? `block ${className}` : "block"} style={{ top, height, left, width, ...style }} {...rest}>
      {title != null && (
        <>
          <span className="block-title">{title}</span>
          {subtitle != null && <span className="block-subtitle">{subtitle}</span>}
          {trailing != null && <span className="block-trailing">{trailing}</span>}
        </>
      )}
      {children}
    </div>
  );
}
