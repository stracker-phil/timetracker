import React from "react";
import ReactDOM from "react-dom/client";
import { ErrorBoundary } from "../../components/ErrorBoundary";
import { api, toAppError } from "../../api";
import { applyInitialLook } from "../../lib/theme";
import "../../styles";
import { AwayPrompt } from "./AwayPrompt";

applyInitialLook();
// This window has no toasts: its errors go to the log.
window.addEventListener("error", (e) => void api.logError(`Away prompt: ${e.message}`).catch(() => {}));
window.addEventListener("unhandledrejection", (e) => void api.logError(`Away prompt: ${toAppError(e.reason).message}`).catch(() => {}));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <AwayPrompt />
    </ErrorBoundary>
  </React.StrictMode>,
);
