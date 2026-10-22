import type { CSSProperties, HTMLAttributes, ReactNode, Ref } from "react";

interface Props extends Omit<HTMLAttributes<HTMLDivElement>, "title"> {
  ref?: Ref<HTMLDivElement>;
  /** The project's shade (`shadeColor`); the fill and text derive from it (`theme.css`). */
  color: string;
  title: ReactNode;
  subtitle?: ReactNode;
  /** The foot's left side, e.g. the tag. */
  meta?: ReactNode;
  /** The foot's right side, e.g. the duration. */
  trailing?: ReactNode;
  /** Extra content, e.g. resize grips. */
  children?: ReactNode;
}

/**
 * A recording as a card filled in its project's color: the title, a subtitle, then a foot with meta and a
 * trailing value (docs/ux.md, Components).
 */
export function Card({ ref, color, title, subtitle, meta, trailing, className, style, children, ...rest }: Props) {
  return (
    <div ref={ref} className={className ? `card ${className}` : "card"} style={{ ...style, "--rec": color } as CSSProperties} {...rest}>
      <span className="card-title">{title}</span>
      {subtitle != null && <span className="card-subtitle">{subtitle}</span>}
      {(meta != null || trailing != null) && (
        <span className="card-foot">
          <span className="card-meta">{meta}</span>
          {trailing != null && <span className="card-trailing">{trailing}</span>}
        </span>
      )}
      {children}
    </div>
  );
}
