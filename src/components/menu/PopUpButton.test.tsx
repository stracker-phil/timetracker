import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentProps } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { resetStore } from "../../state/store";
import { MenuHost, PopUpButton } from "./index";

const options = [
  { value: "blue", label: "Blue" },
  { value: "green", label: "Green" },
  { value: "red", label: "Red" },
];

function setup(props: Partial<ComponentProps<typeof PopUpButton<string>>> = {}) {
  const onChange = vi.fn();
  render(
    <>
      <PopUpButton value="blue" options={options} onChange={onChange} {...props} />
      <MenuHost />
    </>,
  );
  return onChange;
}

describe("PopUpButton", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  it("announces a menu and shows the current choice", () => {
    setup();
    const button = screen.getByRole("button", { name: "Blue" });
    expect(button).toHaveAttribute("aria-haspopup", "menu");
    expect(button).toHaveAttribute("aria-expanded", "false");
  });

  it("is named by its aria-label and names its menu the same", async () => {
    setup({ "aria-label": "Accent" });
    await userEvent.click(screen.getByRole("button", { name: "Accent" }));
    expect(screen.getByRole("menu", { name: "Accent" })).toBeInTheDocument();
  });

  it("is named by a label that points at it and names its menu the same", async () => {
    render(
      <>
        <label htmlFor="accent">Accent color</label>
        <PopUpButton id="accent" value="blue" options={options} onChange={vi.fn()} />
        <MenuHost />
      </>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Accent color" }));
    expect(screen.getByRole("menu", { name: "Accent color" })).toBeInTheDocument();
  });

  it("opens its choices with the current one checked, and follows the menu with aria-expanded", async () => {
    setup();
    const button = screen.getByRole("button", { name: "Blue" });
    await userEvent.click(button);
    expect(button).toHaveAttribute("aria-expanded", "true");
    expect(screen.getAllByRole("menuitemradio").map((row) => row.textContent)).toEqual(["Blue", "Green", "Red"]);
    expect(screen.getByRole("menuitemradio", { name: "Blue" })).toHaveAttribute("aria-checked", "true");
    expect(screen.getByRole("menuitemradio", { name: "Green" })).toHaveAttribute("aria-checked", "false");
    await userEvent.keyboard("{Escape}");
    expect(button).toHaveAttribute("aria-expanded", "false");
  });

  it("closes its menu when pressed again", async () => {
    setup();
    const button = screen.getByRole("button", { name: "Blue" });
    await userEvent.click(button);
    await userEvent.click(button);
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("reports another choice through onChange and closes", async () => {
    const onChange = setup();
    await userEvent.click(screen.getByRole("button", { name: "Blue" }));
    await userEvent.click(screen.getByRole("menuitemradio", { name: "Green" }));
    expect(onChange).toHaveBeenCalledWith("green");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("doesn't report the choice it already has", async () => {
    const onChange = setup();
    await userEvent.click(screen.getByRole("button", { name: "Blue" }));
    await userEvent.click(screen.getByRole("menuitemradio", { name: "Blue" }));
    expect(onChange).not.toHaveBeenCalled();
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("works with numeric values", async () => {
    const onChange = vi.fn();
    render(
      <>
        <PopUpButton
          value={15}
          options={[
            { value: 5, label: "5 min" },
            { value: 15, label: "15 min" },
          ]}
          onChange={onChange}
        />
        <MenuHost />
      </>,
    );
    await userEvent.click(screen.getByRole("button", { name: "15 min" }));
    await userEvent.click(screen.getByRole("menuitemradio", { name: "5 min" }));
    expect(onChange).toHaveBeenCalledWith(5);
  });

  it("doesn't open when disabled", async () => {
    setup({ disabled: true });
    const button = screen.getByRole("button", { name: "Blue" });
    expect(button).toBeDisabled();
    await userEvent.click(button);
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it.each(["{ArrowDown}", "{ArrowUp}"])("opens on %s", async (key) => {
    setup();
    screen.getByRole("button", { name: "Blue" }).focus();
    await userEvent.keyboard(key);
    expect(screen.getByRole("menu")).toBeInTheDocument();
    expect(screen.getByRole("menuitemradio", { name: "Blue" })).toHaveFocus();
  });

  it("returns the focus to the button when the menu closes", async () => {
    setup();
    const button = screen.getByRole("button", { name: "Blue" });
    await userEvent.click(button);
    await userEvent.keyboard("{Escape}");
    expect(button).toHaveFocus();
  });
});
