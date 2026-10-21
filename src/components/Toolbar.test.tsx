import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { installCommands } from "../commands";
import { resetStore } from "../state/store";
import { settings } from "../test/settings";
import { Dialogs } from "./Dialogs";
import { Toolbar } from "./Toolbar";

describe("Toolbar", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ settings: settings() });
    installCommands();
    render(
      <>
        <Toolbar />
        <Dialogs />
      </>,
    );
  });

  it("shows the app title and the Settings button only", () => {
    expect(screen.getByText("TimeTracker")).toBeInTheDocument();
    expect(screen.getAllByRole("button").map((b) => b.getAttribute("aria-label"))).toEqual(["Settings"]);
  });

  it("has no view switch", () => {
    expect(screen.queryByRole("group", { name: "View" })).not.toBeInTheDocument();
    for (const name of ["Day", "Week", "Month", "Reports"]) expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();
  });

  it("opens Settings from its button", async () => {
    await userEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(await screen.findByRole("heading", { name: "Appearance" })).toBeInTheDocument();
  });
});
