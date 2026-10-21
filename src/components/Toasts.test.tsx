import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { resetStore, useStore } from "../state/store";
import { Toasts } from "./Toasts";

describe("Toasts", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  afterEach(() => vi.useRealTimers());

  describe("action button", () => {
    it("runs the action and dismisses the toast when clicked", async () => {
      const run = vi.fn();
      useStore.getState().toast("info", "Safari is hidden", { label: "Undo", run });
      render(<Toasts />);
      await userEvent.click(screen.getByRole("button", { name: "Undo" }));
      expect(run).toHaveBeenCalled();
      expect(screen.queryByText("Safari is hidden")).not.toBeInTheDocument();
    });

    it("is absent on a toast without an action", () => {
      useStore.getState().toast("info", "Plain");
      render(<Toasts />);
      expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();
    });
  });

  describe("auto dismiss", () => {
    async function showFor(ms: number, action?: { label: string; run: () => void; ttlMs?: number }) {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      useStore.getState().toast("info", "Heads up", action);
      render(<Toasts />);
      await act(() => vi.advanceTimersByTimeAsync(ms));
    }

    it("removes an info toast after 4 s", async () => {
      await showFor(4100);
      expect(screen.queryByText("Heads up")).not.toBeInTheDocument();
    });

    it("keeps a toast with an action beyond the 4 s of its kind", async () => {
      await showFor(5000, { label: "Undo", run: vi.fn() });
      expect(screen.getByText("Heads up")).toBeInTheDocument();
    });

    it("removes a toast with an action after 10 s", async () => {
      await showFor(10_100, { label: "Undo", run: vi.fn() });
      expect(screen.queryByText("Heads up")).not.toBeInTheDocument();
    });

    it("keeps a toast with a ttlMs beyond the 10 s default", async () => {
      await showFor(15_000, { label: "Undo", run: vi.fn(), ttlMs: 30_000 });
      expect(screen.getByText("Heads up")).toBeInTheDocument();
    });

    it("removes a toast with a ttlMs after that time", async () => {
      await showFor(30_100, { label: "Undo", run: vi.fn(), ttlMs: 30_000 });
      expect(screen.queryByText("Heads up")).not.toBeInTheDocument();
    });

    it("removes a toast with a shorter ttlMs before the 10 s default", async () => {
      await showFor(2100, { label: "Undo", run: vi.fn(), ttlMs: 2000 });
      expect(screen.queryByText("Heads up")).not.toBeInTheDocument();
    });
  });
});
