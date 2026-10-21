import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { installCommands } from "../commands";
import { resetStore, useStore } from "../state/store";
import { client, projectItem, projectsStore } from "../test/projectFixtures";
import { CommandPalette } from "./CommandPalette";
import { Dialogs } from "./Dialogs";

const acme = client({ id: "c-acme", name: "Acme" });
const promo = projectItem({ project: { id: "p-promo", name: "Promo", clientId: acme.id }, client: acme });
const solo = projectItem({ project: { id: "p-solo", name: "Solo" } });
const old = projectItem({ project: { id: "p-old", name: "Promo archive", archivedAt: 5 } });

async function openPalette(query = "") {
  resetStore({ ...projectsStore({ projects: [promo, solo, old], clients: [acme] }), paletteOpen: true, view: "projects" });
  installCommands();
  render(
    <>
      <CommandPalette />
      <Dialogs />
    </>,
  );
  const input = screen.getByLabelText("Search commands");
  if (query) await userEvent.type(input, query);
}

const options = () => screen.getAllByRole("option").map((o) => o.textContent ?? "");
const projectOptions = () => options().filter((t) => t.startsWith("Project: "));

describe("CommandPalette projects", () => {
  beforeEach(() => vi.clearAllMocks());

  it("lists no projects while nothing is typed", async () => {
    await openPalette();
    expect(projectOptions()).toEqual([]);
    expect(options().length).toBeGreaterThan(0);
  });

  it("lists matching projects as Project: Client / Name after the matching commands", async () => {
    await openPalette("pro");
    const texts = options();
    const first = texts.findIndex((t) => t.startsWith("Project: "));
    expect(texts.some((t) => t.startsWith("New project"))).toBe(true);
    expect(texts[first]).toContain("Project: Acme / Promo");
    expect(texts.slice(0, first).every((t) => !t.startsWith("Project: "))).toBe(true);
    expect(texts.slice(first).every((t) => t.startsWith("Project: "))).toBe(true);
  });

  it("names a project without a client by its name alone", async () => {
    await openPalette("solo");
    expect(projectOptions()).toEqual([expect.stringContaining("Project: Solo")]);
  });

  it("never lists archived projects", async () => {
    await openPalette("p:archive");
    expect(screen.queryByRole("option")).not.toBeInTheDocument();
    expect(screen.getByText("No matches.")).toBeInTheDocument();
  });

  it("limits the list to projects after p:", async () => {
    await openPalette("p:");
    expect(options()).toHaveLength(2);
    expect(options().every((t) => t.startsWith("Project: "))).toBe(true);
  });

  it("filters the projects after p: by the text behind it", async () => {
    await openPalette("p:solo");
    expect(options()).toEqual([expect.stringContaining("Project: Solo")]);
  });

  it("limits the list to commands after >", async () => {
    await openPalette(">pro");
    expect(projectOptions()).toEqual([]);
    expect(options().some((t) => t.startsWith("New project"))).toBe(true);
  });

  it("opens the project sheet on Enter and closes the palette", async () => {
    await openPalette("p:promo{Enter}");
    expect(await screen.findByRole("dialog", { name: "Edit project" })).toBeInTheDocument();
    expect(screen.getByLabelText("Name")).toHaveValue("Promo");
    expect(useStore.getState().paletteOpen).toBe(false);
  });

  it("opens the project sheet of a clicked project", async () => {
    await openPalette("p:");
    await userEvent.click(screen.getByRole("option", { name: /Project: Solo/ }));
    expect(await screen.findByRole("dialog", { name: "Edit project" })).toBeInTheDocument();
    expect(screen.getByLabelText("Name")).toHaveValue("Solo");
  });
});
