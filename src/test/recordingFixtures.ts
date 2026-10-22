// Must not import the store or "../api" at runtime: apiMock.ts imports this file while the api module is mocked.
import type { Recording, RecordingSpan } from "../api/types";
import { at } from "./dayFixtures";

/** The id the fixtures give the first span of recording `id`. */
export const spanIdOf = (id: string) => `${id}-s1`;

/**
 * A recording of `DAY` for the default project with one span, 09:00 to 09:45. `start` and `end` shape that
 * span (a null `end` is the timer's open span); `spans` replaces the list; `patch` overrides other fields.
 */
export function recording(patch: Partial<Recording> & { start?: number; end?: number | null } = {}): Recording {
  const { start, end, spans, ...rest } = patch;
  const id = rest.id ?? "r1";
  const shaped: RecordingSpan[] =
    start !== undefined || end !== undefined
      ? [{ id: spans?.[0]?.id ?? spanIdOf(id), start: start ?? at("09:00"), end: end === undefined ? at("09:45") : end }]
      : (spans ?? [{ id: spanIdOf(id), start: at("09:00"), end: at("09:45") }]);
  return {
    id,
    projectId: "project-1",
    note: "",
    tagId: null,
    createdAt: 0,
    updatedAt: 0,
    ...rest,
    spans: shaped,
  };
}
