import type { ReactNode } from "react";

/** The title and totals of the Week and Month views; the sidebar changes the week or month. */
export function CalendarHeader({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <header className="day-head">
      <h1 className="day-title">{title}</h1>
      {children}
    </header>
  );
}
