import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentProps } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore } from "../../state/store";
import { client, project, projectItem, projectsStore } from "../../test/projectFixtures";
import { Toasts } from "../Toasts";
import { ProjectSheet } from "./ProjectSheet";

const mocked = vi.mocked(api, true);
const acme = client({ id: "c-acme", name: "Acme" });
const fieldError = (path: string, message: string) => ({ kind: "invalid_input", message: "Invalid", code: null, field: { path, message } });
const save = () => userEvent.click(screen.getByRole("button", { name: "Save" }));

function renderSheet(props: Partial<ComponentProps<typeof ProjectSheet>> = {}, store: Parameters<typeof projectsStore>[0] = { clients: [acme] }) {
  const onClose = vi.fn();
  resetStore(projectsStore(store));
  render(
    <>
      <ProjectSheet onClose={onClose} {...props} />
      <Toasts />
    </>,
  );
  return { onClose };
}

describe("ProjectSheet", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("creating", () => {
    it("saves the typed name with the defaults and closes", async () => {
      const { onClose } = renderSheet();
      expect(screen.getByRole("heading", { name: "New project" })).toBeInTheDocument();
      await userEvent.type(screen.getByLabelText("Name"), "Website");
      await save();
      expect(mocked.projectsSave).toHaveBeenCalledWith(null, { clientId: null, name: "Website", color: "orange", shade: null, billable: false });
      expect(onClose).toHaveBeenCalled();
    });

    it("saves the chosen color without a client", async () => {
      renderSheet();
      await userEvent.type(screen.getByLabelText("Name"), "Website");
      await userEvent.click(screen.getByRole("button", { name: "Accent teal" }));
      await save();
      expect(mocked.projectsSave).toHaveBeenCalledWith(null, { clientId: null, name: "Website", color: "teal", shade: null, billable: false });
    });

    it("starts with the Billable switch off and saves it on when toggled", async () => {
      renderSheet();
      const billable = screen.getByRole("switch", { name: /^Billable/ });
      expect(billable).not.toBeChecked();
      await userEvent.type(screen.getByLabelText("Name"), "Website");
      await userEvent.click(billable);
      expect(billable).toBeChecked();
      await save();
      expect(mocked.projectsSave).toHaveBeenCalledWith(null, expect.objectContaining({ billable: true }));
    });

    it("saves the chosen client", async () => {
      renderSheet();
      await userEvent.type(screen.getByLabelText("Name"), "Website");
      await userEvent.selectOptions(screen.getByLabelText("Client"), "Acme");
      await save();
      expect(mocked.projectsSave).toHaveBeenCalledWith(null, expect.objectContaining({ clientId: "c-acme", name: "Website", shade: null }));
    });

    it("starts with no shade pressed", () => {
      renderSheet();
      const shades = within(screen.getByRole("group", { name: "Shade" })).getAllByRole("button");
      expect(shades).toHaveLength(9);
      expect(shades.every((b) => b.getAttribute("aria-pressed") === "false")).toBe(true);
    });

    it("saves the picked shade as its zero-based index", async () => {
      renderSheet();
      await userEvent.type(screen.getByLabelText("Name"), "Website");
      await userEvent.click(screen.getByRole("button", { name: "Shade 3" }));
      expect(screen.getByRole("button", { name: "Shade 3" })).toHaveAttribute("aria-pressed", "true");
      await save();
      expect(mocked.projectsSave).toHaveBeenCalledWith(null, expect.objectContaining({ shade: 2 }));
    });

    it("offers a Color field and a Shade field without a client", () => {
      renderSheet();
      expect(screen.getByRole("group", { name: "Accent" })).toBeInTheDocument();
      expect(screen.getByText("Shade")).toBeInTheDocument();
    });

    it("replaces the Accent group by a shade field of the client's color when a client is chosen", async () => {
      renderSheet();
      await userEvent.selectOptions(screen.getByLabelText("Client"), "Acme");
      expect(screen.queryByRole("group", { name: "Accent" })).not.toBeInTheDocument();
      expect(screen.getByText("Shade of Acme's color")).toBeInTheDocument();
      expect(screen.getByRole("group", { name: "Shade" })).toBeInTheDocument();
    });

    it("has no Accent group for a preselected client", () => {
      renderSheet({ initialClientId: "c-acme" });
      expect(screen.queryByRole("group", { name: "Accent" })).not.toBeInTheDocument();
      expect(screen.getByText("Shade of Acme's color")).toBeInTheDocument();
    });

    it("preselects the client given by initialClientId and saves with it", async () => {
      renderSheet({ initialClientId: "c-acme" });
      expect(screen.getByLabelText("Client")).toHaveDisplayValue("Acme");
      await userEvent.type(screen.getByLabelText("Name"), "Website");
      await save();
      expect(mocked.projectsSave).toHaveBeenCalledWith(null, expect.objectContaining({ clientId: "c-acme" }));
    });

    it("starts without a client when no initialClientId is given", () => {
      renderSheet();
      expect(screen.getByLabelText("Client")).toHaveDisplayValue("No client");
    });

    it("rings the chosen color", async () => {
      renderSheet();
      expect(screen.getByRole("button", { name: "Accent orange" })).toHaveAttribute("aria-pressed", "true");
      await userEvent.click(screen.getByRole("button", { name: "Accent blue" }));
      expect(screen.getByRole("button", { name: "Accent blue" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("button", { name: "Accent orange" })).toHaveAttribute("aria-pressed", "false");
    });

    it("prefills the name given by a picker", () => {
      renderSheet({ initialName: "Newsletter" });
      expect(screen.getByLabelText("Name")).toHaveValue("Newsletter");
    });

    it("hands the saved project to onSaved", async () => {
      const onSaved = vi.fn();
      renderSheet({ onSaved });
      await userEvent.type(screen.getByLabelText("Name"), "Website");
      await save();
      expect(onSaved).toHaveBeenCalledWith(expect.objectContaining({ id: "project-new", name: "Website" }));
    });

    it("closes without saving on Cancel", async () => {
      const { onClose } = renderSheet();
      await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
      expect(onClose).toHaveBeenCalled();
      expect(mocked.projectsSave).not.toHaveBeenCalled();
    });

    it("offers no Archive button", () => {
      renderSheet();
      expect(screen.queryByRole("button", { name: /Archive|Restore/ })).not.toBeInTheDocument();
    });
  });

  describe("errors", () => {
    it("shows a field error under its field, keeps the sheet open and shows no toast", async () => {
      mocked.projectsSave.mockRejectedValueOnce(fieldError("name", "already exists"));
      const { onClose } = renderSheet();
      await userEvent.type(screen.getByLabelText("Name"), "Website");
      await save();
      const alert = await screen.findByRole("alert");
      expect(alert).toHaveTextContent("already exists");
      expect(alert.parentElement).toContainElement(screen.getByLabelText("Name"));
      expect(screen.getAllByRole("alert")).toHaveLength(1);
      expect(onClose).not.toHaveBeenCalled();
      expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
    });

    it("shows a shade error under the shade field", async () => {
      mocked.projectsSave.mockRejectedValueOnce(fieldError("shade", "out of range"));
      renderSheet();
      await userEvent.type(screen.getByLabelText("Name"), "Website");
      await save();
      const alert = await screen.findByRole("alert");
      expect(alert).toHaveTextContent("out of range");
      expect(alert.parentElement).toContainElement(screen.getByRole("group", { name: "Shade" }));
    });

    it("toasts an error without a field and keeps the sheet open", async () => {
      mocked.projectsSave.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      const { onClose } = renderSheet();
      await userEvent.type(screen.getByLabelText("Name"), "Website");
      await save();
      expect(await screen.findByText("Could not save the project: Disk full")).toBeInTheDocument();
      expect(onClose).not.toHaveBeenCalled();
    });
  });

  describe("new client", () => {
    async function chooseNewClient() {
      await userEvent.selectOptions(screen.getByLabelText("Client"), "New client…");
    }

    it("shows a New client field only after choosing New client…", async () => {
      renderSheet();
      expect(screen.queryByLabelText("New client")).not.toBeInTheDocument();
      await chooseNewClient();
      expect(screen.getByLabelText("New client")).toBeInTheDocument();
    });

    it("offers the Accent group and the Shade field for the new client", async () => {
      renderSheet({ initialClientId: "c-acme" });
      expect(screen.queryByRole("group", { name: "Accent" })).not.toBeInTheDocument();
      await chooseNewClient();
      expect(screen.getByRole("group", { name: "Accent" })).toBeInTheDocument();
      expect(screen.getByText("Shade")).toBeInTheDocument();
    });

    it("creates the client with the chosen accent first and saves the project with its id", async () => {
      const { onClose } = renderSheet();
      await userEvent.type(screen.getByLabelText("Name"), "Website");
      await chooseNewClient();
      await userEvent.type(screen.getByLabelText("New client"), "Beta");
      await userEvent.click(screen.getByRole("button", { name: "Accent teal" }));
      await save();
      expect(mocked.clientsSave).toHaveBeenCalledWith(null, { name: "Beta", color: "teal" });
      expect(mocked.projectsSave).toHaveBeenCalledWith(null, expect.objectContaining({ clientId: "client-new", name: "Website" }));
      expect(onClose).toHaveBeenCalled();
    });

    it("shows a client name error under the New client field and does not save the project", async () => {
      mocked.clientsSave.mockRejectedValueOnce(fieldError("name", "already exists"));
      renderSheet();
      await userEvent.type(screen.getByLabelText("Name"), "Website");
      await chooseNewClient();
      await userEvent.type(screen.getByLabelText("New client"), "Acme");
      await save();
      const alert = await screen.findByRole("alert");
      expect(alert).toHaveTextContent("already exists");
      expect(alert.parentElement).toContainElement(screen.getByLabelText("New client"));
      expect(mocked.projectsSave).not.toHaveBeenCalled();
    });

    it("keeps the created client when the project then fails, so a retry does not create it twice", async () => {
      mocked.projectsSave.mockRejectedValueOnce(fieldError("name", "already exists"));
      renderSheet();
      await userEvent.type(screen.getByLabelText("Name"), "Website");
      await chooseNewClient();
      await userEvent.type(screen.getByLabelText("New client"), "Beta");
      await save();
      await screen.findByRole("alert");
      await save();
      expect(mocked.clientsSave).toHaveBeenCalledTimes(1);
      expect(mocked.projectsSave).toHaveBeenLastCalledWith(null, expect.objectContaining({ clientId: "client-new" }));
    });
  });

  describe("editing", () => {
    const existing = projectItem({ project: { id: "p1", name: "Website", clientId: "c-acme", color: "purple", shade: 4 }, client: acme });

    it("prefills every field", () => {
      renderSheet({ id: "p1" }, { projects: [existing], clients: [acme] });
      expect(screen.getByRole("heading", { name: "Edit project" })).toBeInTheDocument();
      expect(screen.getByLabelText("Name")).toHaveValue("Website");
      expect(screen.getByLabelText("Client")).toHaveDisplayValue("Acme");
      expect(screen.getByRole("button", { name: "Shade 5" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("button", { name: "Shade 1" })).toHaveAttribute("aria-pressed", "false");
    });

    it("prefills the accent of a project without a client", () => {
      const loose = projectItem({ project: { id: "p4", color: "purple", shade: 1 } });
      renderSheet({ id: "p4" }, { projects: [loose] });
      expect(screen.getByRole("button", { name: "Accent purple" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("button", { name: "Shade 2" })).toHaveAttribute("aria-pressed", "true");
    });

    it("saves under the project's id", async () => {
      renderSheet({ id: "p1" }, { projects: [existing], clients: [acme] });
      await userEvent.clear(screen.getByLabelText("Name"));
      await userEvent.type(screen.getByLabelText("Name"), "Site");
      await save();
      expect(mocked.projectsSave).toHaveBeenCalledWith("p1", { clientId: "c-acme", name: "Site", color: "purple", shade: 4, billable: false });
    });

    it("reflects a billable project in the Billable switch and saves it off when toggled", async () => {
      const billable = projectItem({ project: { id: "p5", name: "Retainer", billable: true } });
      renderSheet({ id: "p5" }, { projects: [billable] });
      const toggle = screen.getByRole("switch", { name: /^Billable/ });
      expect(toggle).toBeChecked();
      await userEvent.click(toggle);
      await save();
      expect(mocked.projectsSave).toHaveBeenCalledWith("p5", expect.objectContaining({ billable: false }));
    });

    it("saves a newly picked shade", async () => {
      renderSheet({ id: "p1" }, { projects: [existing], clients: [acme] });
      await userEvent.click(screen.getByRole("button", { name: "Shade 9" }));
      await save();
      expect(mocked.projectsSave).toHaveBeenCalledWith("p1", expect.objectContaining({ shade: 8 }));
    });

    it("archives the project and closes", async () => {
      const { onClose } = renderSheet({ id: "p1" }, { projects: [existing], clients: [acme] });
      await userEvent.click(screen.getByRole("button", { name: "Archive" }));
      expect(mocked.projectsArchive).toHaveBeenCalledWith("p1", true);
      expect(onClose).toHaveBeenCalled();
    });

    it("offers Restore for an archived project", async () => {
      const archived = projectItem({ project: { ...project({ id: "p2", name: "Legacy" }), archivedAt: 5 } });
      renderSheet({ id: "p2" }, { projects: [archived] });
      await userEvent.click(screen.getByRole("button", { name: "Restore" }));
      expect(mocked.projectsArchive).toHaveBeenCalledWith("p2", false);
    });

    it("lists an archived client only when the project already belongs to it", () => {
      const old = client({ id: "c-old", name: "Old", archivedAt: 5 });
      const other = client({ id: "c-gone", name: "Gone", archivedAt: 5 });
      const inOld = projectItem({ project: { id: "p3", clientId: "c-old" }, client: old });
      renderSheet({ id: "p3" }, { projects: [inOld], clients: [acme, old, other] });
      const options = within(screen.getByLabelText("Client")).getAllByRole("option").map((o) => o.textContent);
      expect(options).toEqual(["No client", "Acme", "Old (archived)", "New client…"]);
    });
  });
});
