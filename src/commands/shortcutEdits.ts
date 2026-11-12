/** Changing a command's key in Settings → Shortcuts: one binding per command, and an override only while the
 *  key differs from the default. */
import { allCommands, bindingFor, getCommand, scopesOverlap, type Command, type Overrides } from "./registry";

export const isDefaultBinding = (id: string, custom: Overrides): boolean => bindingFor(id, custom) === bindingFor(id, {});

/** The other command that already holds `binding` where `id`'s key works; undefined while the key is free. */
export function holderOf(id: string, binding: string, custom: Overrides): Command | undefined {
  const scope = getCommand(id)?.scope;
  if (!scope) return undefined;
  return allCommands().find((c) => c.id !== id && scopesOverlap(c.scope, scope) && bindingFor(c.id, custom) === bindingFor(id, { [id]: binding }));
}

/** `custom` with `id` bound to `binding` (null unbinds); a binding equal to the default drops the override. */
export function bind(custom: Overrides, id: string, binding: string | null): Overrides {
  const { [id]: _previous, ...rest } = custom;
  const next = { ...rest, [id]: binding };
  return isDefaultBinding(id, next) ? rest : next;
}

/** Moves `binding` from `holder` to `id` in one change: `id` takes it, `holder` loses its key. */
export function reassign(custom: Overrides, id: string, binding: string, holder: string): Overrides {
  return bind(bind(custom, holder, null), id, binding);
}
