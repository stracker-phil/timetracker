import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ACCENTS } from "../api";
import { AccentSwatches } from "./Accent";

const swatchNames = () => within(screen.getByRole("group", { name: "Accent" })).getAllByRole("button").map((b) => b.getAttribute("aria-label"));

describe("AccentSwatches", () => {
  it("offers one swatch per accent hue and no System swatch by default", () => {
    render(<AccentSwatches value="teal" onChange={() => {}} />);
    expect(swatchNames()).toEqual(ACCENTS.map((a) => `Accent ${a}`));
  });

  it("puts the System swatch first with withSystem", () => {
    render(<AccentSwatches value="teal" onChange={() => {}} withSystem />);
    expect(swatchNames()).toEqual(["Accent system", ...ACCENTS.map((a) => `Accent ${a}`)]);
  });

  it("presses the System swatch only while the value is system", () => {
    render(<AccentSwatches value="system" onChange={() => {}} withSystem />);
    expect(screen.getByRole("button", { name: "Accent system" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: `Accent ${ACCENTS[0]}` })).toHaveAttribute("aria-pressed", "false");
  });

  it("reports system when the System swatch is clicked", async () => {
    const onChange = vi.fn();
    render(<AccentSwatches value="teal" onChange={onChange} withSystem />);
    await userEvent.click(screen.getByRole("button", { name: "Accent system" }));
    expect(onChange).toHaveBeenCalledWith("system");
  });

  it("reports the hue of a clicked swatch", async () => {
    const onChange = vi.fn();
    render(<AccentSwatches value={null} onChange={onChange} />);
    await userEvent.click(screen.getByRole("button", { name: "Accent teal" }));
    expect(onChange).toHaveBeenCalledWith("teal");
  });
});
