import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type ClientInput, type ProjectInput } from "../../api";
import { client, project, projectItem, tag } from "../../test/projectFixtures";
import { resetStore, useStore } from "../store";
import { groupProjects, projectLabel } from "./projects";

const mocked = vi.mocked(api, true);
const toastMessages = () => useStore.getState().toasts.map((t) => t.message);
const fieldError = { kind: "invalid_input", message: "Invalid name", code: null, field: { path: "name", message: "already exists" } };
const input: ProjectInput = { clientId: null, name: "Website", color: "blue", shade: null };
const clientInput: ClientInput = { name: "Acme", color: "teal" };

describe("projects slice", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  describe("loadProjects", () => {
    it("stores the projects, clients, tags and recent projects the backend lists", async () => {
      const item = projectItem({ project: { id: "p1" } });
      mocked.projectsList.mockResolvedValueOnce([item]);
      mocked.clientsList.mockResolvedValueOnce([client()]);
      mocked.tagsList.mockResolvedValueOnce([tag({ name: "billing" })]);
      mocked.projectsRecent.mockResolvedValueOnce([project({ id: "p1" })]);
      await useStore.getState().loadProjects();
      const s = useStore.getState();
      expect([s.projects, s.clients, s.tags, s.recent, s.projectsLoaded]).toEqual([[item], [client()], [tag({ name: "billing" })], [project({ id: "p1" })], true]);
    });

    it("asks for archived projects and clients too", async () => {
      await useStore.getState().loadProjects();
      expect(mocked.projectsList).toHaveBeenCalledWith(true);
      expect(mocked.clientsList).toHaveBeenCalledWith(true);
    });

    it("toasts and stays unloaded when a list cannot be read", async () => {
      mocked.tagsList.mockRejectedValueOnce({ kind: "db", message: "Locked", code: null });
      await useStore.getState().loadProjects();
      expect(toastMessages()).toEqual(["Could not load the projects: Locked"]);
      expect(useStore.getState().projectsLoaded).toBe(false);
    });
  });

  describe("writes", () => {
    it("saveProject sends the id and input and returns the saved project", async () => {
      const saved = await useStore.getState().saveProject("p9", input);
      expect(mocked.projectsSave).toHaveBeenCalledWith("p9", input);
      expect(saved.id).toBe("p9");
    });

    it.each([
      ["saveProject", () => useStore.getState().saveProject(null, input)],
      ["saveClient", () => useStore.getState().saveClient(null, clientInput)],
      ["saveTag", () => useStore.getState().saveTag(null, "billing")],
      ["deleteTag", () => useStore.getState().deleteTag("tag-1")],
      ["archiveProject", () => useStore.getState().archiveProject("p1", true)],
      ["archiveClient", () => useStore.getState().archiveClient("c1", true)],
    ])("%s reloads the lists afterwards", async (_, run) => {
      mocked.projectsList.mockResolvedValueOnce([projectItem({ project: { name: "After" } })]);
      await run();
      await vi.waitFor(() => expect(useStore.getState().projects.map((p) => p.project.name)).toEqual(["After"]));
    });

    it("saveClient sends the id and input", async () => {
      await useStore.getState().saveClient("c9", clientInput);
      expect(mocked.clientsSave).toHaveBeenCalledWith("c9", clientInput);
    });

    it("archiveProject passes the archived flag on", async () => {
      await useStore.getState().archiveProject("p1", false);
      expect(mocked.projectsArchive).toHaveBeenCalledWith("p1", false);
    });

    it("rejects and toasts when a write fails without a field", async () => {
      mocked.projectsSave.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      await expect(useStore.getState().saveProject(null, input)).rejects.toMatchObject({ message: "Disk full" });
      expect(toastMessages()).toEqual(["Could not save the project: Disk full"]);
    });

    it("rejects without a toast when the error names a field", async () => {
      mocked.clientsSave.mockRejectedValueOnce(fieldError);
      await expect(useStore.getState().saveClient(null, clientInput)).rejects.toMatchObject({ field: { path: "name" } });
      expect(toastMessages()).toEqual([]);
    });

    it("reloads the lists after a failed write too", async () => {
      mocked.tagsDelete.mockRejectedValueOnce({ kind: "not_found", message: "Gone", code: null });
      mocked.tagsList.mockResolvedValueOnce([tag({ name: "left" })]);
      await expect(useStore.getState().deleteTag("tag-1")).rejects.toBeDefined();
      await vi.waitFor(() => expect(useStore.getState().tags.map((t) => t.name)).toEqual(["left"]));
    });
  });

  describe("groupProjects", () => {
    const acme = client({ id: "c-acme", name: "Acme" });
    const bolt = client({ id: "c-bolt", name: "bolt" });
    const inClient = (c: typeof acme, name: string, archivedAt: number | null = null) => projectItem({ project: { id: `p-${name}`, name, clientId: c.id, archivedAt }, client: c });
    const loose = (name: string) => projectItem({ project: { id: `p-${name}`, name } });

    it("lists clients by name ignoring case, each with its projects by name, and client-less projects last", () => {
      const groups = groupProjects([loose("Solo"), inClient(bolt, "Zed"), inClient(acme, "Beta"), inClient(acme, "Alpha")], [bolt, acme], false);
      expect(groups.map((g) => [g.client?.name ?? null, g.projects.map((p) => p.project.name)])).toEqual([
        ["Acme", ["Alpha", "Beta"]],
        ["bolt", ["Zed"]],
        [null, ["Solo"]],
      ]);
    });

    it("lists a client without projects", () => {
      expect(groupProjects([], [acme], false)).toEqual([{ client: acme, projects: [] }]);
    });

    it("leaves out the client-less group when no project lacks a client", () => {
      expect(groupProjects([inClient(acme, "Alpha")], [acme], false).map((g) => g.client)).toEqual([acme]);
    });

    it("hides archived projects and clients unless asked", () => {
      const oldClient = client({ id: "c-old", name: "Old", archivedAt: 5 });
      const projects = [inClient(acme, "Live"), inClient(acme, "Dead", 5), loose("Gone")].map((p) => (p.project.name === "Gone" ? { ...p, project: { ...p.project, archivedAt: 5 } } : p));
      expect(groupProjects(projects, [acme, oldClient], false).map((g) => g.projects.map((p) => p.project.name))).toEqual([["Live"]]);
      const all = groupProjects(projects, [acme, oldClient], true);
      expect(all.map((g) => [g.client?.name ?? null, g.projects.map((p) => p.project.name)])).toEqual([
        ["Acme", ["Dead", "Live"]],
        ["Old", []],
        [null, ["Gone"]],
      ]);
    });
  });

  describe("projectLabel", () => {
    it("joins client and project name", () => {
      expect(projectLabel(projectItem({ project: { name: "Site" }, client: client({ name: "Acme" }) }))).toBe("Acme / Site");
    });

    it("is the project name alone without a client", () => {
      expect(projectLabel(projectItem({ project: { name: "Site" } }))).toBe("Site");
    });
  });
});
