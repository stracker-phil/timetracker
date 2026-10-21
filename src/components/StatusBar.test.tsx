import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { resetStore } from "../state/store";
import { at } from "../test/dayFixtures";
import { StatusBar } from "./StatusBar";

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
});
