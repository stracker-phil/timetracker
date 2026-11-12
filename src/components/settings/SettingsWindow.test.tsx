import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Settings } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { renderWithClient, seedQuery, SETTINGS_KEY } from "../../test/queries";
import { settings } from "../../test/settings";
import { SettingsWindow, type SettingsRequest } from "./SettingsWindow";

const mocked = vi.mocked(api, true);

function Harness({ initial }: { initial?: string }) {
  const [request, setRequest] = useState<SettingsRequest>({ id: initial });
  return <SettingsWindow request={request} onOpen={(id) => setRequest({ id })} />;
}

function renderWindow(initial?: string, loaded = true, patch: Partial<Settings> = {}) {
  if (loaded) seedQuery(SETTINGS_KEY, settings({ accent: "blue", ...patch }));
  return renderWithClient(<Harness initial={initial} />);
}
const nav = () => screen.getByRole("navigation", { name: "Settings pages" });
const search = () => screen.getByRole("searchbox", { name: "Search Settings" });
const heading = () => screen.getByRole("heading", { level: 2 });
const results = () => within(nav()).getByRole("group", { name: "Results" });
const hitNames = () => within(results()).getAllByRole("button").map((b) => b.getAttribute("aria-label") ?? b.textContent?.trim());
const rowOf = (label: string) => screen.getByLabelText(label).closest<HTMLElement>(".settings-row")!;

function stubScrollIntoView() {
  const scrollIntoView = vi.fn();
  Element.prototype.scrollIntoView = scrollIntoView;
  return scrollIntoView;
}

