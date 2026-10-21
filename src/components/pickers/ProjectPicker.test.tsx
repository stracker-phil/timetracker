import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore } from "../../state/store";
import { client, projectItem, projectsStore } from "../../test/projectFixtures";
import { ProjectPicker } from "./ProjectPicker";

const mocked = vi.mocked(api, true);

const acme = client({ id: "c-acme", name: "Acme" });
const bolt = client({ id: "c-bolt", name: "Bolt" });
const site = projectItem({ project: { id: "p-site", name: "Website", clientId: acme.id }, client: acme });
const news = projectItem({ project: { id: "p-news", name: "Newsletter", clientId: acme.id }, client: acme });
const solo = projectItem({ project: { id: "p-solo", name: "Solo" } });
const legacy = projectItem({ project: { id: "p-legacy", name: "Legacy", clientId: acme.id, archivedAt: 5 }, client: acme });

function renderPicker(store: Parameters<typeof projectsStore>[0] = {}, value: string | null = null) {
  const onChange = vi.fn();
  resetStore(projectsStore({ projects: [site, news, solo, legacy], clients: [acme, bolt], recent: [solo.project, site.project, legacy.project], ...store }));
  render(<ProjectPicker value={value} onChange={onChange} />);
  return { onChange };
}

const field = () => screen.getByRole("combobox", { name: "Project" });
const open = () => userEvent.click(field());
const options = () => within(screen.getByRole("listbox", { name: "Projects" })).getAllByRole("option");
const optionTexts = () => options().map((o) => o.textContent);

describe("ProjectPicker", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("list", () => {
    it("opens on focus with Recent first, then the projects by client", async () => {
      renderPicker();
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
      await open();
      expect(optionTexts()).toEqual(["Solo", "WebsiteAcme", "NewsletterAcme", "WebsiteAcme", "Solo", "New project…"]);
      expect(screen.getByText("Recent")).toBeInTheDocument();
      expect(screen.getByText("No client")).toBeInTheDocument();
    });

    it("never lists archived projects, not even as recent", async () => {
      renderPicker();
      await open();
      expect(screen.queryByText("Legacy")).not.toBeInTheDocument();
    });

    it("omits the Recent section when no recent project is active", async () => {
      renderPicker({ recent: [legacy.project] });
      await open();
      expect(screen.queryByText("Recent")).not.toBeInTheDocument();
    });

    it("shows the chosen project as Client / Name", () => {
      renderPicker({}, "p-site");
      expect(field()).toHaveValue("Acme / Website");
    });
  });

  describe("search", () => {
    it("lists only the matches, then the New project option", async () => {
      renderPicker();
      await userEvent.type(field(), "news");
      expect(optionTexts()).toEqual(["NewsletterAcme", 'New project "news"…']);
    });

    it("matches on the client name too", async () => {
      renderPicker();
      await userEvent.type(field(), "acme");
      expect(optionTexts().slice(0, -1).sort()).toEqual(["NewsletterAcme", "WebsiteAcme"]);
    });

    it("puts the best match first", async () => {
      renderPicker({ projects: [projectItem({ project: { id: "p-b", name: "Beta alpha" } }), projectItem({ project: { id: "p-a", name: "Alpha" } })], recent: [] });
      await userEvent.type(field(), "alp");
      expect(optionTexts()).toEqual(["Alpha", "Beta alpha", 'New project "alp"…']);
    });

    it("offers only New project when nothing matches", async () => {
      renderPicker();
      await userEvent.type(field(), "zzz");
      expect(optionTexts()).toEqual(['New project "zzz"…']);
    });
  });

  describe("keyboard", () => {
    it("picks the first option on Enter", async () => {
      const { onChange } = renderPicker();
      await open();
      await userEvent.keyboard("{Enter}");
      expect(onChange).toHaveBeenCalledWith("p-solo");
    });

    it("moves with the arrow keys and picks the highlighted option", async () => {
      const { onChange } = renderPicker();
      await open();
      await userEvent.keyboard("{ArrowDown}");
      expect(options()[1]).toHaveAttribute("aria-selected", "true");
      await userEvent.keyboard("{ArrowDown}{ArrowUp}{Enter}");
      expect(onChange).toHaveBeenCalledWith("p-site");
    });

    it("stops at the last option", async () => {
      renderPicker();
      await open();
      await userEvent.keyboard("{ArrowDown>10/}");
      expect(options().at(-1)).toHaveAttribute("aria-selected", "true");
    });

    it("closes the list with Escape without the key reaching the surroundings", async () => {
      const outer = vi.fn();
      resetStore(projectsStore({ projects: [site] }));
      render(
        <div onKeyDown={outer}>
          <ProjectPicker value={null} onChange={vi.fn()} />
        </div>,
      );
      await open();
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
      expect(outer).not.toHaveBeenCalledWith(expect.objectContaining({ key: "Escape" }));
    });

    it("picks a clicked option", async () => {
      const { onChange } = renderPicker();
      await open();
      await userEvent.click(options()[2]);
      expect(onChange).toHaveBeenCalledWith("p-news");
    });

    it("closes the list after picking", async () => {
      renderPicker();
      await open();
      await userEvent.keyboard("{Enter}");
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    });
  });

  describe("New project", () => {
    it("opens the project sheet prefilled with the query", async () => {
      renderPicker();
      await userEvent.type(field(), "Podcast");
      await userEvent.click(screen.getByRole("option", { name: 'New project "Podcast"…' }));
      const dialog = await screen.findByRole("dialog", { name: "New project" });
      expect(within(dialog).getByLabelText("Name")).toHaveValue("Podcast");
    });

    it("picks the saved project", async () => {
      const { onChange } = renderPicker();
      await userEvent.type(field(), "Podcast{Enter}");
      await userEvent.click(await screen.findByRole("button", { name: "Save" }));
      expect(mocked.projectsSave).toHaveBeenCalledWith(null, expect.objectContaining({ name: "Podcast" }));
      expect(onChange).toHaveBeenCalledWith("project-new");
    });

    it("picks nothing when the sheet is cancelled", async () => {
      const { onChange } = renderPicker();
      await userEvent.type(field(), "Podcast{Enter}");
      await userEvent.click(await screen.findByRole("button", { name: "Cancel" }));
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(onChange).not.toHaveBeenCalled();
    });

    it("is the only option without projects, with no name yet", async () => {
      renderPicker({ projects: [], recent: [] });
      await open();
      expect(optionTexts()).toEqual(["New project…"]);
    });
  });
});
