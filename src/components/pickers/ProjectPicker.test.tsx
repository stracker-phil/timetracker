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

function renderPicker(store: Parameters<typeof projectsStore>[0] = {}, value: string | null = null, showRecent = false) {
  const onChange = vi.fn();
  resetStore(projectsStore({ projects: [site, news, solo, legacy], clients: [acme, bolt], recent: [solo.project, site.project, legacy.project], ...store }));
  render(<ProjectPicker value={value} onChange={onChange} showRecent={showRecent} />);
  return { onChange };
}

const field = () => screen.getByRole("combobox", { name: "Project" });
const open = async () => {
  await userEvent.click(field());
  if (field().getAttribute("aria-expanded") !== "true") await userEvent.keyboard("{ArrowDown}");
};
const options = () => within(screen.getByRole("listbox", { name: "Projects" })).getAllByRole("option");
const optionTexts = () => options().map((o) => o.textContent);

describe("ProjectPicker", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("list", () => {
    it("lists Recent first, then the projects by client", async () => {
      renderPicker();
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

    it("shows an empty value with the No project placeholder when nothing is chosen", () => {
      renderPicker();
      expect(field()).toHaveValue("");
      expect(field()).toHaveAttribute("placeholder", "No project");
    });
  });

  describe("opening", () => {
    it("opens nothing on focus when a project is chosen", async () => {
      renderPicker({}, "p-site");
      await userEvent.tab();
      expect(field()).toHaveFocus();
      expect(field()).toHaveAttribute("aria-expanded", "false");
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    });

    it("opens the list on Tab focus when no project is chosen", async () => {
      renderPicker();
      await userEvent.tab();
      expect(field()).toHaveFocus();
      expect(field()).toHaveAttribute("aria-expanded", "true");
      expect(screen.getByRole("listbox", { name: "Projects" })).toBeInTheDocument();
    });

    it("opens the list on the first click when no project is chosen", async () => {
      renderPicker();
      await userEvent.click(field());
      expect(field()).toHaveFocus();
      expect(screen.getByRole("listbox", { name: "Projects" })).toBeInTheDocument();
    });

    it("closes the list with Escape and keeps focus when no project is chosen", async () => {
      renderPicker();
      await userEvent.tab();
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
      expect(field()).toHaveFocus();
      expect(field()).toHaveAttribute("aria-expanded", "false");
    });

    it("only focuses on the first click and opens the list on the second when a project is chosen", async () => {
      renderPicker({}, "p-site");
      await userEvent.click(field());
      expect(field()).toHaveFocus();
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
      await userEvent.click(field());
      expect(field()).toHaveAttribute("aria-expanded", "true");
      expect(screen.getByRole("listbox", { name: "Projects" })).toBeInTheDocument();
    });

    it("toggles the list with the chevron", async () => {
      renderPicker({}, "p-site");
      await userEvent.click(screen.getByRole("button", { name: "Show projects" }));
      expect(screen.getByRole("listbox", { name: "Projects" })).toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "Show projects" }));
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    });

    it.each(["{ArrowDown}", "{ArrowUp}", " "])("opens on %j with the current project highlighted and checked", async (key) => {
      renderPicker({}, "p-news");
      await userEvent.click(field());
      await userEvent.keyboard(key);
      const current = screen.getByRole("option", { name: /^Newsletter/ });
      expect(current).toHaveAttribute("aria-selected", "true");
      expect(current).toHaveAttribute("aria-checked", "true");
    });

    it("opens filtered by the typed character with the first match highlighted", async () => {
      renderPicker();
      await userEvent.click(field());
      await userEvent.keyboard("n");
      expect(field()).toHaveValue("n");
      expect(options()[0]).toHaveTextContent("Newsletter");
      expect(options()[0]).toHaveAttribute("aria-selected", "true");
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

    it("keeps the value when Escape closes the list", async () => {
      renderPicker({}, "p-site");
      await open();
      await userEvent.keyboard("{Escape}");
      expect(field()).toHaveValue("Acme / Website");
    });

    it("does nothing on Backspace while the list is closed", async () => {
      const { onChange } = renderPicker({}, "p-site");
      await userEvent.click(field());
      await userEvent.keyboard("{Backspace}");
      expect(onChange).not.toHaveBeenCalled();
      expect(field()).toHaveValue("Acme / Website");
    });

    it("lets Enter reach the surrounding form while the list is closed", async () => {
      const onSubmit = vi.fn((e: { preventDefault: () => void }) => e.preventDefault());
      resetStore(projectsStore({ projects: [site] }));
      render(
        <form onSubmit={onSubmit}>
          <ProjectPicker value="p-site" onChange={vi.fn()} />
          <button type="submit">Go</button>
        </form>,
      );
      await userEvent.click(field());
      await userEvent.keyboard("{Enter}");
      expect(onSubmit).toHaveBeenCalled();
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

  describe("recent projects", () => {
    const recentList = () => screen.getByRole("list", { name: "Recent projects" });

    it("lists the active recent projects by name and client, skipping archived ones", () => {
      renderPicker({}, null, true);
      const buttons = within(recentList()).getAllByRole("button");
      expect(buttons.map((b) => b.getAttribute("aria-label"))).toEqual(["Solo", "Acme / Website"]);
    });

    it("picks a project by clicking it", async () => {
      const { onChange } = renderPicker({}, null, true);
      await userEvent.click(within(recentList()).getByRole("button", { name: "Acme / Website" }));
      expect(onChange).toHaveBeenCalledWith("p-site");
    });

    it("shows at most five", () => {
      const many = Array.from({ length: 7 }, (_, i) => projectItem({ project: { id: `p-${i}`, name: `Project ${i}` } }));
      renderPicker({ projects: many, recent: many.map((m) => m.project) }, null, true);
      expect(within(recentList()).getAllByRole("button")).toHaveLength(5);
    });

    it("is not tabbable from the field", async () => {
      renderPicker({}, null, true);
      await userEvent.tab();
      await userEvent.tab();
      expect(within(recentList()).getByRole("button", { name: "Solo" })).not.toHaveFocus();
    });

    it("renders nothing without showRecent", () => {
      renderPicker();
      expect(screen.queryByRole("list", { name: "Recent projects" })).not.toBeInTheDocument();
    });

    it("renders nothing while a project is chosen", () => {
      renderPicker({}, "p-site", true);
      expect(screen.queryByRole("list", { name: "Recent projects" })).not.toBeInTheDocument();
    });

    it("renders nothing when no recent project is active", () => {
      renderPicker({ recent: [legacy.project] }, null, true);
      expect(screen.queryByRole("list", { name: "Recent projects" })).not.toBeInTheDocument();
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
