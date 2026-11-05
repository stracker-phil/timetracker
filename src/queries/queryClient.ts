import { notifyManager, QueryCache, QueryClient, type QueryKey } from "@tanstack/react-query";
import { fail } from "../state/requests";

declare module "@tanstack/react-query" {
  interface Register {
    /** `failure` names what could not load; a failed query with it shows as an error toast. */
    queryMeta: { failure?: string };
  }
}

/** Unused per-key reads (other days, weeks, reports) are dropped after this long. */
const GC_MS = 5 * 60_000;

/** Rust announces every change (`useEvents`), so the cache never refetches on its own. */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    queryCache: new QueryCache({
      onError: (error, query) => {
        if (query.meta?.failure) fail(error, query.meta.failure);
      },
    }),
    defaultOptions: {
      queries: { retry: false, staleTime: Infinity, gcTime: GC_MS, refetchOnWindowFocus: false, refetchOnReconnect: false },
      mutations: { retry: false },
    },
  });
}

/**
 * Each window's one client. Hooks reach it through the `QueryClientProvider` in the window's root; non-hook
 * code (commands, slices) reads and refreshes the same cache through the accessors of `src/queries/`.
 */
export const queryClient = createQueryClient();

/** Refetches the shown queries under `keys`; one already running finishes and counts, so a burst of events doesn't restart it. */
export function invalidate(keys: QueryKey[]) {
  for (const queryKey of keys) void queryClient.invalidateQueries({ queryKey }, { cancelRefetch: false });
}

/**
 * Refetches the queries under `keys` and settles once the components showing them have their new data; for
 * writes whose form closes on fresh lists.
 */
export async function refreshed(...keys: QueryKey[]): Promise<void> {
  await Promise.all(keys.map((queryKey) => queryClient.invalidateQueries({ queryKey })));
  // The cache tells its observers in a scheduled batch; this runs after it.
  await new Promise<void>((resolve) => notifyManager.schedule(resolve));
}
