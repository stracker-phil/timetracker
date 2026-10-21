import { render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { installCommands } from "../commands";
import { resetStore } from "../state/store";
import { ShortcutsDialog } from "./ShortcutsDialog";

describe("ShortcutsDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ view: "day" });
    installCommands();
  });

  const row = (group: HTMLElement, title: string) => within(group).getByText(title).closest("div") as HTMLElement;

  it("lists the date navigation keys in the group shared by Day, Week, Month and Reports", () => {
    render(<ShortcutsDialog scopes={["day", "dated", "global"]} />);
    const group = screen.getByRole("region", { name: "Day, Week, Month and Reports" });
    expect(within(row(group, "Previous day, week, month or period")).getByText("P")).toBeInTheDocument();
    expect(within(row(group, "Next day, week, month or period")).getByText("N")).toBeInTheDocument();
    expect(within(row(group, "Go to today")).getByText("O")).toBeInTheDocument();
  });

  it("lists ] and [ for recordings in the Day group", () => {
    render(<ShortcutsDialog scopes={["day", "dated", "global"]} />);
    const group = screen.getByRole("region", { name: "Day" });
    expect(within(row(group, "Next recording")).getByText("]")).toBeInTheDocument();
    expect(within(row(group, "Previous recording")).getByText("[")).toBeInTheDocument();
  });

  it("omits the date navigation group where the view has no period", () => {
    render(<ShortcutsDialog scopes={["projects", "global"]} />);
    expect(screen.queryByRole("region", { name: "Day, Week, Month and Reports" })).not.toBeInTheDocument();
  });
});
