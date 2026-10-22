import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Card } from "./Card";

describe("Card", () => {
  it("shows the title, subtitle, meta and trailing value", () => {
    render(<Card color="#336699" title="Website" subtitle="standup" meta="support" trailing="0:45" />);
    for (const text of ["Website", "standup", "support", "0:45"]) expect(screen.getByText(text)).toBeInTheDocument();
  });

  it("shows only the title when nothing else is given", () => {
    render(<Card color="#336699" title="Website" aria-label="Card" />);
    expect(screen.getByLabelText("Card")).toHaveTextContent(/^Website$/);
  });

  it("shows the trailing value without meta", () => {
    render(<Card color="#336699" title="Website" trailing="0:45" aria-label="Card" />);
    expect(screen.getByLabelText("Card")).toHaveTextContent("Website0:45");
  });

  it("renders its children after the text", () => {
    render(
      <Card color="#336699" title="Website">
        <span aria-label="Drag to change the end" />
      </Card>,
    );
    expect(screen.getByLabelText("Drag to change the end")).toBeInTheDocument();
  });

  it("passes its color on as the --rec variable and keeps the given style", () => {
    render(<Card color="#336699" title="Website" aria-label="Card" style={{ top: 10 }} />);
    const card = screen.getByLabelText("Card");
    expect(card.style.getPropertyValue("--rec")).toBe("#336699");
    expect(card).toHaveStyle({ top: "10px" });
  });

  it("passes props such as aria-label and event handlers through", async () => {
    const onDoubleClick = vi.fn();
    render(<Card color="#336699" title="Website" aria-label="Website: standup" onDoubleClick={onDoubleClick} />);
    await userEvent.dblClick(screen.getByLabelText("Website: standup"));
    expect(onDoubleClick).toHaveBeenCalled();
  });
});
