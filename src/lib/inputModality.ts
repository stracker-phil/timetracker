import { isEditable } from "./nativeMenu";

const MODIFIERS = new Set(["Meta", "Alt", "Control", "Shift", "CapsLock", "Fn"]);

/**
 * Marks `<html data-input="keyboard">` while the keyboard moves around the page: a key press outside a text field
 * (or Tab in one) sets it, a pointer press clears it. The key focus styles (`.focused`) show only with it, so a
 * click never draws a focus outline (docs/ux.md, Focus).
 */
export function installInputModality(root: HTMLElement = document.documentElement): () => void {
  const onKey = (e: KeyboardEvent) => {
    if (MODIFIERS.has(e.key) || (e.key !== "Tab" && isEditable(e.target))) return;
    root.dataset.input = "keyboard";
  };
  const onPointer = () => delete root.dataset.input;
  window.addEventListener("keydown", onKey, true);
  window.addEventListener("pointerdown", onPointer, true);
  return () => {
    window.removeEventListener("keydown", onKey, true);
    window.removeEventListener("pointerdown", onPointer, true);
  };
}
