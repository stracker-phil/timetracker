import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Toolbar } from "./Toolbar";

describe("Toolbar", () => {
  it("shows the title as the level-1 heading", () => {
    render(<Toolbar title="Projects" />);
    expect(screen.getByRole("heading", { level: 1, name: "Projects" })).toBeInTheDocument();
  });

  it("shows the detail beside the title", () => {
    render(<Toolbar title="Week" detail={<span aria-label="Totals">12h 30m</span>} />);
    expect(screen.getByLabelText("Totals")).toHaveTextContent("12h 30m");
  });

  it("renders the actions", () => {
    render(
      <Toolbar title="Rules">
        <button type="button">New rule</button>
      </Toolbar>,
    );
    expect(screen.getByRole("button", { name: "New rule" })).toBeInTheDocument();
  });

  it("renders nothing but the title without detail and actions", () => {
    render(<Toolbar title="Reports" />);
    expect(screen.queryByLabelText("Totals")).not.toBeInTheDocument();
    expect(screen.queryAllByRole("button")).toHaveLength(0);
  });
});
