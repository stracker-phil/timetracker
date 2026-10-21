import { create, type StoreApi } from "zustand";
import { dataSlice, type DataSlice } from "./slices/data";
import { settingsSlice, type SettingsSlice } from "./slices/settings";
import { uiSlice, type UiSlice } from "./slices/ui";
import { viewStateSlice, type ViewStateSlice } from "./slices/viewState";

export { fail } from "./requests";
export { flushUiState } from "./slices/viewState";
export type { Dialog, Toast, View } from "./slices/ui";

/** Every slice's members; modules add theirs with `declare module "../state/store" { interface Store { … } }`. */
export interface Store extends SettingsSlice, UiSlice, ViewStateSlice, DataSlice {}

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
registerSlice(viewStateSlice);
registerSlice(dataSlice);
