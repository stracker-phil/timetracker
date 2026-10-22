import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { client, projectItem, projectsStore } from "../../test/projectFixtures";
import { Dialogs } from "../Dialogs";
import { Toasts } from "../Toasts";
import { ProjectsPage } from "./ProjectsPage";

const mocked = vi.mocked(api, true);

const acme = client({ id: "c-acme", name: "Acme" });
const empty = client({ id: "c-empty", name: "Zeta" });
const site = projectItem({ project: { id: "p-site", name: "Website", clientId: acme.id }, client: acme });
const solo = projectItem({ project: { id: "p-solo", name: "Solo work" } });
const old = projectItem({ project: { id: "p-old", name: "Legacy", clientId: acme.id, archivedAt: 5 }, client: acme });

function renderPage(store: Parameters<typeof projectsStore>[0]) {
  const patch = projectsStore(store);
  mocked.projectsList.mockResolvedValue(patch.projects ?? []);
  mocked.clientsList.mockResolvedValue(patch.clients ?? []);
  mocked.tagsList.mockResolvedValue(patch.tags ?? []);
  resetStore(patch);
  render(
    <>
      <ProjectsPage />
      <Dialogs />
      <Toasts />
    </>,
  );
}

const bands = () => screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);

describe("ProjectsPage", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("grouping", () => {
    it("lists a band per client, with its projects, then No client, then the tags", () => {
      renderPage({ projects: [solo, site], clients: [acme] });
      expect(bands()).toEqual(["Acme", "No client", "Tags"]);
      expect(within(screen.getByRole("region", { name: "Acme" })).getByRole("button", { name: "Website" })).toBeInTheDocument();
      expect(within(screen.getByRole("region", { name: "No client" })).getByRole("button", { name: "Solo work" })).toBeInTheDocument();
    });

    it("says No projects under a client without projects", () => {
      renderPage({ projects: [site], clients: [acme, empty] });
      expect(within(screen.getByRole("region", { name: "Zeta" })).getByText("No projects")).toBeInTheDocument();
    });

    it("explains the empty page once loaded", () => {
      renderPage({});
      expect(screen.getByText(/No projects yet/)).toBeInTheDocument();
    });

    it("does not claim the page is empty before the projects loaded", () => {
      resetStore();
      render(<ProjectsPage />);
      expect(screen.queryByText(/No projects yet/)).not.toBeInTheDocument();
    });
  });

  describe("project rows", () => {
    it("shows when the project was last used", () => {
      const item = projectItem({ project: { name: "Retainer" }, lastUsed: Date.now() - 2 * 86_400_000 });
      renderPage({ projects: [item] });
      expect(screen.getByText("last used 2 d ago")).toBeInTheDocument();
    });

    it("shows the hours of this week and this month", () => {
      const item = projectItem({ project: { name: "Retainer" }, weekMs: 5_400_000, monthMs: 45_000_000 });
      renderPage({ projects: [item] });
      expect(screen.getByTitle("This week: 1:30 h")).toHaveTextContent("1:30");
      expect(screen.getByTitle("This month: 12:30 h")).toHaveTextContent("12:30");
    });

    it("shows zero hours for a project without recordings", () => {
      renderPage({ projects: [projectItem()] });
      expect(screen.getByTitle("This week: 0:00 h")).toHaveTextContent("0:00");
      expect(screen.getByTitle("This month: 0:00 h")).toHaveTextContent("0:00");
    });

    it("badges a billable project only", () => {
      const billable = projectItem({ project: { id: "p-bill", name: "Retainer", billable: true } });
      renderPage({ projects: [billable, solo] });
      const rows = screen.getAllByRole("listitem");
      const row = (name: string) => within(rows.find((r) => within(r).queryByRole("button", { name }))!);
      expect(row("Retainer").getByText("billable")).toBeInTheDocument();
      expect(row("Solo work").queryByText("billable")).not.toBeInTheDocument();
    });

    it("omits last used for a project that was never recorded", () => {
      renderPage({ projects: [projectItem()] });
      expect(screen.queryByText(/last used/)).not.toBeInTheDocument();
    });

    it("reloads the hours when recordings change", async () => {
      const recorded = (weekMs: number) => projectItem({ project: { id: "p-site", name: "Website" }, weekMs });
      renderPage({ projects: [recorded(3_600_000)] });
      expect(await screen.findByTitle("This week: 1:00 h")).toBeInTheDocument();
      mocked.projectsList.mockResolvedValue([recorded(7_200_000)]);
      act(() => useStore.getState().dataChanged({ kind: "recordings", day: "2026-03-10" }));
      expect(await screen.findByTitle("This week: 2:00 h")).toBeInTheDocument();
      expect(screen.queryByTitle("This week: 1:00 h")).not.toBeInTheDocument();
    });
  });

  describe("client band head", () => {
    it("labels the hours columns and counts the projects", () => {
      renderPage({ projects: [site, projectItem({ project: { id: "p-two", name: "Intranet", clientId: acme.id }, client: acme })], clients: [acme] });
      const band = within(screen.getByRole("region", { name: "Acme" }));
      expect(band.getByText("This week")).toBeInTheDocument();
      expect(band.getByText("This month")).toBeInTheDocument();
      expect(band.getByText("2 projects")).toBeInTheDocument();
    });

    it("uses the singular for one project", () => {
      renderPage({ projects: [site], clients: [acme] });
      expect(within(screen.getByRole("region", { name: "Acme" })).getByText("1 project")).toBeInTheDocument();
    });
  });

  describe("archived", () => {
    it("hides archived projects until Show archived is switched on", async () => {
      renderPage({ projects: [site, old], clients: [acme] });
      expect(screen.queryByRole("button", { name: "Legacy" })).not.toBeInTheDocument();
      await userEvent.click(screen.getByRole("switch", { name: "Show archived" }));
      expect(screen.getByRole("button", { name: "Legacy" })).toBeInTheDocument();
    });

    it("restores an archived project with its Restore button", async () => {
      renderPage({ projects: [old], clients: [acme] });
      await userEvent.click(screen.getByRole("switch", { name: "Show archived" }));
      await userEvent.click(screen.getByRole("button", { name: "Restore" }));
      expect(mocked.projectsArchive).toHaveBeenCalledWith("p-old", false);
    });

    it("offers no Restore button on active projects", () => {
      renderPage({ projects: [site], clients: [acme] });
      expect(screen.queryByRole("button", { name: "Restore" })).not.toBeInTheDocument();
    });

    it("marks an archived client", async () => {
      renderPage({ clients: [client({ name: "Retired", archivedAt: 5 })] });
      await userEvent.click(screen.getByRole("switch", { name: "Show archived" }));
      expect(screen.getByText("archived")).toBeInTheDocument();
    });
  });

  describe("opening sheets", () => {
    it("opens the project sheet of a clicked project, prefilled", async () => {
      renderPage({ projects: [site], clients: [acme] });
      await userEvent.click(screen.getByRole("button", { name: "Website" }));
      const dialog = await screen.findByRole("dialog", { name: "Edit project" });
      expect(within(dialog).getByLabelText("Name")).toHaveValue("Website");
    });

    it("opens an empty project sheet from New project", async () => {
      renderPage({});
      await userEvent.click(screen.getByRole("button", { name: /New project/ }));
      expect(await screen.findByRole("dialog", { name: "New project" })).toBeInTheDocument();
    });

    it("opens the project sheet with the client preselected from a band's plus button, and saves with it", async () => {
      renderPage({ projects: [site], clients: [acme] });
      await userEvent.click(within(screen.getByRole("region", { name: "Acme" })).getByRole("button", { name: "New project for Acme" }));
      const dialog = await screen.findByRole("dialog", { name: "New project" });
      expect(within(dialog).getByLabelText("Client")).toHaveDisplayValue("Acme");
      await userEvent.type(within(dialog).getByLabelText("Name"), "Intranet");
      await userEvent.click(within(dialog).getByRole("button", { name: "Save" }));
      expect(mocked.projectsSave).toHaveBeenCalledWith(null, expect.objectContaining({ clientId: "c-acme", name: "Intranet" }));
    });

    it("opens the project sheet without a client from the No client band", async () => {
      renderPage({ projects: [solo], clients: [acme] });
      await userEvent.click(within(screen.getByRole("region", { name: "No client" })).getByRole("button", { name: "New project without a client" }));
      const dialog = await screen.findByRole("dialog", { name: "New project" });
      expect(within(dialog).getByLabelText("Client")).toHaveDisplayValue("No client");
    });

    it("offers no plus button on an archived client's band", async () => {
      renderPage({ clients: [client({ id: "c-ret", name: "Retired", archivedAt: 5 })] });
      await userEvent.click(screen.getByRole("switch", { name: "Show archived" }));
      expect(screen.getByRole("region", { name: "Retired" })).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "New project for Retired" })).not.toBeInTheDocument();
    });

    it("opens an empty client sheet from New client", async () => {
      renderPage({});
      await userEvent.click(screen.getByRole("button", { name: /New client/ }));
      expect(await screen.findByRole("dialog", { name: "New client" })).toBeInTheDocument();
    });

    it("opens the client sheet from a client's name", async () => {
      renderPage({ clients: [acme] });
      await userEvent.click(screen.getByRole("button", { name: "Acme" }));
      expect(await screen.findByRole("dialog", { name: "Edit client" })).toBeInTheDocument();
    });
  });

  describe("row context menu", () => {
    const openMenu = (name: string) => fireEvent.contextMenu(screen.getByRole("button", { name }));

    it("offers Edit…, Archive and Copy id for an active project", () => {
      renderPage({ projects: [site], clients: [acme] });
      openMenu("Website");
      expect(screen.getAllByRole("menuitem").map((i) => i.textContent)).toEqual(["Edit…", "Archive", "Copy id"]);
    });

    it("offers Restore instead of Archive for an archived project", async () => {
      renderPage({ projects: [old], clients: [acme] });
      await userEvent.click(screen.getByRole("switch", { name: "Show archived" }));
      openMenu("Legacy");
      expect(screen.getAllByRole("menuitem").map((i) => i.textContent)).toEqual(["Edit…", "Restore", "Copy id"]);
    });

    it("archives the project with Archive", async () => {
      renderPage({ projects: [site], clients: [acme] });
      openMenu("Website");
      await userEvent.click(screen.getByRole("menuitem", { name: "Archive" }));
      expect(mocked.projectsArchive).toHaveBeenCalledWith("p-site", true);
    });

    it("opens the project sheet with Edit…", async () => {
      renderPage({ projects: [site], clients: [acme] });
      openMenu("Website");
      await userEvent.click(screen.getByRole("menuitem", { name: "Edit…" }));
      expect(await screen.findByRole("dialog", { name: "Edit project" })).toBeInTheDocument();
      expect(useStore.getState().dialog).toEqual({ type: "project", id: "p-site" });
    });

    it("copies the project id with Copy id", async () => {
      renderPage({ projects: [site], clients: [acme] });
      openMenu("Website");
      await userEvent.click(screen.getByRole("menuitem", { name: "Copy id" }));
      expect(await screen.findByText("Copied the project id")).toBeInTheDocument();
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith("p-site");
    });
  });
});
