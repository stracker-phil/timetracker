/** Answers requests from other processes that only the page can answer. */
import { api, toAppError, type RemoteEvent, type RemoteRequest } from "../api";

function handle(request: RemoteRequest): Promise<unknown> {
  return Promise.reject(new Error(`Unknown request ${JSON.stringify(request)}`));
}

export async function answerRemote({ id, request }: RemoteEvent): Promise<void> {
  let result: unknown = null;
  let error: string | null = null;
  try {
    result = (await handle(request)) ?? null;
  } catch (e) {
    error = toAppError(e).message;
  }
  await api.remoteReply(id, result, error).catch((e) => api.logError(`Remote reply failed: ${toAppError(e).message}`));
}
