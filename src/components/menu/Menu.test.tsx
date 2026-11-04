import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { resetStore } from "../../state/store";
import { Modal } from "../Modal";
import { MenuHost, openContextMenu, openMenu, type MenuEntry } from "./index";

function Harness({ entries, label = "Actions" }: { entries: MenuEntry[]; label?: string }) {
  return (
    <>
      <button type="button" onClick={(e) => openMenu(entries, e.currentTarget, label)}>
        Open
      </button>
      <button type="button">Elsewhere</button>
      <div data-testid="area" onContextMenu={(e) => openContextMenu(e, entries, label)}>
        Area
      </div>
      <MenuHost />
    </>
  );
}

function setup(entries: MenuEntry[], label?: string) {
  return render(<Harness entries={entries} label={label} />);
}

const opener = () => screen.getByRole("button", { name: "Open" });
const menu = (name = "Actions") => screen.queryByRole("menu", { name });

async function openFromButton() {
  await userEvent.click(opener());
}

function keyed(run = vi.fn()): MenuEntry[] {
  return [{ text: "Apple", run }, { text: "Banana", enabled: false, run }, { text: "Cherry", run }, { text: "Date", run }];
}

describe("Menu", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    document.documentElement.removeAttribute("data-input");
  });

  describe("roles", () => {
    it("is a menu named by its label", async () => {
      setup(keyed());
      await openFromButton();
      expect(menu("Actions")).toBeInTheDocument();
    });

    it("falls back to the name Menu without a label", async () => {
      render(
        <>
          <button type="button" onClick={(e) => openMenu(keyed(), e.currentTarget)}>
            Open
          </button>
          <MenuHost />
        </>,
      );
      await openFromButton();
      expect(screen.getByRole("menu", { name: "Menu" })).toBeInTheDocument();
    });

    it("lists plain rows as menuitems", async () => {
      setup(keyed());
      await openFromButton();
      expect(screen.getAllByRole("menuitem").map((row) => row.textContent)).toEqual(["Apple", "Banana", "Cherry", "Date"]);
    });

    it("lists checked rows as menuitemradio and toggles as menuitemcheckbox, with aria-checked", async () => {
      setup([
        { text: "Small", checked: false },
        { text: "Large", checked: true },
        { text: "Show grid", checked: true, toggle: true },
        { text: "Show hours", checked: false, toggle: true },
      ]);
      await openFromButton();
      expect(screen.getByRole("menuitemradio", { name: "Small" })).toHaveAttribute("aria-checked", "false");
      expect(screen.getByRole("menuitemradio", { name: "Large" })).toHaveAttribute("aria-checked", "true");
      expect(screen.getByRole("menuitemcheckbox", { name: "Show grid" })).toHaveAttribute("aria-checked", "true");
      expect(screen.getByRole("menuitemcheckbox", { name: "Show hours" })).toHaveAttribute("aria-checked", "false");
    });

    it("groups the rows under a caption and separates them with separators", async () => {
      setup([{ text: "Top" }, "separator", { caption: "Move to" }, { text: "Inbox" }, { text: "Archive" }]);
      await openFromButton();
      const group = screen.getByRole("group", { name: "Move to" });
      expect(within(group).getAllByRole("menuitem").map((row) => row.textContent)).toEqual(["Inbox", "Archive"]);
      expect(within(menu()!).getByRole("menuitem", { name: "Top" })).toBeInTheDocument();
      expect(within(menu()!).getAllByRole("separator")).toHaveLength(1);
    });

    it("marks a disabled row aria-disabled and doesn't run it", async () => {
      const run = vi.fn();
      setup([{ text: "Delete", enabled: false, run }]);
      await openFromButton();
      const row = screen.getByRole("menuitem", { name: "Delete" });
      expect(row).toHaveAttribute("aria-disabled", "true");
      await userEvent.click(row);
      expect(run).not.toHaveBeenCalled();
      expect(menu()).toBeInTheDocument();
    });

    it("marks a row that opens a submenu with aria-haspopup", async () => {
      setup([{ text: "More", items: [{ text: "Inner" }] }]);
      await openFromButton();
      expect(screen.getByRole("menuitem", { name: "More" })).toHaveAttribute("aria-haspopup", "menu");
    });
  });

  describe("choosing", () => {
    it("runs a clicked row and closes the menu", async () => {
      const run = vi.fn();
      setup([{ text: "Rename", run }]);
      await openFromButton();
      await userEvent.click(screen.getByRole("menuitem", { name: "Rename" }));
      expect(run).toHaveBeenCalled();
      expect(menu()).not.toBeInTheDocument();
    });

    it("runs a row of a submenu and closes every panel", async () => {
      const run = vi.fn();
      setup([{ text: "More", items: [{ text: "Inner", run }] }]);
      await openFromButton();
      await userEvent.click(screen.getByRole("menuitem", { name: "More" }));
      await userEvent.click(await screen.findByRole("menuitem", { name: "Inner" }));
      expect(run).toHaveBeenCalled();
      expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    });
  });

  describe("keys", () => {
    it("starts on its first enabled row when opened from a button", async () => {
      setup([{ text: "Hidden", enabled: false }, { text: "First" }, { text: "Second" }]);
      await openFromButton();
      expect(screen.getByRole("menuitem", { name: "First" })).toHaveFocus();
    });

    it("starts on no row when opened by a right-click with the mouse", () => {
      setup(keyed());
      fireEvent.contextMenu(screen.getByTestId("area"));
      expect(menu()).toBeInTheDocument();
      expect(screen.getByRole("menuitem", { name: "Apple" })).not.toHaveFocus();
    });

    it("moves down, skipping a disabled row, and wraps to the first", async () => {
      setup(keyed());
      await openFromButton();
      await userEvent.keyboard("{ArrowDown}");
      expect(screen.getByRole("menuitem", { name: "Cherry" })).toHaveFocus();
      await userEvent.keyboard("{ArrowDown}");
      expect(screen.getByRole("menuitem", { name: "Date" })).toHaveFocus();
      await userEvent.keyboard("{ArrowDown}");
      expect(screen.getByRole("menuitem", { name: "Apple" })).toHaveFocus();
    });

    it("moves up, skipping a disabled row, and wraps to the last", async () => {
      setup(keyed());
      await openFromButton();
      await userEvent.keyboard("{ArrowUp}");
      expect(screen.getByRole("menuitem", { name: "Date" })).toHaveFocus();
      await userEvent.keyboard("{ArrowUp}{ArrowUp}");
      expect(screen.getByRole("menuitem", { name: "Apple" })).toHaveFocus();
    });

    it("jumps to the first and last enabled rows on Home and End", async () => {
      setup([{ text: "Off", enabled: false }, { text: "One" }, { text: "Two" }, { text: "Three" }, { text: "Off too", enabled: false }]);
      await openFromButton();
      await userEvent.keyboard("{End}");
      expect(screen.getByRole("menuitem", { name: "Three" })).toHaveFocus();
      await userEvent.keyboard("{Home}");
      expect(screen.getByRole("menuitem", { name: "One" })).toHaveFocus();
    });

    it("jumps to the next enabled row starting with the typed letter", async () => {
      setup(keyed());
      await openFromButton();
      await userEvent.keyboard("d");
      expect(screen.getByRole("menuitem", { name: "Date" })).toHaveFocus();
    });

    it("stays on the row when the typed letter only matches a disabled row", async () => {
      setup(keyed());
      await openFromButton();
      await userEvent.keyboard("b");
      expect(screen.getByRole("menuitem", { name: "Apple" })).toHaveFocus();
    });

    it.each([
      ["Enter", "{Enter}"],
      ["Space", " "],
    ])("runs the highlighted row on %s", async (_name, key) => {
      const run = vi.fn();
      setup(keyed(run));
      await openFromButton();
      await userEvent.keyboard("{ArrowDown}");
      await userEvent.keyboard(key);
      expect(run).toHaveBeenCalled();
      expect(menu()).not.toBeInTheDocument();
    });

    it("opens the submenu of the highlighted row on ArrowRight and focuses its first row", async () => {
      setup([{ text: "More", items: [{ text: "Inner one" }, { text: "Inner two" }] }]);
      await openFromButton();
      await userEvent.keyboard("{ArrowRight}");
      expect(menu("More")).toBeInTheDocument();
      expect(screen.getByRole("menuitem", { name: "Inner one" })).toHaveFocus();
    });

    it("opens the submenu on Enter", async () => {
      setup([{ text: "More", items: [{ text: "Inner" }] }]);
      await openFromButton();
      await userEvent.keyboard("{Enter}");
      expect(menu("More")).toBeInTheDocument();
    });

    it("closes only the submenu on ArrowLeft and returns to its parent row", async () => {
      setup([{ text: "More", items: [{ text: "Inner" }] }]);
      await openFromButton();
      await userEvent.keyboard("{ArrowRight}");
      await userEvent.keyboard("{ArrowLeft}");
      expect(menu("More")).not.toBeInTheDocument();
      expect(menu()).toBeInTheDocument();
      expect(screen.getByRole("menuitem", { name: "More" })).toHaveFocus();
    });

    it("opens a submenu on hover", async () => {
      setup([{ text: "Plain" }, { text: "More", items: [{ text: "Inner" }] }]);
      await openFromButton();
      await userEvent.hover(screen.getByRole("menuitem", { name: "More" }));
      expect(menu("More")).toBeInTheDocument();
      expect(screen.getByRole("menuitem", { name: "More" })).toHaveAttribute("aria-expanded", "true");
    });
  });

  describe("closing", () => {
    it("closes on Escape", async () => {
      setup(keyed());
      await openFromButton();
      await userEvent.keyboard("{Escape}");
      expect(menu()).not.toBeInTheDocument();
    });

    it("closes the whole stack on Escape inside a submenu", async () => {
      setup([{ text: "More", items: [{ text: "Inner" }] }]);
      await openFromButton();
      await userEvent.keyboard("{ArrowRight}");
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    });

    it("closes on a press outside", async () => {
      setup(keyed());
      await openFromButton();
      fireEvent.mouseDown(screen.getByRole("button", { name: "Elsewhere" }));
      expect(menu()).not.toBeInTheDocument();
    });

    it("stays open on a press inside", async () => {
      setup(keyed());
      await openFromButton();
      fireEvent.mouseDown(menu()!);
      fireEvent.mouseDown(screen.getByRole("menuitem", { name: "Apple" }));
      expect(menu()).toBeInTheDocument();
    });

    it("closes on a scroll outside the menu", async () => {
      setup(keyed());
      await openFromButton();
      fireEvent.scroll(document);
      expect(menu()).not.toBeInTheDocument();
    });

    it("stays open when the menu itself scrolls", async () => {
      setup(keyed());
      await openFromButton();
      fireEvent.scroll(menu()!);
      expect(menu()).toBeInTheDocument();
    });

    it("closes when the window loses focus", async () => {
      setup(keyed());
      await openFromButton();
      fireEvent.blur(window);
      expect(menu()).not.toBeInTheDocument();
    });

    it("closes when the opening button is pressed again", async () => {
      setup(keyed());
      await openFromButton();
      await userEvent.click(opener());
      expect(menu()).not.toBeInTheDocument();
    });

    it("replaces the open menu when another opens", async () => {
      setup(keyed(), "First");
      await openFromButton();
      fireEvent.contextMenu(screen.getByTestId("area"));
      expect(screen.getAllByRole("menu")).toHaveLength(1);
    });
  });

  describe("focus", () => {
    it("returns to the opening button when the menu closes with Escape", async () => {
      setup(keyed());
      await openFromButton();
      await userEvent.keyboard("{Escape}");
      expect(opener()).toHaveFocus();
    });

    it("returns to the opening button after a row is chosen", async () => {
      setup(keyed());
      await openFromButton();
      await userEvent.click(screen.getByRole("menuitem", { name: "Apple" }));
      expect(opener()).toHaveFocus();
    });

    it("returns to the element that had the focus when a context menu was opened", async () => {
      setup(keyed());
      screen.getByRole("button", { name: "Elsewhere" }).focus();
      fireEvent.contextMenu(screen.getByTestId("area"));
      await userEvent.keyboard("{Escape}");
      expect(screen.getByRole("button", { name: "Elsewhere" })).toHaveFocus();
    });

    it("keeps a focus a chosen row moved elsewhere", async () => {
      setup([{ text: "Go", run: () => screen.getByRole("button", { name: "Elsewhere" }).focus() }]);
      await openFromButton();
      await userEvent.click(screen.getByRole("menuitem", { name: "Go" }));
      expect(screen.getByRole("button", { name: "Elsewhere" })).toHaveFocus();
    });
  });

  describe("context menu", () => {
    it("keeps the default context menu shut", () => {
      setup(keyed());
      const notPrevented = fireEvent.contextMenu(screen.getByTestId("area"));
      expect(notPrevented).toBe(false);
      expect(menu()).toBeInTheDocument();
    });

    it("keeps the default context menu shut over the open menu", async () => {
      setup(keyed());
      await openFromButton();
      expect(fireEvent.contextMenu(screen.getByRole("menuitem", { name: "Apple" }))).toBe(false);
    });
  });

  describe("inside a Modal", () => {
    it("closes only the menu on Escape, then the Modal on the next", async () => {
      const onClose = vi.fn();
      render(
        <>
          <Modal onClose={onClose} label="Settings sheet">
            <button type="button" onClick={(e) => openMenu(keyed(), e.currentTarget, "Actions")}>
              Open
            </button>
          </Modal>
          <MenuHost />
        </>,
      );
      await openFromButton();
      await userEvent.keyboard("{Escape}");
      expect(menu()).not.toBeInTheDocument();
      expect(onClose).not.toHaveBeenCalled();
      expect(screen.getByRole("dialog", { name: "Settings sheet" })).toBeInTheDocument();
      await userEvent.keyboard("{Escape}");
      expect(onClose).toHaveBeenCalled();
    });
  });
});
