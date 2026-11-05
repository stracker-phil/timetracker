import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { installCommands } from "../../commands";
import { resetStore } from "../../state/store";
import { renderWithClient, seedQuery, SETTINGS_KEY } from "../../test/queries";
import { settings } from "../../test/settings";
import { SettingsWindow } from "./SettingsWindow";

const mocked = vi.mocked(api, true);

const group = (name: string) => within(screen.getByRole("group", { name }));

function renderPage(patch: Parameters<typeof settings>[0] = {}) {
  resetStore();
  seedQuery(SETTINGS_KEY, settings(patch));
  installCommands();
  return renderWithClient(<SettingsWindow page="appearance" onPage={() => {}} />);
}

describe("Settings Appearance page", () => {
  beforeEach(() => vi.clearAllMocks());

  it("has no Mode switcher", () => {
    renderPage();
    expect(screen.queryByRole("group", { name: "Mode" })).not.toBeInTheDocument();
  });

  it.each(["Light mode scheme", "Dark mode scheme"])("offers %s as Default, Light and Dark", (name) => {
    renderPage();
    expect(group(name).getAllByRole("button").map((b) => b.textContent)).toEqual(["Default", "Light", "Dark"]);
  });

  it("presses the saved scheme in each group", () => {
    renderPage({ lightScheme: "light", darkScheme: "default" });
    expect(group("Light mode scheme").getByRole("button", { name: "Light" })).toHaveAttribute("aria-pressed", "true");
    expect(group("Light mode scheme").getByRole("button", { name: "Default" })).toHaveAttribute("aria-pressed", "false");
    expect(group("Dark mode scheme").getByRole("button", { name: "Default" })).toHaveAttribute("aria-pressed", "true");
    expect(group("Dark mode scheme").getByRole("button", { name: "Light" })).toHaveAttribute("aria-pressed", "false");
  });

  it("saves the clicked tile as the light scheme and leaves the dark one unchanged", async () => {
    renderPage({ lightScheme: "default", darkScheme: "dark" });
    await userEvent.click(group("Light mode scheme").getByRole("button", { name: "Dark" }));
    expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ lightScheme: "dark", darkScheme: "dark" }));
  });

  it("saves the clicked tile as the dark scheme and leaves the light one unchanged", async () => {
    renderPage({ lightScheme: "default", darkScheme: "dark" });
    await userEvent.click(group("Dark mode scheme").getByRole("button", { name: "Light" }));
    expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ lightScheme: "default", darkScheme: "light" }));
  });

  it("saves without a Settings saved toast", async () => {
    renderPage();
    await userEvent.click(screen.getByRole("button", { name: "Accent teal" }));
    expect(screen.queryByText("Settings saved")).not.toBeInTheDocument();
  });

  describe("Text size", () => {
    const sizes = () => group("Text size");

    it("offers Small, Medium and Large", () => {
      renderPage();
      expect(sizes().getAllByRole("button").map((b) => b.textContent)).toEqual(["Small", "Medium", "Large"]);
    });

    it("presses the saved size", () => {
      renderPage({ textSize: "large" });
      expect(sizes().getByRole("button", { name: "Large" })).toHaveAttribute("aria-pressed", "true");
      expect(sizes().getByRole("button", { name: "Medium" })).toHaveAttribute("aria-pressed", "false");
    });

    it("saves the clicked size", async () => {
      renderPage();
      await userEvent.click(sizes().getByRole("button", { name: "Small" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ textSize: "small" }));
    });

    it("shows no pixel sizes", () => {
      renderPage();
      expect(document.body.textContent).not.toMatch(/\d\s*px|\bpx\b/i);
    });

    it.each(["UI size", "Data size", "UI font", "Mono font"])("has no %s field", (label) => {
      renderPage();
      expect(screen.queryByLabelText(label)).not.toBeInTheDocument();
    });
  });

  describe("Accent", () => {
    it("saves the accent of the clicked swatch", async () => {
      renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Accent teal" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ accent: "teal" }));
    });

    it("saves the system accent when the System swatch is clicked", async () => {
      renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Accent system" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ accent: "system" }));
    });

    it("marks the System swatch as pressed while the accent is system", () => {
      renderPage({ accent: "system" });
      expect(screen.getByRole("button", { name: "Accent system" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("button", { name: "Accent orange" })).toHaveAttribute("aria-pressed", "false");
    });

    it("marks the current accent as pressed", () => {
      renderPage();
      expect(screen.getByRole("button", { name: "Accent orange" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("button", { name: "Accent teal" })).toHaveAttribute("aria-pressed", "false");
    });
  });

  describe("Dates", () => {
    it.each([
      ["Calendar title", ["Short", "Full"]],
      ["Day title", ["Long", "Short"]],
    ])("offers %s as %j", (name, options) => {
      renderPage();
      expect(group(name).getAllByRole("button").map((b) => b.textContent)).toEqual(options);
    });

    it("presses the saved choices", () => {
      renderPage({ calendarTitle: "full", dayTitle: "short" });
      expect(group("Calendar title").getByRole("button", { name: "Full" })).toHaveAttribute("aria-pressed", "true");
      expect(group("Calendar title").getByRole("button", { name: "Short" })).toHaveAttribute("aria-pressed", "false");
      expect(group("Day title").getByRole("button", { name: "Short" })).toHaveAttribute("aria-pressed", "true");
      expect(group("Day title").getByRole("button", { name: "Long" })).toHaveAttribute("aria-pressed", "false");
    });

    it("saves calendarTitle when the Calendar title is changed", async () => {
      renderPage();
      await userEvent.click(group("Calendar title").getByRole("button", { name: "Full" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ calendarTitle: "full", dayTitle: "long" }));
      await vi.waitFor(() => expect(group("Calendar title").getByRole("button", { name: "Full" })).toHaveAttribute("aria-pressed", "true"));
    });

    it("saves dayTitle when the Day title is changed", async () => {
      renderPage();
      await userEvent.click(group("Day title").getByRole("button", { name: "Short" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ dayTitle: "short", calendarTitle: "short" }));
      await vi.waitFor(() => expect(group("Day title").getByRole("button", { name: "Short" })).toHaveAttribute("aria-pressed", "true"));
    });
  });
});
