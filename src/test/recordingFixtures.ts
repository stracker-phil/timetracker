// Must not import the store or "../api" at runtime: apiMock.ts imports this file while the api module is mocked.
import type { Recording } from "../api/types";
import { at } from "./dayFixtures";

/** A 09:00 to 09:45 recording of `DAY` for the default project; `patch` overrides fields. */
export function recording(patch: Partial<Recording> = {}): Recording {
  return {
    id: "r1",
    projectId: "project-1",
    start: at("09:00"),
    end: at("09:45"),
    note: "",
    tagId: null,
    createdAt: 0,
    updatedAt: 0,
    ...patch,
  };
}
