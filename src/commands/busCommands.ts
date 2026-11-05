/** The bus commands (ADR 0038) the page offers keys for: Rust runs them (`api.runCommand`), the registry keeps
 *  them so Settings → Shortcuts rebinds them and their keys work while the window has the focus (the menu's
 *  accelerators take over elsewhere). Each one in the menus has its item's default binding (`menu/items.rs`). */
import { api } from "../api";
import { fail } from "../state/requests";
import type { Command } from "./registry";
import { currentTimer, currentTracker } from "../queries/status";

const timerRuns = () => !!currentTimer()?.running;
const trackerKind = () => currentTracker()?.state.kind;
const trackerPausable = () => {
  const kind = trackerKind();
  return kind !== undefined && kind !== "paused";
};

function bus(id: string, title: string, more: Partial<Command> = {}): Command {
  return { id, title, scope: "global", bus: true, run: () => void api.runCommand(id).catch((e) => fail(e, `Couldn't run ${title}`)), ...more };
}

export const BUS_COMMANDS: Command[] = [
  bus("app:settings", "Settings…", { defaultBinding: "Mod+," }),
  bus("app:launchAtLogin", "Launch at login", { menuOnly: true }),
  bus("window:close", "Close window", { defaultBinding: "Mod+W" }),
  bus("app:background", "Hide to Menu Bar", { defaultBinding: "Mod+Q" }),
  bus("app:quit", "Quit TimeTracker"),
  bus("timer:start", "Start timer…", { defaultBinding: "Mod+T" }),
  bus("timer:stop", "Stop timer", { defaultBinding: "Mod+Shift+.", when: timerRuns }),
  bus("timer:toggle", "Start or stop timer"),
  bus("tracker:pause:resumed", "Pause tracking", { when: trackerPausable }),
  bus("tracker:resume", "Resume tracking", { when: () => trackerKind() === "paused" }),
  bus("app:logs", "Open log folder"),
  bus("app:dataFolder", "Show data folder"),
  bus("app:readme", "Open README"),
];
