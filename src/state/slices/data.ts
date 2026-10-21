import type { DataChanged } from "../../api";
import type { Slice, Store } from "../store";

export interface DataSlice {
  /** Bumped when data of every day may have changed. */
  dataVersion: number;
  /** Bumped per day (`YYYY-MM-DD`) when that day's data changed. */
  dayVersions: Record<string, number>;
  /** A `null` day bumps every day. */
  dataChanged: (event: DataChanged) => void;
}

export const dataSlice: Slice<DataSlice> = {
  create: (set) => ({
    dataVersion: 0,
    dayVersions: {},
    dataChanged({ day }) {
      if (day === null) set((s) => ({ dataVersion: s.dataVersion + 1 }));
      else set((s) => ({ dayVersions: { ...s.dayVersions, [day]: (s.dayVersions[day] ?? 0) + 1 } }));
    },
  }),
};

/** Changes whenever `day`'s data may have changed; views showing the day key their loads on it. */
export const selectDayVersion = (day: string) => (s: Store) => `${s.dataVersion}.${s.dayVersions[day] ?? 0}`;
