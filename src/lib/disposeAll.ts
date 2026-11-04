/** Ends event subscriptions (`api.on…`), including those still being set up; a failed one is ignored. */
export function disposeAll(subscriptions: Promise<() => void>[]): void {
  subscriptions.forEach((p) => void p.then((fn) => fn()).catch(() => {}));
}
