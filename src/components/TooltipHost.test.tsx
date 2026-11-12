import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipHost } from "./TooltipHost";

function renderHost() {
  render(
    <>
      <button type="button" title="Start the timer" />
      <button type="button" title="Sort the rows">
        Sort
      </button>
      <button type="button" title="Open the settings" />
      <span>No tip here</span>
      <TooltipHost />
    </>,
  );
}

const iconOnly = () => screen.getByRole("button", { name: "Start the timer" });
const second = () => screen.getByRole("button", { name: "Open the settings" });
const wait = (ms: number) => act(() => vi.advanceTimersByTimeAsync(ms));

function boxAt(element: Element, top: number, bottom: number) {
  element.getBoundingClientRect = () => ({ left: 100, right: 120, top, bottom, width: 20, height: bottom - top, x: 100, y: top, toJSON: () => ({}) });
}

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("TooltipHost", () => {
  describe("titles", () => {
    it("moves the title to data-tooltip so the native tooltip never shows", () => {
      renderHost();

      fireEvent.pointerOver(iconOnly());

      expect(iconOnly()).not.toHaveAttribute("title");
      expect(iconOnly()).toHaveAttribute("data-tooltip", "Start the timer");
    });

    it("keeps an element named only by its title named, as aria-label", () => {
      renderHost();

      fireEvent.pointerOver(iconOnly());

      expect(iconOnly()).toHaveAttribute("aria-label", "Start the timer");
    });

    it("leaves the name of an element with its own text alone", () => {
      renderHost();
      const sort = screen.getByRole("button", { name: "Sort" });

      fireEvent.pointerOver(sort);

      expect(sort).not.toHaveAttribute("aria-label");
      expect(sort).toHaveAttribute("data-tooltip", "Sort the rows");
    });
  });

  describe("pointer timing", () => {
    it("shows the title as a tooltip only after the pointer has rested 450ms", async () => {
      renderHost();

      fireEvent.pointerOver(iconOnly());
      await wait(300);
      expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
      await wait(200);

      expect(screen.getByRole("tooltip")).toHaveTextContent("Start the timer");
    });

    it("shows nothing when the pointer leaves before the delay is over", async () => {
      renderHost();

      fireEvent.pointerOver(iconOnly());
      await wait(200);
      fireEvent.pointerOver(screen.getByText("No tip here"));
      await wait(600);

      expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
    });

    it("shows the next element's tooltip at once while one is showing", async () => {
      renderHost();
      fireEvent.pointerOver(iconOnly());
      await wait(450);

      fireEvent.pointerOver(second());

      expect(screen.getByRole("tooltip")).toHaveTextContent("Open the settings");
    });

    it("shows the next element's tooltip at once within 600ms of the last one", async () => {
      renderHost();
      fireEvent.pointerOver(iconOnly());
      await wait(450);
      fireEvent.pointerOver(screen.getByText("No tip here"));
      expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
      await wait(400);

      fireEvent.pointerOver(second());

      expect(screen.getByRole("tooltip")).toHaveTextContent("Open the settings");
    });

    it("waits the full delay again once the tooltips have been gone for over 600ms", async () => {
      renderHost();
      fireEvent.pointerOver(iconOnly());
      await wait(450);
      fireEvent.pointerOver(screen.getByText("No tip here"));
      await wait(700);

      fireEvent.pointerOver(second());
      expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
      await wait(450);

      expect(screen.getByRole("tooltip")).toHaveTextContent("Open the settings");
    });

    it.each([
      ["a press", (el: HTMLElement) => fireEvent.pointerDown(el)],
      ["a key", (el: HTMLElement) => fireEvent.keyDown(el, { key: "a" })],
      ["scrolling", (el: HTMLElement) => fireEvent.wheel(el)],
      ["the window losing focus", () => fireEvent.blur(window)],
    ])("hides the tooltip on %s", async (_name, hide) => {
      renderHost();
      fireEvent.pointerOver(iconOnly());
      await wait(450);
      expect(screen.getByRole("tooltip")).toBeInTheDocument();

      hide(iconOnly());

      expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
    });
  });

  describe("keyboard focus", () => {
    const focusVisible = (visible: boolean) => {
      const matches = Element.prototype.matches;
      vi.spyOn(Element.prototype, "matches").mockImplementation(function (this: Element, selector: string) {
        return selector === ":focus-visible" ? visible : matches.call(this, selector);
      });
    };

    it("shows the tooltip after the delay when focus is visible", async () => {
      focusVisible(true);
      renderHost();

      act(() => iconOnly().focus());
      await wait(300);
      expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
      await wait(200);

      expect(screen.getByRole("tooltip")).toHaveTextContent("Start the timer");
    });

    it("shows nothing when the element took focus from a click", async () => {
      focusVisible(false);
      renderHost();

      act(() => iconOnly().focus());
      await wait(600);

      expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
    });
  });

  describe("text cut off with an ellipsis", () => {
    const TITLE = "A very long project name that does not fit";

    function ellipsis(element: HTMLElement, scrollWidth: number, clientWidth: number) {
      element.style.textOverflow = "ellipsis";
      Object.defineProperty(element, "scrollWidth", { configurable: true, value: scrollWidth });
      Object.defineProperty(element, "clientWidth", { configurable: true, value: clientWidth });
      return element;
    }

    const hover = async (element: Element) => {
      fireEvent.pointerOver(element);
      await wait(450);
    };

    it("shows its full text as the tooltip", async () => {
      render(
        <>
          <span>{TITLE}</span>
          <TooltipHost />
        </>,
      );

      await hover(ellipsis(screen.getByText(TITLE), 300, 100));

      expect(screen.getByRole("tooltip")).toHaveTextContent(TITLE);
    });

    it("shows no tooltip when the text fits", async () => {
      render(
        <>
          <span>{TITLE}</span>
          <TooltipHost />
        </>,
      );

      await hover(ellipsis(screen.getByText(TITLE), 100, 100));

      expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
    });

    it("takes precedence over a title around it", async () => {
      render(
        <>
          <button type="button" title="Open the project">
            <span>{TITLE}</span>
          </button>
          <TooltipHost />
        </>,
      );

      await hover(ellipsis(screen.getByText(TITLE), 300, 100));

      expect(screen.getByRole("tooltip")).toHaveTextContent(TITLE);
    });

    it("falls back to the title when the text fits", async () => {
      render(
        <>
          <button type="button" title="Open the project">
            <span>{TITLE}</span>
          </button>
          <TooltipHost />
        </>,
      );

      await hover(ellipsis(screen.getByText(TITLE), 100, 100));

      expect(screen.getByRole("tooltip")).toHaveTextContent("Open the project");
    });

    it("shows the cut-off text when the pointer is on an element inside it", async () => {
      render(
        <>
          <span>
            <b>Bold</b> {TITLE}
          </span>
          <TooltipHost />
        </>,
      );
      const bold = screen.getByText("Bold");
      ellipsis(bold.parentElement!, 300, 100);

      await hover(bold);

      expect(screen.getByRole("tooltip")).toHaveTextContent(`Bold ${TITLE}`);
    });
  });

  describe("placement", () => {
    it("places the tooltip below the control when there is room", async () => {
      renderHost();
      boxAt(iconOnly(), 10, 30);

      fireEvent.pointerOver(iconOnly());
      await wait(450);

      const tooltip = screen.getByRole("tooltip");
      expect(tooltip).not.toHaveClass("tooltip--above");
      expect(tooltip.style.top).toBe("36px");
    });

    it("places the tooltip above the control when there is no room below", async () => {
      renderHost();
      boxAt(iconOnly(), window.innerHeight - 30, window.innerHeight - 10);

      fireEvent.pointerOver(iconOnly());
      await wait(450);

      expect(screen.getByRole("tooltip")).toHaveClass("tooltip--above");
    });
  });
});
