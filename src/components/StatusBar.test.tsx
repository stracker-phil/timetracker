import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { resetStore } from "../state/store";
import { at } from "../test/dayFixtures";
import { runningStatus } from "../test/timerFixtures";
import { StatusBar } from "./StatusBar";

const mocked = vi.mocked(api, true);

describe("StatusBar", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows the snapped selection with its length", () => {
    resetStore({ selectionSnap: { start: at("09:00"), end: at("10:30") } });
    render(<StatusBar />);
    expect(screen.getByLabelText("Selection")).toHaveTextContent("09:00–10:30 · 1:30");
  });

  it("shows no selection without one", () => {
    resetStore();
    render(<StatusBar />);
    expect(screen.queryByLabelText("Selection")).not.toBeInTheDocument();
  });

  describe("running timer", () => {
    it("shows the project and the elapsed time", () => {
      resetStore({ timer: runningStatus({ projectName: "Docs" }) });
      render(<StatusBar />);
      expect(screen.getByRole("button", { name: "⏱ Docs 1:23" })).toBeInTheDocument();
    });

    it("shows nothing without a running timer", () => {
      resetStore({ timer: { running: null } });
      render(<StatusBar />);
      expect(screen.queryByTitle("Stop timer")).not.toBeInTheDocument();
    });

    it("stops the timer when clicked", async () => {
      resetStore({ timer: runningStatus() });
      render(<StatusBar />);
      await userEvent.click(screen.getByTitle("Stop timer"));
      expect(mocked.timerStop).toHaveBeenCalled();
    });
  });
});
