/** Answers requests from other processes (`timetracker show`) that only the page can answer. */
import { api, toAppError, type RemoteEvent, type RemoteRequest } from "../api";
import { useStore } from "../state/store";

function handle(request: RemoteRequest): unknown {
  switch (request.type) {
    case "showDay":
      useStore.getState().goToDay(request.day);
      return null;
    default:
      throw new Error(`Unknown request ${JSON.stringify(request)}`);
  }
}

export async function answerRemote({ id, request }: RemoteEvent): Promise<void> {
  let result: unknown = null;
  let error: string | null = null;
  try {
    result = handle(request) ?? null;
  } catch (e) {
    error = toAppError(e).message;
  }
  await api.remoteReply(id, result, error).catch((e) => api.logError(`Remote reply failed: ${toAppError(e).message}`));
}
