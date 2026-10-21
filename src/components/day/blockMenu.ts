import type { ActivityBlock, AppSettingPatch, Lane, Settings, Titles } from "../../api";
import { copyText } from "../../lib/clipboard";
import { useStore } from "../../state/store";
import type { MenuEntry } from "../ContextMenu";

const set = (appKey: string, patch: AppSettingPatch) => useStore.getState().setAppSetting(appKey, patch);

function hide(block: ActivityBlock) {
  const { toast } = useStore.getState();
  const unhide = () => void set(block.appKey, { hidden: false });
  void set(block.appKey, { hidden: true }).then((hidden) => hidden && toast("info", `${block.app} is hidden from the timeline`, { label: "Undo", run: unhide }));
}

/** The app entries of a tracker block, checked by the effective setting; an override equal to the
 *  timeline's own setting is removed rather than stored. */
function appEntries(block: ActivityBlock, lane: Lane, settings: Settings): MenuEntry[] {
  const { appKey, app } = block;
  const override = settings.apps[appKey];
  const grouped = override?.groupByApp ?? settings.timeline.groupByApp;
  const titles = override?.titles ?? settings.timeline.titles;
  const setTitles = (to: Titles) => void set(appKey, { titles: to === settings.timeline.titles ? null : to });
  const setGrouped = (to: boolean) => void set(appKey, { groupByApp: to === settings.timeline.groupByApp ? null : to });
  const ownLane = lane.kind === "app" || !!override?.ownLane;
  return [
    { label: "Group by app", checked: grouped, action: () => setGrouped(!grouped) },
    { label: "Titles: Document name", checked: titles === "document", action: () => setTitles("document") },
    { label: "Titles: App name", checked: titles === "app", action: () => setTitles("app") },
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
  if (entries.at(-1) === "separator") entries.pop();
  return entries;
}
