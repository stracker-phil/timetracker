import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore, useStore, type View } from "../../state/store";
import { at } from "../../test/dayFixtures";
import { projectsStore } from "../../test/projectFixtures";
import { settings } from "../../test/settings";
import { EditorSheet } from "./EditorSheet";

const mocked = vi.mocked(api, true);

/** Opens a new-recording editor while `view` is shown. */
async function openIn(view: View) {
  resetStore({ view, settings: settings(), ...projectsStore() });
  render(<EditorSheet />);
  act(() => useStore.getState().openEditor({ mode: "create", start: at("09:00"), end: at("09:30") }));
  await vi.waitFor(() => expect(useStore.getState().draft).not.toBeNull());
}

describe("EditorSheet", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.recordingsList.mockResolvedValue([]);
  });

  it.each(["week", "month"] as const)("shows the editor opened in the %s view", async (view) => {
    await openIn(view);
    expect(await screen.findByRole("form", { name: "New recording" })).toBeInTheDocument();
  });

  it("hides an editor that was opened in another view", async () => {
    await openIn("day");
    await screen.findByRole("form", { name: "New recording" });
    act(() => useStore.setState({ view: "week" }));
    expect(screen.queryByRole("form")).not.toBeInTheDocument();
  });

  it("shows nothing without an editor", () => {
    resetStore({ view: "week" });
    render(<EditorSheet />);
    expect(screen.queryByRole("form")).not.toBeInTheDocument();
  });
});
