import type { ReactNode } from "react";

interface Props {
  title: ReactNode;
  /** Secondary text beside the title, e.g. the period's totals. */
  detail?: ReactNode;
  /** The view's actions, at the trailing end. */
  children?: ReactNode;
}

/** One row beside the window buttons, rendered by every view; its empty space drags the window. */
export function Toolbar({ title, detail, children }: Props) {
  return (
    <header className="toolbar" data-tauri-drag-region>
      <h1 className="toolbar-title" data-tauri-drag-region>
        {title}
      </h1>
      {detail}
      <span className="spacer" data-tauri-drag-region />
      {children}
    </header>
  );
}
