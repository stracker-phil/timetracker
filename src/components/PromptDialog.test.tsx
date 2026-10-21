import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { resetStore, useStore } from "../state/store";
import { PromptDialog } from "./PromptDialog";

function renderPrompt(initial = "") {
  const onSubmit = vi.fn();
  useStore.setState({ dialog: { type: "prompt", title: "Rename report", label: "Name", initial, confirmLabel: "Rename", onSubmit } });
  render(<PromptDialog title="Rename report" label="Name" initial={initial} confirmLabel="Rename" onSubmit={onSubmit} />);
  return onSubmit;
}

describe("PromptDialog", () => {
  beforeEach(() => resetStore());

  it("shows the title and the initial value", () => {
    renderPrompt("Weekly");
    expect(screen.getByRole("dialog", { name: "Rename report" })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveValue("Weekly");
  });

  it("disables the confirm button while the text is blank", async () => {
    renderPrompt();
    const confirm = screen.getByRole("button", { name: "Rename" });
    expect(confirm).toBeDisabled();
    await userEvent.type(screen.getByRole("textbox", { name: "Name" }), "   ");
    expect(confirm).toBeDisabled();
    await userEvent.type(screen.getByRole("textbox", { name: "Name" }), "x");
    expect(confirm).toBeEnabled();
  });

  it("submits the trimmed text and closes", async () => {
    const onSubmit = renderPrompt();
    await userEvent.type(screen.getByRole("textbox", { name: "Name" }), "  Monthly  {Enter}");
    expect(onSubmit).toHaveBeenCalledWith("Monthly");
    expect(useStore.getState().dialog).toBeNull();
  });

  it("does not submit a blank text with Enter", async () => {
    const onSubmit = renderPrompt();
    await userEvent.type(screen.getByRole("textbox", { name: "Name" }), "  {Enter}");
    expect(onSubmit).not.toHaveBeenCalled();
    expect(useStore.getState().dialog).not.toBeNull();
  });

  it("closes without submitting on Cancel", async () => {
    const onSubmit = renderPrompt("Weekly");
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onSubmit).not.toHaveBeenCalled();
    expect(useStore.getState().dialog).toBeNull();
  });
});
