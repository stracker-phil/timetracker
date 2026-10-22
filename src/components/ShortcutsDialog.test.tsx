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

  it("lists ] and [ for recordings and suggestions in the Day group", () => {
    render(<ShortcutsDialog scopes={["day", "dated", "global"]} />);
    const group = screen.getByRole("region", { name: "Day" });
    expect(within(row(group, "Next recording or suggestion")).getByText("]")).toBeInTheDocument();
    expect(within(row(group, "Previous recording or suggestion")).getByText("[")).toBeInTheDocument();
  });

  it.each(["projects", "tags", "rules"])("omits the date navigation group in the %s view, which has no period", (view) => {
    render(<ShortcutsDialog scopes={[view, "global"]} />);
    expect(screen.queryByRole("region", { name: "Day, Week, Month and Reports" })).not.toBeInTheDocument();
  });

  it("lists the view keys ⌘1 to ⌘7 among the global ones", () => {
    render(<ShortcutsDialog scopes={["tags", "global"]} />);
    const group = screen.getByRole("region", { name: "Everywhere" });
    expect(within(row(group, "Show Tags")).getByText("⌘6")).toBeInTheDocument();
    expect(within(row(group, "Show Rules")).getByText("⌘7")).toBeInTheDocument();
  });
});
