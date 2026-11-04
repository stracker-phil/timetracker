// Must not import the store or "../api" at runtime: apiMock.ts may load this file while the api module is mocked.
import type { MenuCommand } from "../api/types";

/** A menu item as `menu_commands` lists it; `patch` overrides fields. */
export function menuCommand(patch: Partial<MenuCommand> = {}): MenuCommand {
  return { menu: "File", label: "Item", command: "page:recording.new", keys: null, checked: false, ...patch };
}

/** A few items of the app, File, Edit and View menus, in menu order. */
export function menuCommands(): MenuCommand[] {
  return [
    menuCommand({ menu: "TimeTracker", label: "Settings…", command: "app:settings", keys: "⌘," }),
    menuCommand({ menu: "File", label: "Start Timer…", command: "timer:start", keys: "⌘T" }),
    menuCommand({ menu: "File", label: "New Project…", command: "page:projects.new", keys: "⌘N" }),
    menuCommand({ menu: "File", label: "New Rule…", command: "page:rules.new", keys: "⌘N" }),
    menuCommand({ menu: "File", label: "Rename Saved Report…", command: "page:reports.rename" }),
    menuCommand({ menu: "File", label: "Delete Saved Report…", command: "page:reports.delete" }),
    menuCommand({ menu: "Edit", label: "Copy Report as Text", command: "page:reports.copyText", keys: "⌘⇧C" }),
    menuCommand({ menu: "View", label: "Day", command: "page:view.day", keys: "⌘1", checked: true }),
    menuCommand({ menu: "View", label: "Week", command: "page:view.week", keys: "⌘2" }),
    menuCommand({ menu: "View", label: "Zoom In", command: "page:zoom.in", keys: "⌘=" }),
    menuCommand({ menu: "View", label: "Zoom Out", command: "page:zoom.out", keys: "⌘-" }),
    menuCommand({ menu: "View", label: "Reset Zoom", command: "page:zoom.reset", keys: "⌘0" }),
  ];
}
