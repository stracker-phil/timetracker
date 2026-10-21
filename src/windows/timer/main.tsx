import React from "react";
import ReactDOM from "react-dom/client";
import { ErrorBoundary } from "../../components/ErrorBoundary";
import { api, toAppError } from "../../api";
import { applyInitialScheme } from "../../lib/theme";
import "../../styles";
import { TimerPicker } from "./TimerPicker";

applyInitialScheme();
// This window has no toasts: its errors go to the log.
window.addEventListener("error", (e) => void api.logError(`Timer picker: ${e.message}`).catch(() => {}));
window.addEventListener("unhandledrejection", (e) => void api.logError(`Timer picker: ${toAppError(e.reason).message}`).catch(() => {}));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <TimerPicker />
    </ErrorBoundary>
  </React.StrictMode>,
);
