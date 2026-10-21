import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { resetStore } from "../state/store";
import { installCommands, runCommand } from "./index";

const mocked = vi.mocked(api, true);

describe("app.quit", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    installCommands();
  });

  it("quits the app", () => {
    expect(runCommand("app.quit")).toBe(true);
    expect(mocked.quitApp).toHaveBeenCalled();
  });
});
