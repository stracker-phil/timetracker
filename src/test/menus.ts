import { screen } from "@testing-library/react";

/** The part of `userEvent` (the direct API or `userEvent.setup()`) the helpers use. */
interface Typist {
  click: (element: Element) => Promise<unknown>;
  keyboard: (keys: string) => Promise<unknown>;
}

/** Opens the pop-up button with this accessible name and picks one of its choices; needs a `<MenuHost/>`. */
export async function pickFromPopUp(user: Pick<Typist, "click">, name: string | RegExp, option: string) {
  await user.click(screen.getByRole("button", { name }));
  await user.click(await screen.findByRole("menuitemradio", { name: option }));
}

/** The checked choice of the pop-up button with this accessible name; opens its menu and closes it with Escape. */
export async function checkedInPopUp(user: Typist, name: string | RegExp) {
  await user.click(screen.getByRole("button", { name }));
  const checked = (await screen.findAllByRole("menuitemradio")).filter((row) => row.getAttribute("aria-checked") === "true");
  await user.keyboard("{Escape}");
  return checked.map((row) => row.textContent);
}
