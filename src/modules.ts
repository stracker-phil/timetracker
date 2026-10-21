/**
 * The frontend's install list: the core first, then one line per module. Removing a module is deleting
 * its folder and its line here; the core never imports a module.
 */
import { installCommands } from "./commands";
import { registerCoreSettingsPages } from "./components/settings/pages";

/** Call once, before the first render. */
export function installModules() {
  registerCoreSettingsPages();
  installCommands();
}
