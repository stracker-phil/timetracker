import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { installInputModality } from "./inputModality";

const html = document.documentElement;

describe("installInputModality", () => {
  let uninstall: () => void;
  let input: HTMLInputElement;

  beforeEach(() => {
    delete html.dataset.input;
    uninstall = installInputModality();
    input = document.body.appendChild(document.createElement("input"));
  });

  afterEach(() => {
    uninstall();
    input.remove();
    delete html.dataset.input;
  });

  it("marks the keyboard when a key is pressed outside a text field", async () => {
    await userEvent.keyboard("j");
    expect(html.dataset.input).toBe("keyboard");
  });

  it.each(["{Meta}", "{Shift}", "{Alt}", "{Control}"])("ignores the modifier key %s", async (key) => {
    await userEvent.keyboard(key);
    expect(html.dataset.input).toBeUndefined();
  });

  it("marks the keyboard for Tab inside a text field", async () => {
    input.focus();
    await userEvent.keyboard("{Tab}");
    expect(html.dataset.input).toBe("keyboard");
  });

  it("ignores typing inside a text field", async () => {
    input.focus();
    await userEvent.keyboard("abc");
    expect(html.dataset.input).toBeUndefined();
  });

  it("clears the mark on a pointer press", async () => {
    await userEvent.keyboard("j");
    await userEvent.click(document.body);
    expect(html.dataset.input).toBeUndefined();
  });

  it("marks the element it was given instead of the page", async () => {
    const root = document.createElement("div");
    const stop = installInputModality(root);
    await userEvent.keyboard("j");
    stop();
    expect(root.dataset.input).toBe("keyboard");
  });

  it("stops reacting once uninstalled", async () => {
    uninstall();
    await userEvent.keyboard("j");
    expect(html.dataset.input).toBeUndefined();
  });
});
