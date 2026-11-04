import type { ActivityBlock, AppSettingPatch, Lane, Settings } from "../../api";
import { copyText } from "../../lib/clipboard";
import { useStore } from "../../state/store";
import type { MenuEntry } from "../menu";
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
    { text: "Own lane", checked: ownLane, toggle: true, run: () => void set(appKey, { ownLane: !ownLane }) },
    { text: `Hide ${app}`, run: () => hide(block) },
    "separator",
  ];
}

/** App settings apply to tracker blocks only. */
export function blockMenu(block: ActivityBlock, lane: Lane, settings: Settings | null): MenuEntry[] {
  const entries = block.source === "tracker" && settings ? appEntries(block, lane, settings) : [];
  if (!block.private) entries.push({ text: "Copy title", run: () => void copyText(block.title, "the title") });
  const { url } = block;
  if (url) entries.push({ text: "Copy URL", run: () => void copyText(url, "the URL") });
  if (entries.length > 0 && entries.at(-1) !== "separator") entries.push("separator");
  entries.push({ text: "Make a rule…", run: () => useStore.getState().openDialog({ type: "rule", draft: draftFromBlock(block, null) }) });
  return entries;
}
