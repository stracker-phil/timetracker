import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsWindow } from "./SettingsWindow";

const mocked = vi.mocked(api, true);

function Harness({ initial }: { initial?: string }) {
  const [page, setPage] = useState(initial);
  return <SettingsWindow page={page} onPage={setPage} />;
}

const renderWindow = (initial?: string) => render(<Harness initial={initial} />);
const nav = () => screen.getByRole("navigation", { name: "Settings pages" });
const search = () => screen.getByRole("searchbox", { name: "Search Settings" });
const heading = () => screen.getByRole("heading", { level: 2 });

describe("SettingsWindow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.updateSettings.mockImplementation(async (s) => s);
    resetStore({ settings: settings({ accent: "blue" }) });
  });

  afterEach(() => {
    Reflect.deleteProperty(Element.prototype, "scrollIntoView");
  });

  it("renders nothing until the settings are loaded", () => {
    resetStore({ settings: null });
    const { container } = renderWindow();
    expect(container).toBeEmptyDOMElement();
  });

  it("opens the first page and marks it as the current one", () => {
    renderWindow();
    expect(heading()).toHaveTextContent("General");
    expect(within(nav()).getByRole("button", { name: "General" })).toHaveAttribute("aria-current", "page");
    expect(within(nav()).getByRole("button", { name: "Capture" })).not.toHaveAttribute("aria-current");
  });

  it("shows the page whose entry is clicked", async () => {
    renderWindow();
    await userEvent.click(within(nav()).getByRole("button", { name: "Capture" }));
    expect(heading()).toHaveTextContent("Capture");
    expect(within(nav()).getByRole("button", { name: "Capture" })).toHaveAttribute("aria-current", "page");
    expect(within(nav()).getByRole("button", { name: "General" })).not.toHaveAttribute("aria-current");
  });

  it("has no close button", () => {
    renderWindow();
    expect(screen.queryByRole("button", { name: /close|✕/i })).not.toBeInTheDocument();
  });

  describe("search", () => {
    it("lists all pages under their groups while the field is empty", () => {
      renderWindow();
      const names = within(nav()).getAllByRole("button").map((b) => b.textContent?.trim());
      expect(names).toEqual(["General", "Appearance", "Shortcuts", "Capture", "Tracker", "Timeline", "Data"]);
    });

    it("finds a page by a keyword and shows the matching section beside it", async () => {
      renderWindow();
      await userEvent.type(search(), "accent");
      const buttons = within(nav()).getAllByRole("button");
      expect(buttons).toHaveLength(1);
      expect(buttons[0]).toHaveAccessibleName("Appearance Look");
      expect(within(nav()).queryByRole("button", { name: "General" })).not.toBeInTheDocument();
    });

    it.each([
      ["a page label", "capt", ["Capture"]],
      ["a group", "tracking", ["Capture", "Tracker", "Timeline"]],
      ["a section title", "away", ["Tracker Away"]],
      ["a keyword", "hotkey", ["Shortcuts Global shortcut"]],
    ])("finds pages by %s", async (_kind, query, names) => {
      renderWindow();
      await userEvent.type(search(), query);
      expect(within(nav()).getAllByRole("button").map((b) => b.getAttribute("aria-label") ?? b.textContent?.trim())).toEqual(names);
    });

    it("ignores the case of the query", async () => {
      renderWindow();
      await userEvent.type(search(), "ACCENT");
      expect(within(nav()).getByRole("button", { name: "Appearance Look" })).toBeInTheDocument();
    });

    it("says No Results when nothing matches", async () => {
      renderWindow();
      await userEvent.type(search(), "zzz");
      expect(within(nav()).getByText("No Results")).toBeInTheDocument();
      expect(within(nav()).queryAllByRole("button")).toHaveLength(0);
    });

    it("opens the first hit on Enter, scrolled to the section it was found by", async () => {
      const scrollIntoView = vi.fn();
      Element.prototype.scrollIntoView = scrollIntoView;
      renderWindow();
      await userEvent.type(search(), "accent{Enter}");
      expect(heading()).toHaveTextContent("Appearance");
      expect(scrollIntoView.mock.contexts).toEqual([screen.getByRole("region", { name: "Look" })]);
    });

    it("opens a page found by its label on Enter without scrolling to a section", async () => {
      const scrollIntoView = vi.fn();
      Element.prototype.scrollIntoView = scrollIntoView;
      renderWindow();
      await userEvent.type(search(), "capture{Enter}");
      expect(heading()).toHaveTextContent("Capture");
      expect(scrollIntoView).not.toHaveBeenCalled();
    });

    it("keeps the current page on Enter when nothing matches", async () => {
      renderWindow("capture");
      await userEvent.type(search(), "zzz{Enter}");
      expect(heading()).toHaveTextContent("Capture");
    });
  });

  describe("section target", () => {
    it("shows the page holding a section id and scrolls that group to the top", () => {
      const scrollIntoView = vi.fn();
      Element.prototype.scrollIntoView = scrollIntoView;
      renderWindow("away");
      expect(heading()).toHaveTextContent("Tracker");
      expect(within(nav()).getByRole("button", { name: "Tracker" })).toHaveAttribute("aria-current", "page");
      expect(scrollIntoView.mock.contexts).toHaveLength(1);
      expect(scrollIntoView.mock.contexts[0]).toContainElement(screen.getByLabelText("Away after"));
    });
  });

  describe("changing a setting", () => {
    it("saves the whole settings with the change on top and shows no success toast", async () => {
      const current = settings({ accent: "blue" });
      resetStore({ settings: current });
      renderWindow("capture");
      await userEvent.click(screen.getByRole("switch", { name: "Capture document paths" }));
      expect(mocked.updateSettings).toHaveBeenCalledWith({ ...current, capture: { ...current.capture, captureDocuments: false } });
      await vi.waitFor(() => expect(screen.getByRole("switch", { name: "Capture document paths" })).not.toBeChecked());
      expect(useStore.getState().toasts).toEqual([]);
    });

    it("toasts a failed save", async () => {
      mocked.updateSettings.mockRejectedValue({ kind: "io", message: "Disk full", code: null });
      renderWindow("capture");
      await userEvent.click(screen.getByRole("switch", { name: "Capture document paths" }));
      await vi.waitFor(() => expect(useStore.getState().toasts.map((t) => t.message).join()).toContain("Disk full"));
    });
  });
});
