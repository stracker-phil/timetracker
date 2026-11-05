import { useEffect } from "react";
import { useQueryClient, type QueryKey } from "@tanstack/react-query";
import { DAY_KEY } from "../../queries/day";
import { MONTH_KEY, WEEK_KEY } from "../../queries/calendar";
import { useStore } from "../../state/store";

/** The views whose refetch may carry new times for the edited recording (an undo, a nudge, a drag). */
const VIEW_KEYS: unknown[] = [DAY_KEY[0], WEEK_KEY[0], MONTH_KEY[0]];

/** While `active` (an existing recording is edited), its times follow each refetched day, week or month view. */
export function useEditedSync(active: boolean) {
  const client = useQueryClient();
  useEffect(() => {
    if (!active) return;
    return client.getQueryCache().subscribe((event) => {
      const key = event.query.queryKey as QueryKey;
      if (event.type === "updated" && event.action.type === "success" && VIEW_KEYS.includes(key[0])) {
        void useStore.getState().syncEdited();
      }
    });
  }, [client, active]);
}
