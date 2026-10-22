import { api } from "../../api";
import type { Slice } from "../store";

export interface AppIconsSlice {
  /** A PNG data URL per bundle id; `null` once known to have none, absent until asked. */
  appIcons: Record<string, string | null>;
  /** Asks Rust once per bundle id; the asks of one task go out as one request. */
  requestAppIcon: (bundleId: string) => void;
}

export const appIconsSlice: Slice<AppIconsSlice> = {
  create: (set) => {
    const asked = new Set<string>();
    let queued: string[] = [];
    const settle = (ids: string[], urls: Record<string, string>) =>
      set((s) => ({ appIcons: { ...s.appIcons, ...Object.fromEntries(ids.map((id) => [id, urls[id] ?? null])) } }));
    const flush = () => {
      const ids = queued;
      queued = [];
      // Icons only decorate: without them the page shows the initials.
      api.appIcons(ids).then(
        (urls) => settle(ids, urls),
        () => settle(ids, {}),
      );
    };
    return {
      appIcons: {},
      requestAppIcon(bundleId) {
        if (asked.has(bundleId)) return;
        asked.add(bundleId);
        if (queued.length === 0) queueMicrotask(flush);
        queued.push(bundleId);
      },
    };
  },
};
