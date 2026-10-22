import type { ActivityBlock, AppSettingPatch, Lane, Settings } from "../../api";
import { copyText } from "../../lib/clipboard";
import { useStore } from "../../state/store";
import type { MenuEntry } from "../ContextMenu";
import { draftFromBlock } from "../rules/conditions";

const set = (appKey: string, patch: AppSettingPatch) => useStore.getState().setAppSetting(appKey, patch);

function hide(block: ActivityBlock) {
  const { toast } = useStore.getState();
  const unhide = () => void set(block.appKey, { hidden: false });
  void set(block.appKey, { hidden: true }).then((hidden) => hidden && toast("info", `${block.app} is hidden from the timeline`, { label: "Undo", run: unhide }));
}

function appEntries(block: ActivityBlock, lane: Lane, settings: Settings): MenuEntry[] {
  const { appKey, app } = block;
  const ownLane = lane.kind === "app" || !!settings.apps[appKey]?.ownLane;
  return [
    { label: "Own lane", checked: ownLane, action: () => void set(appKey, { ownLane: !ownLane }) },
    { label: `Hide ${app}`, action: () => hide(block) },
    "separator",
  ];
}

/** App settings apply to tracker blocks only. */
export function blockMenu(block: ActivityBlock, lane: Lane, settings: Settings | null): MenuEntry[] {
  const entries = block.source === "tracker" && settings ? appEntries(block, lane, settings) : [];
  if (!block.private) entries.push({ label: "Copy title", action: () => void copyText(block.title, "the title") });
  const { url } = block;
  if (url) entries.push({ label: "Copy URL", action: () => void copyText(url, "the URL") });
  if (entries.length > 0 && entries.at(-1) !== "separator") entries.push("separator");
  entries.push({ label: "Make a rule…", action: () => useStore.getState().openDialog({ type: "rule", draft: draftFromBlock(block, null) }) });
  return entries;
}