describe("SettingsWindow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.updateSettings.mockImplementation(async (s) => s);
    resetStore();
  });

  afterEach(() => {
    Reflect.deleteProperty(Element.prototype, "scrollIntoView");
  });

  it("renders nothing until the settings are loaded", () => {
    mocked.getSettings.mockReturnValueOnce(new Promise(() => {}));
    const { container } = renderWindow(undefined, false);
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
      expect(names).toEqual(["General", "Appearance", "Shortcuts", "Capture", "Tracker", "Timeline", "Data", "Status", "About"]);
      expect(within(nav()).getByRole("group", { name: "Tracking" })).toBeInTheDocument();
    });

    it("replaces the page list with one Results group while typing", async () => {
      renderWindow();
      await userEvent.type(search(), "accent");
      expect(results()).toBeInTheDocument();
      expect(within(nav()).queryByRole("group", { name: "App" })).not.toBeInTheDocument();
      expect(within(nav()).queryByRole("button", { name: "General" })).not.toBeInTheDocument();
    });

    it("shows a row hit as its page label with the row title below, named by both", async () => {
      renderWindow();
      await userEvent.type(search(), "accent");
      const [hit] = within(results()).getAllByRole("button");
      expect(hit).toHaveAccessibleName("Accent, Appearance");
      expect(hit).toHaveTextContent(/^AppearanceAccent$/);
    });

    it("restores the page list when the field is cleared", async () => {
      renderWindow();
      await userEvent.type(search(), "accent");
      await userEvent.clear(search());
      expect(within(nav()).getByRole("button", { name: "General" })).toBeInTheDocument();
      expect(within(nav()).queryByRole("group", { name: "Results" })).not.toBeInTheDocument();
    });

    it.each([
      ["a page label", "capture", "Capture"],
      ["a row title", "away after", "Away after, Tracker"],
      ["a row keyword", "tint", "Accent, Appearance"],
      ["a page keyword", "diagnostics", "Status"],
    ])("finds %s first", async (_kind, query, first) => {
      renderWindow();
      await userEvent.type(search(), query);
      expect(hitNames()[0]).toBe(first);
    });

    it("lists a page before its rows", async () => {
      renderWindow();
      await userEvent.type(search(), "capture");
      const names = hitNames();
      expect(names[0]).toBe("Capture");
      expect(names).toContain("Capture document paths, Capture");
    });

    it("ignores the case of the query", async () => {
      renderWindow();
      await userEvent.type(search(), "ACCENT");
      expect(within(nav()).getByRole("button", { name: "Accent, Appearance" })).toBeInTheDocument();
    });

    it("says No Results when nothing matches", async () => {
      renderWindow();
      await userEvent.type(search(), "qqqzzz");
      expect(within(nav()).getByText("No Results")).toBeInTheDocument();
      expect(within(nav()).queryAllByRole("button")).toHaveLength(0);
    });

    it("opens the page of a clicked row hit, scrolled to the row and washed", async () => {
      const scrollIntoView = stubScrollIntoView();
      renderWindow();
      await userEvent.type(search(), "away after");
      await userEvent.click(within(results()).getByRole("button", { name: "Away after, Tracker" }));
      expect(heading()).toHaveTextContent("Tracker");
      expect(scrollIntoView.mock.contexts).toEqual([rowOf("Away after")]);
      expect(rowOf("Away after")).toHaveClass("found");
    });

    it("opens the first hit on Enter, a row scrolled to and washed", async () => {
      const scrollIntoView = stubScrollIntoView();
      renderWindow();
      await userEvent.type(search(), "accent{Enter}");
      expect(heading()).toHaveTextContent("Appearance");
      const row = screen.getByText("Accent", { selector: ".settings-row-label" }).closest(".settings-row");
      expect(scrollIntoView.mock.contexts).toEqual([row]);
      expect(row).toHaveClass("found");
    });

    it("opens a page found by its label on Enter without scrolling or washing a row", async () => {
      const scrollIntoView = stubScrollIntoView();
      renderWindow();
      await userEvent.type(search(), "capture{Enter}");
      expect(heading()).toHaveTextContent("Capture");
      expect(scrollIntoView).not.toHaveBeenCalled();
    });

    it("keeps the current page on Enter when nothing matches", async () => {
      renderWindow("capture");
      await userEvent.type(search(), "qqqzzz{Enter}");
      expect(heading()).toHaveTextContent("Capture");
    });

    it("reveals the same row again when its hit is chosen twice", async () => {
      const scrollIntoView = stubScrollIntoView();
      renderWindow("capture");
      await userEvent.type(search(), "away after");
      const hit = within(results()).getByRole("button", { name: "Away after, Tracker" });
      await userEvent.click(hit);
      await userEvent.click(hit);
      expect(scrollIntoView.mock.contexts).toEqual([rowOf("Away after"), rowOf("Away after")]);
    });
  });

  describe("request target", () => {
    it("shows the page holding a row id and scrolls to the row, washed", () => {
      const scrollIntoView = stubScrollIntoView();
      renderWindow("away-after");
      expect(heading()).toHaveTextContent("Tracker");
      expect(within(nav()).getByRole("button", { name: "Tracker" })).toHaveAttribute("aria-current", "page");
      expect(scrollIntoView.mock.contexts).toEqual([rowOf("Away after")]);
      expect(rowOf("Away after")).toHaveClass("found");
    });

    it("shows a page id without scrolling to anything", () => {
      const scrollIntoView = stubScrollIntoView();
      renderWindow("capture");
      expect(heading()).toHaveTextContent("Capture");
      expect(scrollIntoView).not.toHaveBeenCalled();
    });

    it("reveals the row a hidden row depends on while it isn't shown", () => {
      const scrollIntoView = stubScrollIntoView();
      renderWindow("summary-interval", true, { timeline: { ...settings().timeline, summaryLane: false } });
      expect(heading()).toHaveTextContent("Timeline");
      expect(screen.queryByLabelText("Summary interval")).not.toBeInTheDocument();
      expect(scrollIntoView.mock.contexts).toEqual([rowOf("Summary lane")]);
      expect(rowOf("Summary lane")).toHaveClass("found");
    });

    it("reveals an anchored group as a whole", () => {
      const scrollIntoView = stubScrollIntoView();
      renderWindow("apps");
      expect(heading()).toHaveTextContent("Timeline");
      expect(scrollIntoView.mock.contexts).toEqual([screen.getByRole("region", { name: "Apps" })]);
    });

    it("reveals the row itself when it is shown", () => {
      const scrollIntoView = stubScrollIntoView();
      renderWindow("summary-interval");
      expect(scrollIntoView.mock.contexts).toEqual([rowOf("Summary interval")]);
      expect(rowOf("Summary lane")).not.toHaveClass("found");
    });
  });

  describe("changing a setting", () => {
    it("saves the whole settings with the change on top and shows no success toast", async () => {
      const current = settings({ accent: "blue" });
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
