/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** "1" in the debug app bundle (`bun run build:app`, `bun run app`), which the dev scripts drive. */
  readonly VITE_DEV_HOOKS?: string;
}

interface Window {
  timetracker?: typeof import("./state/store").useStore;
  /** The window's query cache (`queryClient.getQueryData(["settings"])`), beside the store. */
  timetrackerQueries?: typeof import("./queries/queryClient").queryClient;
}
