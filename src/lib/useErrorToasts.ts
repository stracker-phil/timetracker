import { useEffect } from "react";
import { toAppError } from "../api";
import { useStore } from "../state/store";

/** Shows uncaught errors and unhandled rejections of the window as error toasts. */
export function useErrorToasts() {
  useEffect(() => {
    const { toast } = useStore.getState();
    const onError = (e: ErrorEvent) => toast("error", `Unexpected error: ${e.message}`);
    const onRejection = (e: PromiseRejectionEvent) => toast("error", `Unexpected error: ${toAppError(e.reason).message}`);
    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onRejection);
    return () => {
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onRejection);
    };
  }, []);
}
