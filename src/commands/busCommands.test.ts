import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { resetStore } from "../state/store";
import { BUS_COMMANDS } from "./busCommands";
import { installCommands, runCommand } from "./index";
import { bindingFor, getCommand } from "./registry";

const mocked = vi.mocked(api, true);

describe("bus commands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    installCommands();
  });

  it.each(["app:settings", "app:launchAtLogin", "window:close", "app:background", "app:quit", "timer:start", "timer:toggle", "app:logs", "app:dataFolder", "app:readme"])("%s asks the bus to run its own id", (id) => {
    expect(runCommand(id)).toBe(true);
    expect(mocked.runCommand).toHaveBeenCalledWith(id);
  });

  it.each([
    ["app:settings", "Mod+,"],
    ["window:close", "Mod+W"],
    ["app:background", "Mod+Q"],
    ["timer:start", "Mod+T"],
    ["timer:stop", "Mod+Shift+."],
    ["app:quit", null],
    ["tracker:pause:resumed", null],
  ])("%s has the default binding %s", (id, binding) => {
    expect(bindingFor(id)).toBe(binding);
  });

  it("registers every bus command in the global scope, flagged as a bus command", () => {
    for (const { id } of BUS_COMMANDS) expect(getCommand(id)).toMatchObject({ scope: "global", bus: true });
  });
});
