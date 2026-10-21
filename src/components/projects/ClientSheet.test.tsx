import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore } from "../../state/store";
import { client, projectsStore } from "../../test/projectFixtures";
import { ClientSheet } from "./ClientSheet";

const mocked = vi.mocked(api, true);
const save = () => userEvent.click(screen.getByRole("button", { name: "Save" }));

function renderSheet(id?: string, clients = [client({ id: "c1", name: "Acme" })]) {
  const onClose = vi.fn();
  resetStore(projectsStore({ clients }));
  render(<ClientSheet id={id} onClose={onClose} />);
  return { onClose };
}

describe("ClientSheet", () => {
  beforeEach(() => vi.clearAllMocks());

  it("creates a client from the typed name and closes", async () => {
    const { onClose } = renderSheet();
    expect(screen.getByRole("heading", { name: "New client" })).toBeInTheDocument();
    await userEvent.type(screen.getByLabelText("Name"), "Beta");
    await save();
    expect(mocked.clientsSave).toHaveBeenCalledWith(null, { name: "Beta", color: "" });
    expect(onClose).toHaveBeenCalled();
  });

  it("starts a new client with no color pressed", () => {
    renderSheet();
    const swatches = within(screen.getByRole("group", { name: "Accent" })).getAllByRole("button");
    expect(swatches.every((b) => b.getAttribute("aria-pressed") === "false")).toBe(true);
  });

  it("saves the picked color with a new client", async () => {
    renderSheet();
    await userEvent.type(screen.getByLabelText("Name"), "Beta");
    await userEvent.click(screen.getByRole("button", { name: "Accent teal" }));
    expect(screen.getByRole("button", { name: "Accent teal" })).toHaveAttribute("aria-pressed", "true");
    await save();
    expect(mocked.clientsSave).toHaveBeenCalledWith(null, { name: "Beta", color: "teal" });
  });

  it("shows a color error from Rust under the Color field", async () => {
    mocked.clientsSave.mockRejectedValueOnce({ kind: "invalid_input", message: "Invalid", code: null, field: { path: "color", message: "already taken" } });
    renderSheet();
    await userEvent.type(screen.getByLabelText("Name"), "Beta");
    await save();
    expect(await screen.findByRole("alert")).toHaveTextContent("already taken");
  });

  it("shows a name error from Rust under the field and stays open", async () => {
    mocked.clientsSave.mockRejectedValueOnce({ kind: "invalid_input", message: "Invalid", code: null, field: { path: "name", message: "already exists" } });
    const { onClose } = renderSheet();
    await userEvent.type(screen.getByLabelText("Name"), "Acme");
    await save();
    expect(await screen.findByRole("alert")).toHaveTextContent("already exists");
    expect(onClose).not.toHaveBeenCalled();
  });

  it("prefills the name and color of an existing client and renames it under its id", async () => {
    renderSheet("c1", [client({ id: "c1", name: "Acme", color: "purple" })]);
    expect(screen.getByRole("heading", { name: "Edit client" })).toBeInTheDocument();
    expect(screen.getByLabelText("Name")).toHaveValue("Acme");
    expect(screen.getByRole("button", { name: "Accent purple" })).toHaveAttribute("aria-pressed", "true");
    await userEvent.type(screen.getByLabelText("Name"), " Corp");
    await save();
    expect(mocked.clientsSave).toHaveBeenCalledWith("c1", { name: "Acme Corp", color: "purple" });
  });

  it("archives the client and closes", async () => {
    const { onClose } = renderSheet("c1");
    await userEvent.click(screen.getByRole("button", { name: "Archive" }));
    expect(mocked.clientsArchive).toHaveBeenCalledWith("c1", true);
    expect(onClose).toHaveBeenCalled();
  });

  it("restores an archived client", async () => {
    renderSheet("c1", [client({ id: "c1", name: "Acme", archivedAt: 5 })]);
    await userEvent.click(screen.getByRole("button", { name: "Restore" }));
    expect(mocked.clientsArchive).toHaveBeenCalledWith("c1", false);
  });

  it("offers no Archive for a new client", () => {
    renderSheet();
    expect(screen.queryByRole("button", { name: /Archive|Restore/ })).not.toBeInTheDocument();
  });

  it("closes without saving on Cancel", async () => {
    const { onClose } = renderSheet();
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onClose).toHaveBeenCalled();
    expect(mocked.clientsSave).not.toHaveBeenCalled();
  });
});
