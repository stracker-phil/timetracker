import type { Slice } from "../store";

export interface DataSlice {
  /** Bumped when Rust reports changed data; views key their `useApiResult` on it and reload. */
  dataVersion: number;
  bumpData: () => void;
}

export const dataSlice: Slice<DataSlice> = {
  create: (set) => ({
    dataVersion: 0,
    bumpData() {
      set((s) => ({ dataVersion: s.dataVersion + 1 }));
    },
  }),
};
