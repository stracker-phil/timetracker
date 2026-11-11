import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { api, type FoundEntry } from "../api";

const NONE: FoundEntry[] = [];

// Asked anew on each opening of the palette (`gcTime: 0`), so they need no event of their own.

/** The recordings matching `text`; nothing is asked while it is null. */
export function useRevealRecordings(text: string | null): FoundEntry[] {
  const { data } = useQuery({
    queryKey: ["revealRecordings", text],
    queryFn: () => api.revealRecordings(text ?? ""),
    enabled: text !== null,
    gcTime: 0,
    placeholderData: keepPreviousData,
    meta: { failure: "Could not search the recordings" },
  });
  return text === null ? NONE : (data ?? NONE);
}

/** The last days' suggestions, asked once while `enabled`. */
export function useRevealSuggestions(enabled: boolean): FoundEntry[] {
  const { data } = useQuery({
    queryKey: ["revealSuggestions"],
    queryFn: api.revealSuggestions,
    enabled,
    gcTime: 0,
    meta: { failure: "Could not load the suggestions" },
  });
  return enabled ? (data ?? NONE) : NONE;
}
