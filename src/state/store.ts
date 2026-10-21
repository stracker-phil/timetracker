import { create, type StoreApi } from "zustand";
import { dataSlice, type DataSlice } from "./slices/data";
import { daySlice, type DaySlice } from "./slices/day";
import { navSlice, type NavSlice } from "./slices/nav";
import { projectsSlice, type ProjectsSlice } from "./slices/projects";
import { recordingsSlice, type RecordingsSlice } from "./slices/recordings";
import { reportsSlice, type ReportsSlice } from "./slices/reports";
import { rulesSlice, type RulesSlice } from "./slices/rules";
import { timerSlice, type TimerSlice } from "./slices/timer";
import { settingsSlice, type SettingsSlice } from "./slices/settings";
import { trackerSlice, type TrackerSlice } from "./slices/tracker";
import { uiSlice, type UiSlice } from "./slices/ui";
import { viewStateSlice, type ViewStateSlice } from "./slices/viewState";

export { fail } from "./requests";
export { selectDayVersion } from "./slices/data";
export { flushUiState } from "./slices/viewState";
export type { Dialog, Toast } from "./slices/ui";
export type { View } from "./slices/nav";
export { TIMELINE_ZOOM } from "./slices/viewState";

/** Every slice's members; modules add theirs with `declare module "../state/store" { interface Store { … } }`. */
export interface Store extends SettingsSlice, UiSlice, NavSlice, ViewStateSlice, DataSlice, TrackerSlice, ProjectsSlice, DaySlice, RecordingsSlice, ReportsSlice, RulesSlice, TimerSlice {}

type SetStore = StoreApi<Store>["setState"];
type GetStore = StoreApi<Store>["getState"];

export interface Slice<T> {
  /** Initial state and actions; also re-run by `resetStore`. */
  create(set: SetStore, get: GetStore): T;
}

// Empty until the slices below register; registerSlice fills it.
export const useStore = create<Store>()(() => ({}) as Store);

const slices: Slice<Partial<Store>>[] = [];

/** Adds a slice's state and actions to the live store; works after creation (modules register late). */
export function registerSlice<T extends Partial<Store>>(slice: Slice<T>) {
  slices.push(slice);
  useStore.setState(slice.create(useStore.setState, useStore.getState));
}

/** Restores every registered slice's initial state, then applies `patch`; for tests. */
export function resetStore(patch: Partial<Store> = {}) {
  const initial = slices.reduce<Partial<Store>>((state, slice) => ({ ...state, ...slice.create(useStore.setState, useStore.getState) }), {});
  useStore.setState({ ...initial, ...patch });
}

registerSlice(settingsSlice);
registerSlice(uiSlice);
registerSlice(navSlice);
registerSlice(viewStateSlice);
registerSlice(dataSlice);
registerSlice(trackerSlice);
registerSlice(projectsSlice);
registerSlice(daySlice);
registerSlice(recordingsSlice);
registerSlice(reportsSlice);
registerSlice(rulesSlice);
registerSlice(timerSlice);
