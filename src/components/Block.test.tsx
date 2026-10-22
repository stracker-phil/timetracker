import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Block } from "./Block";

describe("Block", () => {
  it("shows the title, subtitle and trailing value",() => {
    render(<Block top={0} height={60} title="Safari" subtitle="Docs" trailing="0:30" />);
    for (const text of ["Safari", "Docs", "0:30"]) expect(screen.getByText(text)).toBeInTheDocument();
  });

  it("shows no text without a title, even when subtitle and trailing are given", () => {
    render(<Block top={0} height={6} subtitle="Docs" trailing="0:30" aria-label="Safari" />);
    const block = screen.getByLabelText("Safari");
    expect(block).toBeEmptyDOMElement();
  });

  it("leaves out the subtitle when none is given", () => {
    render(<Block top={0} height={60} title="Safari" trailing="0:30" aria-label="Safari block" />);
    expect(screen.getByLabelText("Safari block")).toHaveTextContent("Safari0:30");
  });

  it("renders children even for a block without text", () => {
    render(
      <Block top={0} height={6}>
        <button type="button">Actions</button>
      </Block>,
    );
    expect(screen.getByRole("button", { name: "Actions" })).toBeInTheDocument();
  });

  it("places itself by top, height, left and width", () => {
    render(<Block top={20} height={60} left="50%" width="50%" title="Safari" aria-label="Safari block" />);
    expect(screen.getByLabelText("Safari block")).toHaveStyle({ top: "20px", height: "60px", left: "50%", width: "50%" });
  });

  it("passes props such as aria-label and event handlers through", async () => {
    const onClick = vi.fn();
    render(<Block top={0} height={60} title="Safari" aria-label="Safari: Docs" onClick={onClick} />);
    await userEvent.click(screen.getByLabelText("Safari: Docs"));
    expect(onClick).toHaveBeenCalled();
  });
});
