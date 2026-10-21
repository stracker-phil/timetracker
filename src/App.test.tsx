import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./api", async (orig) => ({ ...(await orig<object>()), api: (await import("./test/apiMock")).createApiMock() }));

import App from "./App";
import { api } from "./api";
import { resetStore, type View } from "./state/store";

const mocked = vi.mocked(api, true);

/** Renders the app and waits until the settings arrived and the commands are installed. */
async function renderApp(patch: Parameters<typeof resetStore>[0] = {}) {
  resetStore(patch);
  render(<App />);
  await screen.findByRole("button", { name: "13 px" });
}

const mainHeading = (name: string) => screen.getByRole("heading", { level: 1, name });
const sidebarEntry = (name: string) => within(screen.getByRole("navigation", { name: "Views" })).getByRole("button", { name: new RegExp(`^${name}`) });

describe("App", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("shell", () => {
    it("shows the toolbar title, the status bar and the Day view", async () => {
      await renderApp();
      expect(screen.getByText("TimeTracker")).toBeInTheDocument();
      expect(screen.getByText("No activity recorded yet")).toBeInTheDocument();
      expect(mainHeading("Day")).toBeInTheDocument();
    });

    it.each(["Day", "Week", "Month", "Reports", "Projects"])("lists %s in the sidebar", async (name) => {
      await renderApp();
      expect(sidebarEntry(name)).toBeInTheDocument();
    });

    it("marks the active view in the sidebar", async () => {
      await renderApp({ view: "month" });
      expect(sidebarEntry("Month")).toHaveAttribute("aria-current", "page");
      expect(sidebarEntry("Day")).not.toHaveAttribute("aria-current");
    });

    it("shows a toast for an error launch event", async () => {
      mocked.takeLaunchEvents.mockResolvedValueOnce([{ type: "error", message: "Could not open the link" }]);
      await renderApp();
      expect(await screen.findByRole("alert")).toHaveTextContent("Could not open the link");
    });

    it("shows a toast when the launch events cannot be read", async () => {
      mocked.takeLaunchEvents.mockRejectedValueOnce({ kind: "io", message: "Channel closed", code: null });
      await renderApp();
      expect(await screen.findByRole("alert")).toHaveTextContent("Launch request failed: Channel closed");
    });
  });

  describe("navigation", () => {
    it("switches the view when a sidebar entry is clicked", async () => {
      await renderApp();
      await userEvent.click(sidebarEntry("Reports"));
      expect(mainHeading("Reports")).toBeInTheDocument();
    });

    it("switches the view from the toolbar", async () => {
      await renderApp();
      await userEvent.click(within(screen.getByRole("group", { name: "View" })).getByRole("button", { name: "Month" }));
      expect(mainHeading("Month")).toBeInTheDocument();
    });

    it.each<[string, string, View]>([
      ["{Meta>}1{/Meta}", "Day", "projects"],
      ["{Meta>}2{/Meta}", "Week", "day"],
      ["{Meta>}3{/Meta}", "Month", "day"],
      ["{Meta>}4{/Meta}", "Reports", "day"],
      ["{Meta>}5{/Meta}", "Projects", "day"],
    ])("shows the view for %s", async (keys, heading, from) => {
      await renderApp({ view: from });
      await userEvent.keyboard(keys);
      expect(mainHeading(heading)).toBeInTheDocument();
    });
  });

  describe("sidebar", () => {
    it("hides with ⌘⌃S and returns with the same keys", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}{Control>}s{/Control}{/Meta}");
      expect(screen.queryByRole("complementary", { name: "Sidebar" })).not.toBeInTheDocument();
      await userEvent.keyboard("{Meta>}{Control>}s{/Control}{/Meta}");
      expect(screen.getByRole("complementary", { name: "Sidebar" })).toBeInTheDocument();
    });

    it("hides when the status bar button is clicked", async () => {
      await renderApp();
      await userEvent.click(screen.getByRole("button", { name: "Toggle sidebar" }));
      expect(screen.queryByRole("complementary", { name: "Sidebar" })).not.toBeInTheDocument();
    });
  });

  describe("settings", () => {
    it("opens with ⌘, and closes with it again", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>},{/Meta}");
      const dialog = await screen.findByRole("dialog");
      expect(within(dialog).getByRole("heading", { name: "Appearance" })).toBeInTheDocument();
      await userEvent.keyboard("{Meta>},{/Meta}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("opens from the toolbar button", async () => {
      await renderApp();
      await userEvent.click(screen.getByRole("button", { name: "Settings" }));
      expect(await screen.findByRole("heading", { name: "Appearance" })).toBeInTheDocument();
    });

    it("closes with Escape", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>},{/Meta}");
      await screen.findByRole("dialog");
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
  });

  describe("command palette", () => {
    it("opens with ⌘K", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      expect(await screen.findByRole("dialog", { name: "Command palette" })).toBeInTheDocument();
    });

    it("filters the commands by the typed text", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await userEvent.type(await screen.findByLabelText("Search commands"), "zoom");
      const options = screen.getAllByRole("option").map((o) => o.textContent);
      expect(options).toHaveLength(3);
      expect(options.join(" ")).toMatch(/Zoom in.*Zoom out.*Reset zoom/);
    });

    it("says so when no command matches", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await userEvent.type(await screen.findByLabelText("Search commands"), "qqq");
      expect(screen.getByText("No matching commands.")).toBeInTheDocument();
    });

    it("runs the chosen command on Enter and closes", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await userEvent.type(await screen.findByLabelText("Search commands"), "show week{Enter}");
      expect(mainHeading("Week")).toBeInTheDocument();
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("runs a command that is clicked", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await userEvent.click(await screen.findByRole("option", { name: /Show Projects/ }));
      expect(mainHeading("Projects")).toBeInTheDocument();
    });

    it("closes with Escape without running anything", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await screen.findByRole("dialog");
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(mainHeading("Day")).toBeInTheDocument();
    });
  });
});
