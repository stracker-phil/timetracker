/** Answers requests from other processes (`timetracker show`, `select`, `run page:<id>`) that only the page
 *  can answer. */
import { api, toAppError, type RemoteEvent, type RemoteRequest } from "../api";
import { useStore } from "../state/store";
import { canRunFromMenu } from "./pageCommands";
import { getCommand } from "./registry";

function runPageCommand(id: string) {
  const command = getCommand(id);
  if (!command?.menu) throw new Error(`Unknown command “page:${id}”.`);
  if (!canRunFromMenu(command)) throw new Error(`“page:${id}” isn't available right now.`);
  void command.run();
}

function handle(request: RemoteRequest): unknown {
  switch (request.type) {
    case "showDay":
      useStore.getState().goToDay(request.day);
      return null;
    case "select":
      useStore.getState().revealEntry(request.entry);
      return null;
    case "runCommand":
      runPageCommand(request.id);
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
