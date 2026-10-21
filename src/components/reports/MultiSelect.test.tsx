import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { MultiSelect } from "./MultiSelect";

const ITEMS = [
  { id: "a", label: "Alpha" },
  { id: "b", label: "Beta" },
  { id: "c", label: "Gamma" },
];

function Harness({ initial = [], onChange }: { initial?: string[]; onChange?: (ids: string[]) => void }) {
  const [value, setValue] = useState(initial);
  return (
    <MultiSelect
      label="Projects"
      icon={null}
      items={ITEMS}
      value={value}
      onChange={(ids) => {
        setValue(ids);
        onChange?.(ids);
      }}
    />
  );
}

describe("MultiSelect", () => {
  it("shows an All placeholder while nothing is chosen", () => {
    render(<Harness />);
    expect(screen.getByRole("combobox", { name: "Projects" })).toHaveAttribute("placeholder", "All projects");
  });

  it("lists the items when focused and adds a clicked one", async () => {
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);
    await userEvent.click(screen.getByRole("combobox", { name: "Projects" }));
    await userEvent.click(screen.getByRole("option", { name: "Beta" }));
    expect(onChange).toHaveBeenLastCalledWith(["b"]);
    expect(screen.getByText("Beta")).toBeInTheDocument();
  });

  it("filters by what is typed and adds the first match with Enter", async () => {
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);
    await userEvent.type(screen.getByRole("combobox", { name: "Projects" }), "gam");
    expect(screen.queryByRole("option", { name: "Alpha" })).not.toBeInTheDocument();
    await userEvent.keyboard("{Enter}");
    expect(onChange).toHaveBeenLastCalledWith(["c"]);
    expect(screen.getByRole("combobox", { name: "Projects" })).toHaveValue("");
  });

  it("shows the chosen items joined and offers only the others", async () => {
    render(<Harness initial={["a", "b"]} />);
    expect(screen.getByText("Alpha · Beta")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("combobox", { name: "Projects" }));
    expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["Gamma"]);
  });

  it("removes the last chosen item with Backspace on an empty query", async () => {
    const onChange = vi.fn();
    render(<Harness initial={["a", "b"]} onChange={onChange} />);
    await userEvent.click(screen.getByRole("combobox", { name: "Projects" }));
    await userEvent.keyboard("{Backspace}");
    expect(onChange).toHaveBeenLastCalledWith(["a"]);
    expect(screen.getByText("Alpha")).toBeInTheDocument();
  });

  it("keeps the chosen items when Backspace edits a typed query", async () => {
    const onChange = vi.fn();
    render(<Harness initial={["a"]} onChange={onChange} />);
    await userEvent.type(screen.getByRole("combobox", { name: "Projects" }), "x{Backspace}");
    expect(onChange).not.toHaveBeenCalled();
  });
});
