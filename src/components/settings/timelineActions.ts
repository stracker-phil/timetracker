import type { TimelineSettings, Titles } from "../../api";
import { useStore } from "../../state/store";

export const TITLES: readonly Titles[] = ["document", "app"];

/** Saves a timeline setting right away; the Day view reloads. */
export const saveTimeline = (patch: Partial<TimelineSettings>) => void useStore.getState().saveTimeline(patch);
