import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { renderWithClient, seedQuery, SETTINGS_KEY } from "../../test/queries";
import { settings } from "../../test/settings";
import { NumberField, TextField } from "./fields";
import { SettingsWindow } from "./SettingsWindow";

const mocked = vi.mocked(api, true);

describe("TextField", () => {
  const onCommit = vi.fn();

  beforeEach(() => {
    onCommit.mockReset();
  });

  const renderField = () => render(<TextField label="Display name" value="Ada" onCommit={onCommit} />);

  it("commits the typed text when the field loses focus, not on each keystroke", async () => {
    renderField();
    await userEvent.type(screen.getByLabelText("Display name"), " Lovelace");
    expect(onCommit).not.toHaveBeenCalled();
    await userEvent.tab();
    expect(onCommit).toHaveBeenCalledWith("Ada Lovelace");
  });

  it("commits the typed text on Enter", async () => {
    renderField();
    await userEvent.type(screen.getByLabelText("Display name"), "!{Enter}");
    expect(onCommit).toHaveBeenCalledWith("Ada!");
  });

  it("does not commit when the text is unchanged", async () => {
    renderField();
    await userEvent.click(screen.getByLabelText("Display name"));
    await userEvent.tab();
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("shows the message of a refused value under the field and keeps the typed text", async () => {
    onCommit.mockResolvedValue("That name is taken.");
    renderField();
    await userEvent.type(screen.getByLabelText("Display name"), "x");
    await userEvent.tab();
    expect(await screen.findByRole("alert")).toHaveTextContent("That name is taken.");
    expect(screen.getByLabelText("Display name")).toHaveValue("Adax");
    expect(screen.getByLabelText("Display name")).toBeInvalid();
  });

  it("clears the message once a later value is accepted", async () => {
    onCommit.mockResolvedValueOnce("That name is taken.").mockResolvedValue(null);
    renderField();
    const input = screen.getByLabelText("Display name");
    await userEvent.type(input, "x");
    await userEvent.tab();
    await screen.findByRole("alert");
    await userEvent.clear(input);
    await userEvent.type(input, "Grace");
    await userEvent.tab();
    await vi.waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
    expect(onCommit).toHaveBeenLastCalledWith("Grace");
  });
});

describe("NumberField", () => {
  const onCommit = vi.fn();

  beforeEach(() => {
    onCommit.mockReset();
  });

  const renderField = () => render(<NumberField label="Away after" value={5} min={1} max={120} suffix="min" onCommit={onCommit} />);

  it("commits a number within the range on blur", async () => {
    renderField();
    const input = screen.getByLabelText("Away after");
    await userEvent.clear(input);
    await userEvent.type(input, "30");
    await userEvent.tab();
    expect(onCommit).toHaveBeenCalledWith(30);
  });

  it.each(["500", "0", "2.5", ""])("shows the range message and saves nothing for %j", async (text) => {
    renderField();
    const input = screen.getByLabelText("Away after");
    await userEvent.clear(input);
    if (text) await userEvent.type(input, text);
    await userEvent.tab();
    expect(await screen.findByRole("alert")).toHaveTextContent("Enter a whole number from 1 to 120.");
    expect(onCommit).not.toHaveBeenCalled();
  });
});

describe("a field on a settings page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.updateSettings.mockImplementation(async (s) => s);
    resetStore();
    seedQuery(SETTINGS_KEY, settings());
  });

  const renderTracker = () => renderWithClient(<SettingsWindow request={{ id: "tracker" }} onOpen={() => {}} />);

  it("shows Rust's invalid_input message under the field, keeps the text and raises no toast", async () => {
    mocked.updateSettings.mockRejectedValue({ kind: "invalid_input", message: "Away time must be shorter than a day.", code: null });
    renderTracker();
    const input = screen.getByLabelText("Away after");
    await userEvent.clear(input);
    await userEvent.type(input, "90");
    await userEvent.tab();
    expect(await screen.findByRole("alert")).toHaveTextContent("Away time must be shorter than a day.");
    expect(input).toHaveValue(90);
    expect(useStore.getState().toasts).toEqual([]);
  });

  it("saves a valid value inline and clears an earlier message", async () => {
    mocked.updateSettings.mockRejectedValueOnce({ kind: "invalid_input", message: "Away time must be shorter than a day.", code: null });
    renderTracker();
    const input = screen.getByLabelText("Away after");
    await userEvent.clear(input);
    await userEvent.type(input, "90");
    await userEvent.tab();
    await screen.findByRole("alert");
    await userEvent.clear(input);
    await userEvent.type(input, "45");
    await userEvent.tab();
    await vi.waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
    expect(mocked.updateSettings).toHaveBeenLastCalledWith({ ...settings(), tracker: { ...settings().tracker, idleMinutes: 45 } });
    expect(useStore.getState().toasts).toEqual([]);
  });

  it("does not save an out-of-range number", async () => {
    renderTracker();
    const input = screen.getByLabelText("Away after");
    await userEvent.clear(input);
    await userEvent.type(input, "999");
    await userEvent.tab();
    expect(await screen.findByRole("alert")).toHaveTextContent("Enter a whole number from 1 to 120.");
    expect(mocked.updateSettings).not.toHaveBeenCalled();
  });
});
